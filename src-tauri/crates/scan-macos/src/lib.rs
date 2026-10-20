//! Walker A: getattrlistbulk with a work-stealing deque (milestone M2).
//!
//! Each worker pops a directory task, opens it, drains getattrlistbulk into a
//! thread-local entry list, then appends the whole directory as one contiguous
//! block to the shared arena under a single lock acquisition.

mod watch;

pub use scan_core::live::Batch;
pub use scan_core::raise_fd_limit;
pub use watch::Watcher;

use crossbeam_deque::{Injector, Stealer, Worker};
use scan_core::live::DirEntry;
use scan_core::{Arena, Cancel, Flags, HardlinkSet, NodeId, Progress, ScanConfig, NONE};
use std::ffi::{CStr, CString};
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
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
    dev: u32,
    ino: u64,
    /// (dev, ino) of a regular file with nlink > 1, recorded once its id is known.
    link: Option<(u64, u64)>,
}

struct Shared<'a> {
    arena: Mutex<Arena>,
    links: &'a HardlinkSet,
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

/// One decoded getattrlistbulk record; `name` borrows the read buffer.
struct Record<'a> {
    name: &'a [u8],
    err: u32,
    objtype: u32,
    devid: u64,
    mtime: i64,
    fileid: u64,
    mountstatus: u32,
    nlink: u32,
    allocsize: u64,
}

/// `None` for a record too short to carry a name.
fn parse_record(rec: &[u8]) -> Option<Record<'_>> {
    let len = rec.len();
    if len < RECORD_MIN {
        return None;
    }
    let name_off = i32_at(rec, OFF_NAME) as isize;
    let name_len = u32_at(rec, OFF_NAME + 4) as usize;
    let start = (OFF_NAME as isize + name_off) as usize;
    let mut name = &rec[start..start + name_len];
    while let Some((&0, rest)) = name.split_last() {
        name = rest;
    }

    let mut r = Record {
        name,
        err: u32_at(rec, OFF_ERROR),
        objtype: u32_at(rec, OFF_OBJTYPE),
        devid: i32_at(rec, OFF_DEVID) as u64,
        mtime: i64_at(rec, OFF_MODTIME),
        fileid: u64_at(rec, OFF_FILEID),
        mountstatus: 0,
        nlink: 1,
        allocsize: 0,
    };
    let mut cur = OFF_TAIL;
    if u32_at(rec, OFF_RET_DIRATTR) & ATTR_DIR_MOUNTSTATUS != 0 && cur + 4 <= len {
        r.mountstatus = u32_at(rec, cur);
        cur += 4;
    }
    let ret_file = u32_at(rec, OFF_RET_FILEATTR);
    if ret_file & ATTR_FILE_LINKCOUNT != 0 && cur + 4 <= len {
        r.nlink = u32_at(rec, cur);
        cur += 4;
    }
    if ret_file & ATTR_FILE_ALLOCSIZE != 0 && cur + 8 <= len {
        r.allocsize = i64_at(rec, cur).max(0) as u64;
    }
    Some(r)
}

/// The `n` length-prefixed records getattrlistbulk left in `buf`.
fn records(buf: &[u8], n: usize) -> impl Iterator<Item = &[u8]> {
    let mut pos = 0usize;
    (0..n).map(move |_| {
        let len = u32_at(buf, pos) as usize;
        let rec = &buf[pos..pos + len];
        pos += len;
        rec
    })
}

/// One getattrlistbulk call, retried on EINTR. `Ok(0)` means the directory is drained.
fn read_bulk(fd: libc::c_int, al: &mut AttrList, buf: &mut [u8]) -> io::Result<usize> {
    loop {
        let n = unsafe {
            getattrlistbulk(
                fd,
                al,
                buf.as_mut_ptr() as *mut libc::c_void,
                buf.len(),
                FSOPT_NOFOLLOW | FSOPT_PACK_INVAL_ATTRS,
            )
        };
        if n >= 0 {
            return Ok(n as usize);
        }
        let err = io::Error::last_os_error();
        if err.raw_os_error() != Some(libc::EINTR) {
            return Err(err);
        }
    }
}

fn open_dir(path: &CStr) -> libc::c_int {
    unsafe { libc::open(path.as_ptr(), libc::O_RDONLY | libc::O_DIRECTORY | O_NOFOLLOW_ANY | libc::O_CLOEXEC) }
}

/// `links` collects the hard-link registry for the scan; pass a fresh set per walk.
pub fn walk(cfg: &ScanConfig, progress: &Progress, cancel: &Cancel, links: &HardlinkSet) -> io::Result<Arena> {
    let mut arena = Arena::with_capacity(1 << 20, 16 << 20);
    let root = arena.push(NONE, cfg.root.as_os_str().as_bytes(), 0, 0, Flags::DIR);
    let root_path = CString::new(cfg.root.as_os_str().as_bytes()).map_err(|_| io::Error::other("NUL in path"))?;

    let workers: Vec<Worker<Task>> = (0..cfg.threads).map(|_| Worker::new_lifo()).collect();
    let shared = Shared {
        arena: Mutex::new(arena),
        links,
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

    let fd = open_dir(&task.path);
    if fd < 0 {
        mark_error(shared, task.id);
        return;
    }

    let mut al = attrlist();
    let (mut files, mut dirs, mut bytes, mut errors) = (0u64, 0u64, 0u64, 0u64);
    let mut subdirs: Vec<usize> = Vec::new();

    loop {
        let n = match read_bulk(fd, &mut al, buf) {
            Ok(n) => n,
            Err(_) => {
                errors += 1;
                break;
            }
        };
        if n == 0 || shared.cancel.load(Ordering::Relaxed) {
            break;
        }

        for rec in records(buf, n) {
            let Some(Record { name, err: entry_err, objtype, devid, mtime, fileid, mountstatus, nlink, allocsize }) =
                parse_record(rec)
            else {
                errors += 1;
                continue;
            };

            let mut flags = Flags::empty();
            let mut size = allocsize;
            let mut link = None;
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
                    if nlink > 1 {
                        link = Some((devid, fileid));
                        if shared.links.is_dup(devid, fileid) {
                            flags |= Flags::HARDLINK_DUP;
                            size = 0;
                        }
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
                dev: devid as u32,
                ino: fileid,
                link,
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
            let id = arena.push(task.id, name, e.size, e.mtime, e.flags) as usize;
            arena.dev[id] = e.dev;
            arena.ino[id] = e.ino;
        }
        first
    };
    for (i, e) in entries.iter().enumerate() {
        if let Some((dev, ino)) = e.link {
            shared.links.record(dev, ino, first_id + i as NodeId, !e.flags.contains(Flags::HARDLINK_DUP));
        }
    }
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

/// Lists one directory the way the walker records it: allocated sizes, directories at 0,
/// symlinks not followed, mount points flagged `MOUNT`. Hard links are not deduplicated; a
/// live listing has no scan-wide view of which link was seen first, so each counts as an
/// ordinary file. Like the walker, fails if any component of `path` is a symlink, so pass
/// canonical paths.
pub fn list_dir(path: &Path) -> io::Result<Vec<DirEntry>> {
    let cpath = CString::new(path.as_os_str().as_bytes()).map_err(|_| io::Error::other("NUL in path"))?;
    let fd = open_dir(&cpath);
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    let fd = unsafe { OwnedFd::from_raw_fd(fd) };

    let mut al = attrlist();
    let mut buf = vec![0u8; BUF_SIZE];
    let mut out = Vec::new();
    // A failed read mid-directory is an error for the whole listing: reconciling against a
    // partial one would drop the entries it missed.
    while let n @ 1.. = read_bulk(fd.as_raw_fd(), &mut al, &mut buf)? {
        // A truncated record has no name to report, so it is skipped as the walker skips it.
        for r in records(&buf, n).filter_map(parse_record) {
            let mut flags = Flags::empty();
            let mut size = r.allocsize;
            match r.objtype {
                VDIR => {
                    flags |= Flags::DIR;
                    size = 0;
                    if r.mountstatus & DIR_MNTSTATUS_MNTPOINT != 0 {
                        flags |= Flags::MOUNT;
                    }
                }
                VLNK => flags |= Flags::SYMLINK,
                _ => {}
            }
            if r.err != 0 {
                flags |= Flags::ERROR;
                size = 0;
            }
            out.push(DirEntry {
                name: r.name.to_vec(),
                size,
                mtime: r.mtime,
                flags,
                dev: r.devid,
                ino: r.fileid,
                nlink: r.nlink,
            });
        }
    }
    Ok(out)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::PathBuf;

    pub(crate) fn write(path: &Path, bytes: usize) {
        fs::write(path, vec![0xA5u8; bytes]).unwrap();
    }

    /// A fresh, empty directory under the temp dir. The walker opens with O_NOFOLLOW_ANY and
    /// FSEvents reports real paths, and /var is a symlink, so the temp dir is resolved first.
    pub(crate) fn temp_root(tag: &str) -> PathBuf {
        let root = fs::canonicalize(std::env::temp_dir()).unwrap().join(format!("osoji-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn attributes_bytes_to_the_roots_direct_children() {
        let root = temp_root("tops");
        fs::create_dir_all(root.join("a")).unwrap();
        fs::create_dir_all(root.join("b/deep")).unwrap();
        write(&root.join("a/x"), 8192);
        write(&root.join("b/y"), 8192);
        write(&root.join("b/deep/z"), 8192);
        write(&root.join("top-file"), 8192);

        let cfg = ScanConfig { root: root.clone(), threads: 2, cross_mounts: false, qos: false };
        let progress = Progress::default();
        let arena = walk(&cfg, &progress, &Cancel::new(false), &HardlinkSet::default()).unwrap();

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

    #[test]
    fn list_dir_matches_the_walker() {
        let root = temp_root("list");
        fs::create_dir_all(root.join("sub")).unwrap();
        write(&root.join("sub/inner"), 4096);
        write(&root.join("small"), 10);
        write(&root.join("big"), 100_000);
        fs::hard_link(root.join("big"), root.join("big-link")).unwrap();
        std::os::unix::fs::symlink("sub", root.join("sym")).unwrap();

        let cfg = ScanConfig { root: root.clone(), threads: 2, cross_mounts: false, qos: false };
        let arena = walk(&cfg, &Progress::default(), &Cancel::new(false), &HardlinkSet::default()).unwrap();
        let walked: BTreeMap<Vec<u8>, (u64, i64, Flags)> = arena
            .children(0)
            .map(|id| {
                let i = id as usize;
                // Directory sizes in the arena are aggregated; the walker records them as 0 itself.
                let size = if arena.flags[i].contains(Flags::DIR) { 0 } else { arena.size[i] };
                (arena.name(id).to_vec(), (size, arena.mtime[i], arena.flags[i]))
            })
            .collect();
        let listed: BTreeMap<Vec<u8>, (u64, i64, Flags)> =
            list_dir(&root).unwrap().into_iter().map(|e| (e.name, (e.size, e.mtime, e.flags))).collect();

        assert_eq!(listed.keys().collect::<Vec<_>>(), walked.keys().collect::<Vec<_>>());
        for (name, w) in &walked {
            let l = listed[name];
            if w.2.contains(Flags::HARDLINK_DUP) {
                // The walker zeroes the second link it meets; a listing counts both in full.
                assert_eq!(l.0, listed[&b"big"[..]].0);
                assert_eq!(l.2, w.2 - Flags::HARDLINK_DUP);
            } else {
                assert_eq!(&l, w, "{}", String::from_utf8_lossy(name));
            }
        }
        let walked_ids: BTreeMap<Vec<u8>, (u32, u64)> = arena
            .children(0)
            .map(|id| (arena.name(id).to_vec(), (arena.dev[id as usize], arena.ino[id as usize])))
            .collect();
        // The arena keeps dev as u32, the listing as u64.
        let listed_ids: BTreeMap<Vec<u8>, (u32, u64)> =
            list_dir(&root).unwrap().into_iter().map(|e| (e.name, (e.dev as u32, e.ino))).collect();
        assert_eq!(listed_ids, walked_ids);
        assert!(walked_ids.values().all(|&(_, ino)| ino != 0));
        assert_eq!(walked_ids[&b"big"[..]], walked_ids[&b"big-link"[..]]);

        assert!(listed[&b"big"[..]].0 >= 100_000);
        assert!(listed[&b"sub"[..]].2.contains(Flags::DIR));
        assert!(listed[&b"sym"[..]].2.contains(Flags::SYMLINK));

        assert!(list_dir(&root.join("sym")).is_err(), "must not follow a symlinked directory path");
        assert!(list_dir(&root.join("missing")).is_err());

        fs::remove_dir_all(&root).unwrap();
    }
}
