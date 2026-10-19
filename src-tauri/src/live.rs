//! Keeps a finished scan current as the disk changes, so the UI never needs a rescan.
//!
//! A batch is applied in phases, and the `scan` lock is never held across I/O: resolve paths to
//! ids (locked), list and walk (unlocked), reconcile and graft rescans (locked), walk new
//! directories (unlocked), graft them (locked). `generation` is read with the ids and checked at
//! every locked phase, because a finished scan replaces the arena and the ids mean nothing in the
//! new one. Batches arrive one at a time on the watcher's thread, so phases of two batches never
//! interleave.
//!
//! Not yet: nodes added here are not matched against the policy. `hits` and the policy flags
//! cover only what the scan itself found.

use crate::{find_node, AppState, Scan};
use scan_core::live::{is_gone, reconcile, DirEntry};
use scan_core::{Arena, Cancel, ChildIndex, Flags, NodeId, Progress, ScanConfig, NONE};
use scan_macos::{Batch, Watcher};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter, Manager};

/// Listened for in src/lib/api.ts.
const EVENT: &str = "scan:changed";

/// Re-walks are usually small subtrees; a few threads keep them off the user's foreground work.
const WALK_THREADS: usize = 2;

#[derive(Serialize, Clone)]
struct Changed {
    dirs: Vec<NodeId>,
}

/// Watch `root`, folding each batch into whatever scan `app` holds and telling the UI what changed.
pub(crate) fn watch(app: AppHandle, root: &Path) -> std::io::Result<Watcher> {
    Watcher::start(root, move |batch| {
        apply_batch(&app.state::<AppState>(), batch, |dirs| {
            if let Err(e) = app.emit(EVENT, Changed { dirs }) {
                eprintln!("live: emit {EVENT}: {e}");
            }
        });
    })
}

/// Fold one watcher batch into the scan. `emit` gets the directories whose contents changed,
/// and is not called when nothing did.
pub(crate) fn apply_batch(state: &AppState, batch: Batch, emit: impl Fn(Vec<NodeId>)) {
    if let Some(snapshot) = Snapshot::take(state, batch) {
        snapshot.apply(state, emit);
    }
}

/// What a batch refers to in the arena as it was when the batch arrived.
struct Snapshot {
    generation: u64,
    dirs: Vec<(NodeId, PathBuf)>,
    rescans: Vec<(NodeId, PathBuf)>,
}

impl Snapshot {
    fn take(state: &AppState, batch: Batch) -> Option<Snapshot> {
        let guard = state.scan.lock().unwrap();
        let scan = guard.as_ref()?;
        let generation = state.generation.load(Ordering::Acquire);
        // A rescan walks everything under it again; reconciling in there too would add twice.
        let covered = |p: &Path| batch.rescan.iter().any(|r| p.starts_with(r));
        // A brand-new directory is not in the arena yet; its parent's listing brings it in.
        let mut dirs: Vec<(NodeId, PathBuf)> = batch
            .dirs
            .iter()
            .filter(|p| !covered(p))
            .map(|p| (find_node(scan, p), p.clone()))
            .filter(|&(id, _)| id != NONE)
            .collect();
        let mut rescans = Vec::new();
        for path in batch.rescan {
            match find_node(scan, &path) {
                // A subtree new to the arena is a new entry in its nearest known ancestor, which a
                // listing turns into a full walk.
                NONE => dirs.extend(
                    path.ancestors().skip(1).map(|p| (find_node(scan, p), p.to_path_buf())).find(|&(id, _)| id != NONE),
                ),
                id => rescans.push((id, path)),
            }
        }
        dirs.sort_unstable_by_key(|&(id, _)| id);
        dirs.dedup_by_key(|&mut (id, _)| id);
        if dirs.is_empty() && rescans.is_empty() {
            return None;
        }
        Some(Snapshot { generation, dirs, rescans })
    }

    fn apply(self, state: &AppState, emit: impl Fn(Vec<NodeId>)) {
        // A partial listing would drop the entries it missed, so a failed one skips the directory.
        let listings: Vec<(NodeId, Vec<DirEntry>)> = self
            .dirs
            .into_iter()
            .filter_map(|(id, path)| match scan_macos::list_dir(&path) {
                Ok(listing) => Some((id, listing)),
                Err(e) => {
                    eprintln!("live: list {}: {e}", path.display());
                    None
                }
            })
            .collect();
        let walks: Vec<(NodeId, Arena)> =
            self.rescans.into_iter().filter_map(|(id, path)| walk(&path).map(|sub| (id, sub))).collect();

        let Some((mut changed, new_dirs)) = locked(state, self.generation, |scan| update(scan, &listings, &walks))
        else {
            return;
        };
        if changed.is_empty() {
            return;
        }

        let grafts: Vec<(NodeId, Arena)> =
            new_dirs.into_iter().filter_map(|(id, path)| walk(&path).map(|sub| (id, sub))).collect();
        let applied = locked(state, self.generation, |scan| {
            for (id, sub) in &grafts {
                // Swept or deleted since it was inserted; its walk would count under a detached node.
                if !is_gone(&scan.arena, *id) {
                    scan.arena.graft(sub, *id);
                }
            }
            // Inserts and grafts are not in the index, and every change reorders its ancestors' children.
            scan.index = ChildIndex::build(&scan.arena);
        });
        if applied.is_some() {
            changed.sort_unstable();
            changed.dedup();
            emit(changed);
        }
    }
}

/// Run `f` on the scan, unless it was replaced since `generation` was read.
fn locked<R>(state: &AppState, generation: u64, f: impl FnOnce(&mut Scan) -> R) -> Option<R> {
    let mut guard = state.scan.lock().unwrap();
    let scan = guard.as_mut()?;
    (state.generation.load(Ordering::Acquire) == generation).then(|| f(scan))
}

/// Reconcile each listing and replace each rescanned subtree. Returns the directories that changed
/// and the new directories still to walk, with their paths.
fn update(
    scan: &mut Scan,
    listings: &[(NodeId, Vec<DirEntry>)],
    walks: &[(NodeId, Arena)],
) -> (Vec<NodeId>, Vec<(NodeId, PathBuf)>) {
    let mut changed = Vec::new();
    let mut new_dirs = Vec::new();
    for (dir, listing) in listings {
        let r = reconcile(&mut scan.arena, &scan.index, *dir, listing);
        if r.is_empty() {
            continue;
        }
        changed.push(*dir);
        // The walker never descends into mount points or directories it cannot read either.
        let walkable =
            r.new_dirs.into_iter().filter(|&id| !scan.arena.flags[id as usize].intersects(Flags::MOUNT | Flags::ERROR));
        new_dirs.extend(walkable.map(|id| (id, scan.arena.path(id))));
    }
    // After the listings, so a subtree whose root a listing just removed is not grafted back.
    for (id, sub) in walks {
        if is_gone(&scan.arena, *id) {
            continue;
        }
        for &c in scan.index.children(*id) {
            scan.arena.mark_removed(c);
        }
        scan.arena.graft(sub, *id);
        changed.push(*id);
    }
    (changed, new_dirs)
}

/// Walk `path` into a fresh arena rooted at it, or None when its root cannot be read: grafting
/// that would empty a directory that is only unreadable.
fn walk(path: &Path) -> Option<Arena> {
    let cfg = ScanConfig { root: path.to_path_buf(), threads: WALK_THREADS, cross_mounts: false, qos: false };
    match scan_macos::walk(&cfg, &Progress::default(), &Cancel::new(false)) {
        Ok(sub) if !sub.flags[0].contains(Flags::ERROR) => Some(sub),
        Ok(_) => {
            eprintln!("live: can't read {}", path.display());
            None
        }
        Err(e) => {
            eprintln!("live: walk {}: {e}", path.display());
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scan_policy::PolicyHits;
    use std::cell::RefCell;
    use std::fs;

    fn temp_root(tag: &str) -> PathBuf {
        // Canonical: the walker refuses symlinked components and /var is one.
        let root =
            fs::canonicalize(std::env::temp_dir()).unwrap().join(format!("osoji-live-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("a/sub")).unwrap();
        write(&root.join("a/x"), 10_000);
        write(&root.join("a/y"), 5_000);
        write(&root.join("a/sub/z"), 8_192);
        write(&root.join("b"), 4_096);
        root
    }

    fn write(path: &Path, bytes: usize) {
        fs::write(path, vec![0xA5u8; bytes]).unwrap();
    }

    fn state_for(root: &Path) -> AppState {
        let arena = walk(root).unwrap();
        let index = ChildIndex::build(&arena);
        let state = AppState::default();
        *state.scan.lock().unwrap() =
            Some(Scan { root: root.to_path_buf(), arena, index, hits: PolicyHits::default(), swept: (0, 0) });
        state
    }

    /// Live nodes by path, so arenas with different id layouts compare equal.
    fn dump(arena: &Arena) -> Vec<(PathBuf, u64, u32)> {
        let mut out: Vec<_> = (0..arena.len() as NodeId)
            .filter(|&i| !is_gone(arena, i))
            .map(|i| (arena.path(i), arena.size[i as usize], arena.items[i as usize]))
            .collect();
        out.sort();
        out
    }

    fn current(state: &AppState) -> Vec<(PathBuf, u64, u32)> {
        dump(&state.scan.lock().unwrap().as_ref().unwrap().arena)
    }

    fn assert_fresh(state: &AppState, root: &Path) {
        assert_eq!(current(state), dump(&walk(root).unwrap()));
    }

    fn id_of(state: &AppState, path: &Path) -> NodeId {
        find_node(state.scan.lock().unwrap().as_ref().unwrap(), path)
    }

    /// Apply `batch` and return what was emitted.
    fn apply(state: &AppState, batch: Batch) -> Vec<Vec<NodeId>> {
        let emitted = RefCell::new(Vec::new());
        apply_batch(state, batch, |ids| emitted.borrow_mut().push(ids));
        emitted.into_inner()
    }

    fn dirs(paths: &[&Path]) -> Batch {
        Batch { dirs: paths.iter().map(|p| p.to_path_buf()).collect(), rescan: Vec::new() }
    }

    #[test]
    fn a_deleted_file_comes_off_its_parents_and_the_same_batch_again_does_nothing() {
        let root = temp_root("delete");
        let state = state_for(&root);
        let a = root.join("a");
        let a_id = id_of(&state, &a);

        fs::remove_file(a.join("x")).unwrap();
        let emitted = apply(&state, dirs(&[&a]));
        assert_eq!(emitted, vec![vec![a_id]]);
        assert_fresh(&state, &root);
        assert_eq!(id_of(&state, &a.join("x")), NONE);

        let after = current(&state);
        assert!(apply(&state, dirs(&[&a])).is_empty());
        assert_eq!(current(&state), after);
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_new_nested_directory_is_walked_and_grafted() {
        let root = temp_root("graft");
        let state = state_for(&root);
        let a = root.join("a");
        let a_id = id_of(&state, &a);

        fs::create_dir_all(a.join("new/deeper")).unwrap();
        write(&a.join("new/f"), 3_000);
        write(&a.join("new/deeper/g"), 20_000);
        let emitted = apply(&state, dirs(&[&a, &a.join("new"), &a.join("new/deeper")]));
        assert_eq!(emitted, vec![vec![a_id]]);
        assert_fresh(&state, &root);
        assert_ne!(id_of(&state, &a.join("new/deeper/g")), NONE, "index rebuilt over the graft");
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_rescan_replaces_the_subtree() {
        let root = temp_root("rescan");
        let state = state_for(&root);
        let sub = root.join("a/sub");
        let sub_id = id_of(&state, &sub);

        fs::remove_file(sub.join("z")).unwrap();
        write(&sub.join("w"), 7_000);
        fs::create_dir_all(sub.join("q")).unwrap();
        write(&sub.join("q/r"), 12_000);
        // The listing of `sub` itself is covered by the rescan and must not add its entries twice.
        let batch = Batch { dirs: vec![sub.clone()], rescan: vec![sub.clone()] };
        assert_eq!(apply(&state, batch), vec![vec![sub_id]]);
        assert_fresh(&state, &root);
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_rescan_of_a_subtree_new_to_the_scan_lists_its_parent() {
        let root = temp_root("rescan-new");
        let state = state_for(&root);
        let a_id = id_of(&state, &root.join("a"));

        fs::create_dir_all(root.join("a/fresh/inner")).unwrap();
        write(&root.join("a/fresh/inner/f"), 9_000);
        let batch = Batch { dirs: Vec::new(), rescan: vec![root.join("a/fresh/inner")] };
        assert_eq!(apply(&state, batch), vec![vec![a_id]]);
        assert_fresh(&state, &root);
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_batch_from_before_the_scan_was_replaced_applies_nothing() {
        let root = temp_root("stale");
        let state = state_for(&root);
        let a = root.join("a");
        let before = current(&state);

        fs::remove_file(a.join("x")).unwrap();
        write(&a.join("n"), 1_000);
        let snapshot = Snapshot::take(&state, dirs(&[&a])).unwrap();
        state.generation.fetch_add(1, Ordering::AcqRel);
        let emitted = RefCell::new(Vec::new());
        snapshot.apply(&state, |ids| emitted.borrow_mut().push(ids));

        assert!(emitted.into_inner().is_empty());
        assert_eq!(current(&state), before);
        fs::remove_dir_all(&root).unwrap();
    }
}
