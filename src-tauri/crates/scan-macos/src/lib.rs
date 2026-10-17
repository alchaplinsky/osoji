//! Walker A: getattrlistbulk with a work-stealing deque (milestone M2).
//!
//! Each worker pops a directory task, opens it, drains getattrlistbulk into a
//! thread-local entry list, then appends the whole directory as one contiguous
//! block to the shared arena under a single lock acquisition.

use crossbeam_deque::{Injector, Stealer, Worker};
use scan_core::{Arena, Cancel, Flags, HardlinkSet, Progress, ScanConfig, NodeId, NONE};
use std::ffi::CString;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Duration;

// sys/attr.h, verified against the macOS SDK.
const ATTR_BIT_MAP_COUNT: u16 = 5;
const ATTR_CMN_NAME: u32 = 0x0000_0001;
const ATTR_CMN_DEVID: u32 = 0x0000_0002;
const ATTR_CMN_OBJTYPE: u32 = 0x0000_0008;
const ATTR_CMN_MODTIME: u32 = 0x0000_0400;
const ATTR_CMN_FILEID: u32 = 0x0200_0000;
const ATTR_CMN_ERROR: u32 = 0x2000_0000;
const ATTR_CMN_RETURNED_ATTRS: u32 = 0x8000_0000;
const ATTR_DIR_MOUNTSTATUS: u32 = 0x0000_0004;
const DIR_MNTSTATUS_MNTPOINT: u32 = 0x0000_0001;
const ATTR_FILE_LINKCOUNT: u32 = 0x0000_0001;
const ATTR_FILE_ALLOCSIZE: u32 = 0x0000_0004;
const FSOPT_NOFOLLOW: u64 = 0x1;
const FSOPT_PACK_INVAL_ATTRS: u64 = 0x8;
const O_NOFOLLOW_ANY: libc::c_int = 0x2000_0000;

// vnode types
const VREG: u32 = 1;
const VDIR: u32 = 2;
const VLNK: u32 = 5;

// Record layout given FSOPT_PACK_INVAL_ATTRS. The common group is fixed; the
// dir group is only present for directories and the file group only for
// files, so those are read by cursor using the returned-attributes bitmap.
const OFF_RET_DIRATTR: usize = 12;
const OFF_RET_FILEATTR: usize = 16;
const OFF_ERROR: usize = 24;
const OFF_NAME: usize = 28;
const OFF_DEVID: usize = 36;
const OFF_OBJTYPE: usize = 40;
const OFF_MODTIME: usize = 44;
const OFF_FILEID: usize = 60;
const OFF_TAIL: usize = 68;
const RECORD_MIN: usize = 68;

const BUF_SIZE: usize = 256 * 1024;

#[repr(C)]
struct AttrList {
    bitmapcount: u16,
    reserved: u16,
    commonattr: u32,
    volattr: u32,
    dirattr: u32,
    fileattr: u32,
    forkattr: u32,
}

extern "C" {
    fn getattrlistbulk(
        dirfd: libc::c_int,
        attr_list: *mut AttrList,
        attr_buf: *mut libc::c_void,
        attr_buf_size: libc::size_t,
        options: u64,
    ) -> libc::c_int;
}

fn attrlist() -> AttrList {
    AttrList {
        bitmapcount: ATTR_BIT_MAP_COUNT,
        reserved: 0,
        commonattr: ATTR_CMN_RETURNED_ATTRS
            | ATTR_CMN_ERROR
            | ATTR_CMN_NAME
            | ATTR_CMN_DEVID
            | ATTR_CMN_OBJTYPE
            | ATTR_CMN_MODTIME
            | ATTR_CMN_FILEID,
        volattr: 0,
        dirattr: ATTR_DIR_MOUNTSTATUS,
        fileattr: ATTR_FILE_LINKCOUNT | ATTR_FILE_ALLOCSIZE,
        forkattr: 0,
    }
}

/// Which of the root's direct children a directory sits under, for per-child progress.
#[derive(Clone, Copy)]
enum Top {
    /// The scan root itself: each entry registers its own counter.
    Root,
    Child(u32),
}

struct Task {
    id: NodeId,
    path: CString,
    top: Top,
}

struct Entry {
    name_off: u32,
    name_len: u16,
    size: u64,
    mtime: i64,
    flags: Flags,
    top: u32,
}

struct Shared<'a> {
    arena: Mutex<Arena>,
    links: HardlinkSet,
    progress: &'a Progress,
    injector: Injector<Task>,
    stealers: Vec<Stealer<Task>>,
    pending: AtomicUsize,
    cross_mounts: bool,
    qos: bool,
    cancel: &'a Cancel,
}

#[inline]
fn u32_at(b: &[u8], off: usize) -> u32 {
    u32::from_ne_bytes(b[off..off + 4].try_into().unwrap())
}
#[inline]
fn i32_at(b: &[u8], off: usize) -> i32 {
    i32::from_ne_bytes(b[off..off + 4].try_into().unwrap())
}
#[inline]
fn u64_at(b: &[u8], off: usize) -> u64 {
    u64::from_ne_bytes(b[off..off + 8].try_into().unwrap())
}
#[inline]
fn i64_at(b: &[u8], off: usize) -> i64 {
    i64::from_ne_bytes(b[off..off + 8].try_into().unwrap())
}

pub fn walk(cfg: &ScanConfig, progress: &Progress, cancel: &Cancel) -> io::Result<Arena> {
    let mut arena = Arena::with_capacity(1 << 20, 16 << 20);
    let root = arena.push(NONE, cfg.root.as_os_str().as_bytes(), 0, 0, Flags::DIR);
    let root_path = CString::new(cfg.root.as_os_str().as_bytes()).map_err(|_| io::Error::other("NUL in path"))?;

    let workers: Vec<Worker<Task>> = (0..cfg.threads).map(|_| Worker::new_lifo()).collect();
    let shared = Shared {
        arena: Mutex::new(arena),
        links: HardlinkSet::default(),
        progress,
        injector: Injector::new(),
        stealers: workers.iter().map(|w| w.stealer()).collect(),
        pending: AtomicUsize::new(1),
        cross_mounts: cfg.cross_mounts,
        qos: cfg.qos,
        cancel,
    };
    shared.injector.push(Task { id: root, path: root_path, top: Top::Root });
    progress.add(0, 1, 0, 0); // count the root itself, as the jwalk walker does

    std::thread::scope(|s| {
        for w in workers {
            s.spawn(|| worker_loop(w, &shared));
        }
    });

    let mut arena = shared.arena.into_inner().unwrap();
    arena.aggregate();
    Ok(arena)
}

fn find_task(local: &Worker<Task>, shared: &Shared) -> Option<Task> {
    local.pop().or_else(|| {
        std::iter::repeat_with(|| {
            shared
                .injector
                .steal_batch_and_pop(local)
                .or_else(|| shared.stealers.iter().map(|s| s.steal_batch_and_pop(local)).collect())
        })
        .find(|s| !s.is_retry())
        .and_then(|s| s.success())
    })
}

fn worker_loop(local: Worker<Task>, shared: &Shared) {
    if shared.qos {
        set_user_initiated_qos();
    }
    let mut buf = vec![0u8; BUF_SIZE];
    let mut entries: Vec<Entry> = Vec::with_capacity(1024);
    let mut names: Vec<u8> = Vec::with_capacity(32 * 1024);
    let mut idle_spins = 0u32;
    loop {
        match find_task(&local, shared) {
            Some(task) => {
                idle_spins = 0;
                if !shared.cancel.load(Ordering::Relaxed) {
                    process_dir(task, shared, &local, &mut buf, &mut entries, &mut names);
                }
                shared.pending.fetch_sub(1, Ordering::AcqRel);
            }
            None => {
                if shared.pending.load(Ordering::Acquire) == 0 {
                    return;
                }
                idle_spins += 1;
                if idle_spins < 16 {
                    std::thread::yield_now();
                } else {
                    std::thread::sleep(Duration::from_micros(50));
                }
            }
        }
    }
}

fn mark_error(shared: &Shared, id: NodeId) {
    shared.arena.lock().unwrap().flags[id as usize] |= Flags::ERROR;
    shared.progress.add(0, 0, 0, 1);
}

fn process_dir(
    task: Task,
    shared: &Shared,
    local: &Worker<Task>,
    buf: &mut [u8],
    entries: &mut Vec<Entry>,
    names: &mut Vec<u8>,
) {
    entries.clear();
    names.clear();

    let fd = unsafe {
        libc::open(
            task.path.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | O_NOFOLLOW_ANY | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        mark_error(shared, task.id);
        return;
    }

    let mut al = attrlist();
    let (mut files, mut dirs, mut bytes, mut errors) = (0u64, 0u64, 0u64, 0u64);
    let mut subdirs: Vec<usize> = Vec::new();

    loop {
        let n = unsafe {
            getattrlistbulk(
                fd,
                &mut al,
                buf.as_mut_ptr() as *mut libc::c_void,
                buf.len(),
                FSOPT_NOFOLLOW | FSOPT_PACK_INVAL_ATTRS,
            )
        };
        if n < 0 {
            let err = io::Error::last_os_error();
            if err.raw_os_error() == Some(libc::EINTR) {
                continue;
            }
            errors += 1;
            break;
        }
        if n == 0 || shared.cancel.load(Ordering::Relaxed) {
            break;
        }

        let mut pos = 0usize;
        for _ in 0..n {
            let len = u32_at(buf, pos) as usize;
            let rec = &buf[pos..pos + len];
            pos += len;
            if len < RECORD_MIN {
                errors += 1;
                continue;
            }

            let name_off = i32_at(rec, OFF_NAME) as isize;
            let name_len = u32_at(rec, OFF_NAME + 4) as usize;
            let start = (OFF_NAME as isize + name_off) as usize;
            let mut name = &rec[start..start + name_len];
            while let Some((&0, rest)) = name.split_last() {
                name = rest;
            }

            let entry_err = u32_at(rec, OFF_ERROR);
            let objtype = u32_at(rec, OFF_OBJTYPE);
            let devid = i32_at(rec, OFF_DEVID) as u64;
            let mtime = i64_at(rec, OFF_MODTIME);
            let fileid = u64_at(rec, OFF_FILEID);
            let mut cur = OFF_TAIL;
            let mut mountstatus = 0u32;
            let mut nlink = 1u32;
            let mut allocsize = 0u64;
            if u32_at(rec, OFF_RET_DIRATTR) & ATTR_DIR_MOUNTSTATUS != 0 && cur + 4 <= len {
                mountstatus = u32_at(rec, cur);
                cur += 4;
            }
            let ret_file = u32_at(rec, OFF_RET_FILEATTR);
            if ret_file & ATTR_FILE_LINKCOUNT != 0 && cur + 4 <= len {
                nlink = u32_at(rec, cur);
                cur += 4;
            }
            if ret_file & ATTR_FILE_ALLOCSIZE != 0 && cur + 8 <= len {
                allocsize = i64_at(rec, cur).max(0) as u64;
            }

            let mut flags = Flags::empty();
            let mut size = allocsize;
            if entry_err != 0 {
                flags |= Flags::ERROR;
                errors += 1;
            }
            match objtype {
                VDIR => {
                    flags |= Flags::DIR;
                    size = 0;
                    dirs += 1;
                    if mountstatus & DIR_MNTSTATUS_MNTPOINT != 0 {
                        flags |= Flags::MOUNT;
                        // Never descend by default. On a system/data volume
                        // group the Data mount's content is already reachable
                        // through firmlinks (/Users, /Applications, ...), so
                        // descending into it would double count.
                        if shared.cross_mounts {
                            subdirs.push(entries.len());
                        }
                    } else if entry_err == 0 {
                        subdirs.push(entries.len());
                    }
                }
                VLNK => {
                    flags |= Flags::SYMLINK;
                    files += 1;
                }
                VREG => {
                    files += 1;
                    if nlink > 1 && shared.links.is_dup(devid, fileid) {
                        flags |= Flags::HARDLINK_DUP;
                        size = 0;
                    }
                }
                _ => files += 1,
            }
            bytes += size;

            let top = match task.top {
                Top::Root => {
                    let top = shared.progress.register_top(name);
                    shared.progress.add_top(top, size);
                    top
                }
                Top::Child(top) => top,
            };
            entries.push(Entry {
                name_off: names.len() as u32,
                name_len: name.len().min(u16::MAX as usize) as u16,
                size,
                mtime,
                flags,
                top,
            });
            names.extend_from_slice(name);
        }
    }
    unsafe { libc::close(fd) };
    if let Top::Child(top) = task.top {
        shared.progress.add_top(top, bytes);
    }

    // Commit the directory as one contiguous block.
    let first_id = {
        let mut arena = shared.arena.lock().unwrap();
        let first = arena.len() as NodeId;
        for e in entries.iter() {
            let off = e.name_off as usize;
            let name = &names[off..off + e.name_len as usize];
            arena.push(task.id, name, e.size, e.mtime, e.flags);
        }
        first
    };
    shared.progress.add(files, dirs, bytes, errors);

    if !subdirs.is_empty() {
        let parent = task.path.as_bytes();
        for &idx in &subdirs {
            let e = &entries[idx];
            let off = e.name_off as usize;
            let name = &names[off..off + e.name_len as usize];
            let mut p = Vec::with_capacity(parent.len() + 1 + name.len());
            p.extend_from_slice(parent);
            p.push(b'/');
            p.extend_from_slice(name);
            let Ok(path) = CString::new(p) else { continue };
            shared.pending.fetch_add(1, Ordering::AcqRel);
            local.push(Task { id: first_id + idx as NodeId, path, top: Top::Child(e.top) });
        }
    }
}

fn set_user_initiated_qos() {
    const QOS_CLASS_USER_INITIATED: u32 = 0x19;
    extern "C" {
        fn pthread_set_qos_class_self_np(qos_class: u32, relative_priority: libc::c_int) -> libc::c_int;
    }
    unsafe {
        pthread_set_qos_class_self_np(QOS_CLASS_USER_INITIATED, 0);
    }
}

/// Stop the kernel from downloading iCloud / File Provider placeholders when we
/// enumerate them. Safe to call once per process; applies to every walker.
pub fn disable_dataless_materialization() {
    const IOPOL_TYPE_VFS_MATERIALIZE_DATALESS_FILES: libc::c_int = 3;
    const IOPOL_SCOPE_PROCESS: libc::c_int = 0;
    const IOPOL_MATERIALIZE_DATALESS_FILES_OFF: libc::c_int = 1;
    extern "C" {
        fn setiopolicy_np(iotype: libc::c_int, scope: libc::c_int, policy: libc::c_int) -> libc::c_int;
    }
    unsafe {
        setiopolicy_np(
            IOPOL_TYPE_VFS_MATERIALIZE_DATALESS_FILES,
            IOPOL_SCOPE_PROCESS,
            IOPOL_MATERIALIZE_DATALESS_FILES_OFF,
        );
    }
}

/// Raise the soft fd limit so deep queues of open directories do not hit EMFILE.
pub fn raise_fd_limit() {
    unsafe {
        let mut rl = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
        if libc::getrlimit(libc::RLIMIT_NOFILE, &mut rl) == 0 {
            let want = 10240.min(rl.rlim_max);
            if rl.rlim_cur < want {
                rl.rlim_cur = want;
                libc::setrlimit(libc::RLIMIT_NOFILE, &rl);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    fn write(path: &Path, bytes: usize) {
        fs::write(path, vec![0xA5u8; bytes]).unwrap();
    }

    #[test]
    fn attributes_bytes_to_the_roots_direct_children() {
        // The walker opens with O_NOFOLLOW_ANY, and /var is a symlink, so resolve the temp dir first.
        let root = fs::canonicalize(std::env::temp_dir()).unwrap().join(format!("unshift-tops-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("a")).unwrap();
        fs::create_dir_all(root.join("b/deep")).unwrap();
        write(&root.join("a/x"), 8192);
        write(&root.join("b/y"), 8192);
        write(&root.join("b/deep/z"), 8192);
        write(&root.join("top-file"), 8192);

        let cfg = ScanConfig { root: root.clone(), threads: 2, cross_mounts: false, qos: false };
        let progress = Progress::default();
        let arena = walk(&cfg, &progress, &Cancel::new(false)).unwrap();

        let mut tops = progress.tops();
        tops.sort();
        let total: u64 = tops.iter().map(|(_, b)| b).sum::<u64>() + progress.rest();
        assert_eq!(tops.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(), vec!["a", "b", "top-file"]);
        assert!(tops[0].1 >= 8192, "a holds one 8 KiB file, got {}", tops[0].1);
        assert_eq!(tops[1].1, 2 * tops[0].1); // b holds two files of the same size
        assert_eq!(tops[2].1, tops[0].1); // the root-level file counts as its own child
        assert_eq!(total, arena.size[0]);
        assert_eq!(progress.rest(), 0);

        fs::remove_dir_all(&root).unwrap();
    }
}
