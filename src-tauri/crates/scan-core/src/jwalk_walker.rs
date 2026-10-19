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
    let (mut files, mut dirs, mut bytes, mut errors) = (0u64, 0u64, 0u64, 0u64);

    for entry in walker {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let entry = match entry {
            Ok(e) => e,
            Err(_) => {
                errors += 1;
                continue;
            }
        };
        let depth = entry.depth();
        stack.truncate(depth);
        let parent = if depth == 0 { NONE } else { stack[depth - 1] };

        let mut flags = Flags::empty();
        let meta = entry.client_state;
        let ft = entry.file_type();
        let mut size = meta.blocks * 512;
        let mut linked = false;

        if ft.is_dir() {
            flags |= Flags::DIR;
            dirs += 1;
            if meta.mount {
                flags |= Flags::MOUNT;
            }
            if entry.read_children_error.is_some() {
                flags |= Flags::ERROR;
                errors += 1;
            }
        } else {
            files += 1;
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
            errors += 1;
            size = 0;
        }
        if depth == 0 {
            size = 0;
        }
        bytes += size;

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
    }

    progress.add(files, dirs, bytes, errors);
    arena.aggregate();
    Ok(arena)
}
