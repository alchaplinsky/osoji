//! Reference walker B: jwalk with per-entry lstat, the same approach dua-cli uses.

use crate::{Arena, Cancel, Flags, HardlinkSet, Progress, ScanConfig, NONE};
use jwalk::{Parallelism, WalkDirGeneric};
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::sync::atomic::Ordering;

#[derive(Clone, Copy, Debug, Default)]
pub struct Meta {
    ok: bool,
    blocks: u64,
    nlink: u32,
    ino: u64,
    dev: u64,
    mtime: i64,
    mount: bool,
}

/// Entries folded into the local counters before they are pushed to `Progress`, so the shared
/// atomics are touched once per batch rather than once per entry.
const BATCH: u32 = 512;

/// Counters accumulated between flushes. `top_bytes` belongs to the top-level child the walk is
/// currently inside, so it must be flushed before the walk moves on to the next one.
#[derive(Default)]
struct Batch {
    files: u64,
    dirs: u64,
    bytes: u64,
    errors: u64,
    top_bytes: u64,
    entries: u32,
}

impl Batch {
    fn flush(&mut self, progress: &Progress, top: u32) {
        progress.add(self.files, self.dirs, self.bytes, self.errors);
        progress.add_top(top, self.top_bytes);
        *self = Self::default();
    }
}

pub fn walk(cfg: &ScanConfig, progress: &Progress, cancel: &Cancel, links: &HardlinkSet) -> io::Result<Arena> {
    let root_md = std::fs::symlink_metadata(&cfg.root)?;
    let root_dev = root_md.dev();
    let cross = cfg.cross_mounts;

    let walker = WalkDirGeneric::<((), Meta)>::new(&cfg.root)
        .follow_links(false)
        .skip_hidden(false)
        .parallelism(Parallelism::RayonNewPool(cfg.threads))
        .process_read_dir(move |_depth, _path, _state, children| {
            for child in children.iter_mut() {
                let Ok(entry) = child else { continue };
                match entry.metadata() {
                    Ok(md) => {
                        let mount = entry.file_type().is_dir() && md.dev() != root_dev;
                        if mount && !cross {
                            entry.read_children_path = None;
                        }
                        entry.client_state = Meta {
                            ok: true,
                            blocks: md.blocks(),
                            nlink: md.nlink() as u32,
                            ino: md.ino(),
                            dev: md.dev(),
                            mtime: md.mtime(),
                            mount,
                        };
                    }
                    Err(_) => entry.client_state = Meta::default(),
                }
            }
        });

    let mut arena = Arena::with_capacity(1 << 20, 16 << 20);
    let mut stack: Vec<u32> = Vec::new();
    let mut batch = Batch::default();
    // The walk is depth-first, so every entry after a direct child of the root sits under that
    // child until the next depth-1 entry comes along.
    let mut top = Progress::REST;

    for entry in walker {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let entry = match entry {
            Ok(e) => e,
            Err(_) => {
                batch.errors += 1;
                continue;
            }
        };
        let depth = entry.depth();
        stack.truncate(depth);
        let parent = if depth == 0 { NONE } else { stack[depth - 1] };
        if depth == 1 {
            batch.flush(progress, top);
            top = progress.register_top(entry.file_name().as_bytes());
        }

        let mut flags = Flags::empty();
        let meta = entry.client_state;
        let ft = entry.file_type();
        let mut size = meta.blocks * 512;
        let mut linked = false;

        if ft.is_dir() {
            flags |= Flags::DIR;
            // A directory's own allocation (4 KiB on ext4, 0 on APFS) is not content: the bulk
            // walker and `list_dir` record directories at 0, and aggregation fills in the total.
            size = 0;
            batch.dirs += 1;
            if meta.mount {
                flags |= Flags::MOUNT;
            }
            if entry.read_children_error.is_some() {
                flags |= Flags::ERROR;
                batch.errors += 1;
            }
        } else {
            batch.files += 1;
            if ft.is_symlink() {
                flags |= Flags::SYMLINK;
            } else if ft.is_file() && meta.nlink > 1 {
                linked = true;
                if links.is_dup(meta.dev, meta.ino) {
                    flags |= Flags::HARDLINK_DUP;
                    size = 0;
                }
            }
        }
        if depth > 0 && !meta.ok {
            flags |= Flags::ERROR;
            batch.errors += 1;
            size = 0;
        }
        if depth == 0 {
            size = 0;
        }
        batch.bytes += size;
        batch.top_bytes += size;

        let name: &[u8] = if depth == 0 { cfg.root.as_os_str().as_bytes() } else { entry.file_name().as_bytes() };
        let id = arena.push(parent, name, size, meta.mtime, flags);
        arena.dev[id as usize] = meta.dev as u32;
        arena.ino[id as usize] = meta.ino;
        if linked {
            links.record(meta.dev, meta.ino, id, !flags.contains(Flags::HARDLINK_DUP));
        }
        if ft.is_dir() {
            stack.push(id);
        }

        batch.entries += 1;
        if batch.entries >= BATCH {
            batch.flush(progress, top);
        }
    }

    batch.flush(progress, top);
    arena.aggregate();
    Ok(arena)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};

    fn write(path: &Path, bytes: usize) {
        fs::write(path, vec![0xA5u8; bytes]).unwrap();
    }

    /// A fresh, empty directory under the temp dir, resolved so the root name is a real path.
    fn temp_root(tag: &str) -> PathBuf {
        let root = fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!("osoji-jwalk-{tag}-{}", std::process::id()));
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

        // The live counters end where the arena does: four files, four dirs including the root.
        assert_eq!(progress.files.load(Ordering::Relaxed), 4);
        assert_eq!(progress.dirs.load(Ordering::Relaxed), 4);
        assert_eq!(progress.bytes.load(Ordering::Relaxed), arena.size[0]);
        assert_eq!(progress.errors.load(Ordering::Relaxed), 0);

        fs::remove_dir_all(&root).unwrap();
    }
}
