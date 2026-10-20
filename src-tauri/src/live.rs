//! Keeps a finished scan current as the disk changes, so the UI never needs a rescan.
//!
//! A batch is applied in phases, and the `scan` lock is never held across I/O: resolve paths to
//! ids (locked), list and walk (unlocked), reconcile and graft rescans (locked), walk new
//! directories (unlocked), graft them and match every node the batch touched against the policy
//! again (locked). Touched means added, resized, or an ancestor of a directory whose contents
//! changed: size and age rules test a directory's total, and those are the totals that moved.
//!
//! Two counters guard what was read before the lock was dropped. `generation` moves when a
//! finished scan replaces the arena: the ids mean nothing in the new one, and that scan's walk saw
//! the disk after the batch's events anyway, so the batch is dropped. `epoch` moves with every
//! change to the arena, sweeps included. A listing is diffed against the arena's children, so one
//! read before a sweep trashed a child still names it, and reconciling it would bring the child
//! back as a new live node. So when the epoch moved since the snapshot, nothing is applied and
//! the batch starts over from a fresh snapshot and fresh listings. Sweeps run one at a time and
//! bump the epoch once each, and applies are serialized, so this converges in a retry or two.
//! The second locked phase only grafts walks of directories the first one inserted: a graft under
//! a node swept or removed meanwhile is skipped, and no graft can bring back what a listing
//! dropped, so it checks `generation` alone; giving up there would leave those directories empty.
//!
//! Each locked phase leaves the child index covering every node it appended (`Scan::reindex`), so
//! anything that runs between phases, a sweep or a view, sees a complete index.
//!
//! A scan in progress queues batches in `AppState.pending` instead; `drain` applies them as one
//! merged batch once its result is in place, after the scan command has returned. Applies hold
//! `AppState.applying` throughout, so the watcher threads (two while a new scan replaces the old)
//! and the drain never interleave phases.

use crate::{ancestors_of, find_node, home_path, unix_now, AppState, Scan};
use scan_core::live::{graft, is_gone, reconcile, DirEntry, Hardlinks};
use scan_core::{Arena, Cancel, Flags, HardlinkSet, NodeId, Progress, ScanConfig, NONE};
use scan_macos::{Batch, Watcher};
use scan_policy::PolicyContext;
use serde::Serialize;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

/// Listened for in src/lib/api.ts.
const EVENT: &str = "scan:changed";

/// How long changed directories collect before one event carries them to the UI. System folders
/// churn every few seconds, and each event has the UI re-read everything it shows; this keeps
/// that to a steady, affordable pace without making a change feel late.
const EMIT_WINDOW: Duration = Duration::from_millis(1500);

/// Re-walks are usually small subtrees; a few threads keep them off the user's foreground work.
const WALK_THREADS: usize = 2;

/// Each retry follows a sweep that landed mid-batch; losing this many races in a row is not
/// converging, and the next batch for these directories lists them again anyway.
const ATTEMPTS: usize = 3;

#[derive(Serialize, Clone)]
struct Changed {
    dirs: Vec<NodeId>,
}

/// Watch `root`, folding each batch into whatever scan `app` holds and telling the UI what changed.
/// Blocks until FSEvents is live (about half a second).
pub(crate) fn watch(app: AppHandle, root: &Path) -> io::Result<Watcher> {
    Watcher::start(root, move |batch| on_batch(&app.state::<AppState>(), batch, notify(&app)))
}

/// Tells the UI which directories changed, coalescing everything reported within `EMIT_WINDOW`
/// of the first into one event.
pub(crate) fn notify(app: &AppHandle) -> impl Fn(Vec<NodeId>) + '_ {
    move |dirs| {
        if dirs.is_empty() {
            return;
        }
        let first = {
            let state = app.state::<AppState>();
            let mut pending = state.changed.lock().unwrap();
            let first = pending.is_empty();
            pending.extend(dirs);
            first
        };
        // The first report of a window arms its timer; later ones just join the set it will send.
        if first {
            let app = app.clone();
            std::thread::spawn(move || {
                std::thread::sleep(EMIT_WINDOW);
                let mut dirs = std::mem::take(&mut *app.state::<AppState>().changed.lock().unwrap());
                dirs.sort_unstable();
                dirs.dedup();
                if let Err(e) = app.emit(EVENT, Changed { dirs }) {
                    log::error!("live: emit {EVENT}: {e}");
                }
            });
        }
    }
}

/// Queue `batch` while a scan runs, otherwise fold it in now.
fn on_batch(state: &AppState, batch: Batch, emit: impl Fn(Vec<NodeId>)) {
    if let Some(queue) = state.pending.lock().unwrap().as_mut() {
        queue.push(batch);
        return;
    }
    apply_batch(state, batch, emit);
}

/// Stop queueing and apply what was queued as one batch: every listing sees the disk as it is
/// now, so one apply does what applying each in turn would, for one index and policy pass instead
/// of one per window of the walk. Batches arriving meanwhile apply directly and may overtake the
/// queue, which is harmless for the same reason.
pub(crate) fn drain(state: &AppState, emit: impl Fn(Vec<NodeId>)) {
    let queued = state.pending.lock().unwrap().take().unwrap_or_default();
    if queued.is_empty() {
        return;
    }
    apply_batch(state, Batch::merge(queued), emit);
}

/// Fold one watcher batch into the scan. `emit` gets the directories whose contents changed,
/// and is not called when nothing did.
pub(crate) fn apply_batch(state: &AppState, batch: Batch, emit: impl Fn(Vec<NodeId>)) {
    let _serial = state.applying.lock().unwrap();
    for _ in 0..ATTEMPTS {
        let Some(snapshot) = Snapshot::take(state, &batch) else {
            return;
        };
        let reads = snapshot.read();
        if snapshot.commit(state, reads, &emit).is_ok() {
            return;
        }
    }
    log::warn!("live: the scan kept changing under a batch; dropped it after {ATTEMPTS} attempts");
}

/// What a batch refers to in the arena as it was when the batch arrived.
struct Snapshot {
    generation: u64,
    epoch: u64,
    dirs: Vec<(NodeId, PathBuf)>,
    rescans: Vec<(NodeId, PathBuf)>,
}

/// The arena changed since the snapshot, so what was read against it may be stale.
struct Moved;

/// What reading a directory found on disk.
enum Read<T> {
    Found(T),
    /// Deleted, renamed away, or replaced by something that is not a directory.
    Gone,
}

/// A walked subtree and the hard links found in it.
type Tree = (Arena, Hardlinks);

/// The unlocked phase's results for one snapshot.
struct Reads {
    listings: Vec<(NodeId, Read<Vec<DirEntry>>)>,
    walks: Vec<(NodeId, Read<Tree>)>,
}

impl Snapshot {
    fn take(state: &AppState, batch: &Batch) -> Option<Snapshot> {
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
        for path in &batch.rescan {
            match find_node(scan, path) {
                // A subtree new to the arena is a new entry in its nearest known ancestor, which a
                // listing turns into a full walk.
                NONE => dirs.extend(
                    path.ancestors().skip(1).map(|p| (find_node(scan, p), p.to_path_buf())).find(|&(id, _)| id != NONE),
                ),
                id => rescans.push((id, path.clone())),
            }
        }
        // The scan could not open these (no Full Disk Access, usually) and holds nothing under
        // them, so every change inside would fail the same way again. Protected folders such as a
        // browser profile churn constantly; the next full scan picks them up once access is granted.
        let readable = |&(id, _): &(NodeId, PathBuf)| !scan.arena.flags[id as usize].contains(Flags::ERROR);
        dirs.retain(readable);
        rescans.retain(readable);
        dirs.sort_unstable_by_key(|&(id, _)| id);
        dirs.dedup_by_key(|&mut (id, _)| id);
        if dirs.is_empty() && rescans.is_empty() {
            return None;
        }
        Some(Snapshot { generation, epoch: scan.epoch, dirs, rescans })
    }

    fn read(&self) -> Reads {
        Reads {
            listings: self.dirs.iter().filter_map(|(id, path)| list(path).map(|l| (*id, l))).collect(),
            walks: self.rescans.iter().filter_map(|(id, path)| rescan(path).map(|w| (*id, w))).collect(),
        }
    }

    /// Apply `reads`, or `Moved` without touching the arena when it changed since the snapshot.
    /// A scan replaced meanwhile makes the batch obsolete, which is not an error.
    fn commit(self, state: &AppState, reads: Reads, emit: impl Fn(Vec<NodeId>)) -> Result<(), Moved> {
        let epoch = self.epoch;
        let first =
            locked(
                state,
                self.generation,
                |scan| if scan.epoch == epoch { Ok(update(scan, reads)) } else { Err(Moved) },
            );
        let Some(first) = first else {
            return Ok(());
        };
        let Update { first_new, mut changed, mut touched, new_dirs } = first?;
        if changed.is_empty() {
            return Ok(());
        }

        let grafts: Vec<(NodeId, Tree)> =
            new_dirs.into_iter().filter_map(|(id, path)| walk(&path).map(|tree| (id, tree))).collect();
        let ctx = PolicyContext { home: home_path(), now: unix_now() };
        let applied = locked(state, self.generation, |scan| {
            for (id, (mut sub, sub_links)) in grafts {
                // Swept or deleted since it was inserted; its walk would count under a detached node.
                if !is_gone(&scan.arena, id) {
                    graft(&mut scan.arena, &mut scan.links, &mut sub, sub_links, id);
                }
            }
            scan.epoch += 1;
            // Grafted-into directories descend from reconciled ones, whose chains are in `touched`.
            scan.reindex(touched.iter().copied());
            touched.extend(first_new..scan.arena.len() as NodeId);
            // Against the extended index, so a pinned path that came back matches its new node.
            scan.restamp(&state.policy, &ctx, touched);
        });
        if applied.is_some() {
            changed.sort_unstable();
            changed.dedup();
            emit(changed);
        }
        Ok(())
    }
}

/// Run `f` on the scan, unless it was replaced since `generation` was read.
fn locked<R>(state: &AppState, generation: u64, f: impl FnOnce(&mut Scan) -> R) -> Option<R> {
    let mut guard = state.scan.lock().unwrap();
    let scan = guard.as_mut()?;
    (state.generation.load(Ordering::Acquire) == generation).then(|| f(scan))
}

/// What the first locked phase did.
struct Update {
    /// The arena's length before: every id from here on was added.
    first_new: NodeId,
    /// Directories whose contents changed.
    changed: Vec<NodeId>,
    /// Existing nodes whose size or total moved, for the policy to match again.
    touched: Vec<NodeId>,
    /// New directories still to walk, with their paths.
    new_dirs: Vec<(NodeId, PathBuf)>,
}

/// Reconcile each listing and replace each rescanned subtree.
fn update(scan: &mut Scan, reads: Reads) -> Update {
    let first_new = scan.arena.len() as NodeId;
    let mut changed = Vec::new();
    let mut touched = Vec::new();
    let mut new_dirs = Vec::new();
    for (dir, listing) in reads.listings {
        let Read::Found(listing) = listing else {
            changed.extend(vanish(scan, dir));
            continue;
        };
        let r = reconcile(&mut scan.arena, &scan.index, &mut scan.links, dir, &listing);
        // At once, so a later listing's detach walks a subtree this one added to.
        scan.index.extend(&scan.arena);
        if r.is_empty() {
            continue;
        }
        changed.push(dir);
        touched.extend(r.changed);
        // The walker never descends into mount points or directories it cannot read either.
        let walkable =
            r.new_dirs.into_iter().filter(|&id| !scan.arena.flags[id as usize].intersects(Flags::MOUNT | Flags::ERROR));
        new_dirs.extend(walkable.map(|id| (id, scan.arena.path(id))));
    }
    // After the listings, so a subtree whose root a listing just removed is not grafted back.
    for (id, walked) in reads.walks {
        if is_gone(&scan.arena, id) {
            continue;
        }
        let Read::Found((mut sub, sub_links)) = walked else {
            changed.extend(vanish(scan, id));
            continue;
        };
        // An empty listing removes every child the way reconcile does, so their hard links hand over.
        reconcile(&mut scan.arena, &scan.index, &mut scan.links, id, &[]);
        graft(&mut scan.arena, &mut scan.links, &mut sub, sub_links, id);
        scan.index.extend(&scan.arena);
        changed.push(id);
    }
    if !changed.is_empty() {
        scan.epoch += 1;
    }
    for &dir in &changed {
        touched.push(dir);
        touched.extend(ancestors_of(&scan.arena, dir));
    }
    touched.sort_unstable();
    touched.dedup();
    // Every change moved the totals along these chains, which is what orders their children.
    scan.reindex(touched.iter().copied());
    Update { first_new, changed, touched, new_dirs }
}

/// `dir` is no longer on disk: detach it, or for the root (the scan's anchor) everything under
/// it. Returns the directory whose contents changed, if anything did.
fn vanish(scan: &mut Scan, dir: NodeId) -> Option<NodeId> {
    if is_gone(&scan.arena, dir) {
        return None;
    }
    // Children go through reconcile so a hard link outside takes over the bytes of one inside.
    let r = reconcile(&mut scan.arena, &scan.index, &mut scan.links, dir, &[]);
    if dir == 0 {
        return (!r.is_empty()).then_some(0);
    }
    scan.arena.mark_removed(dir);
    Some(scan.arena.parent[dir as usize])
}

/// None when the directory is there but cannot be listed: a partial listing would drop the
/// entries it missed.
fn list(path: &Path) -> Option<Read<Vec<DirEntry>>> {
    match scan_macos::list_dir(path) {
        Ok(listing) => Some(Read::Found(listing)),
        Err(e) if vanished(&e) => Some(Read::Gone),
        Err(e) => {
            log::warn!("live: list {}: {e}", path.display());
            None
        }
    }
}

fn rescan(path: &Path) -> Option<Read<Tree>> {
    if let Some(tree) = walk(path) {
        return Some(Read::Found(tree));
    }
    // The walker flags a root it cannot open rather than failing; the path says whether it is gone.
    match std::fs::symlink_metadata(path) {
        Err(e) if vanished(&e) => Some(Read::Gone),
        Ok(meta) if !meta.is_dir() => Some(Read::Gone),
        _ => None,
    }
}

fn vanished(e: &io::Error) -> bool {
    matches!(e.kind(), io::ErrorKind::NotFound | io::ErrorKind::NotADirectory)
}

/// Walk `path` into a fresh arena rooted at it, or None when its root cannot be read: grafting
/// that would empty a directory that is only unreadable.
fn walk(path: &Path) -> Option<Tree> {
    let cfg = ScanConfig { root: path.to_path_buf(), threads: WALK_THREADS, cross_mounts: false, qos: false };
    let links = HardlinkSet::default();
    match scan_macos::walk(&cfg, &Progress::default(), &Cancel::new(false), &links) {
        Ok(sub) if !sub.flags[0].contains(Flags::ERROR) => Some((sub, links.finish())),
        Ok(_) => {
            log::warn!("live: can't read {}", path.display());
            None
        }
        Err(e) => {
            log::warn!("live: walk {}: {e}", path.display());
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{finish_sweep, plan_sweep};
    use scan_core::ChildIndex;
    use scan_policy::Policy;
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

    /// A finished scan of `root`, stamped as `start_scan` stamps it.
    fn scan_of(root: &Path, policy: &Policy) -> Scan {
        let (mut arena, links) = walk(root).unwrap();
        let index = ChildIndex::build(&arena);
        let ctx = PolicyContext { home: home_path(), now: unix_now() };
        let hits = policy.resolve(&arena, &index, &ctx).stamp(&mut arena);
        Scan { root: root.to_path_buf(), arena, index, hits, links, epoch: 0 }
    }

    fn state_with(root: &Path, policy: Policy) -> AppState {
        let state = AppState { policy, ..AppState::default() };
        *state.scan.lock().unwrap() = Some(scan_of(root, &state.policy));
        state
    }

    fn state_for(root: &Path) -> AppState {
        state_with(root, Policy::default())
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

    fn with_scan<R>(state: &AppState, f: impl FnOnce(&Scan) -> R) -> R {
        f(state.scan.lock().unwrap().as_ref().unwrap())
    }

    fn current(state: &AppState) -> Vec<(PathBuf, u64, u32)> {
        with_scan(state, |scan| dump(&scan.arena))
    }

    fn assert_fresh(state: &AppState, root: &Path) {
        assert_eq!(current(state), dump(&walk(root).unwrap().0));
    }

    fn id_of(state: &AppState, path: &Path) -> NodeId {
        with_scan(state, |scan| find_node(scan, path))
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
    fn a_directory_the_scan_could_not_read_is_left_alone_until_a_rescan() {
        use std::os::unix::fs::PermissionsExt;
        let root = temp_root("unreadable");
        let locked = root.join("a/sub");
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
        let state = state_for(&root);
        let locked_id = id_of(&state, &locked);
        assert!(with_scan(&state, |scan| scan.arena.flags[locked_id as usize].contains(Flags::ERROR)));

        // Readable and changed by the time the batch arrives: still skipped, as the scan holds
        // nothing under it to reconcile against. The next scan brings it in.
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
        write(&locked.join("w"), 1_000);
        let before = current(&state);
        assert!(apply(&state, dirs(&[&locked])).is_empty());
        assert!(apply(&state, Batch { dirs: Vec::new(), rescan: vec![locked.clone()] }).is_empty());
        assert_eq!(current(&state), before);
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
        let snapshot = Snapshot::take(&state, &dirs(&[&a])).unwrap();
        state.generation.fetch_add(1, Ordering::AcqRel);
        let reads = snapshot.read();
        let emitted = RefCell::new(Vec::new());
        assert!(snapshot.commit(&state, reads, |ids| emitted.borrow_mut().push(ids)).is_ok());

        assert!(emitted.into_inner().is_empty());
        assert_eq!(current(&state), before);
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn batches_queued_during_a_scan_apply_to_the_scan_that_replaces_it() {
        let old_root = temp_root("queue-old");
        let root = temp_root("queue-new");
        let state = state_for(&old_root);
        let before = current(&state);
        *state.pending.lock().unwrap() = Some(Vec::new());

        // The new walk has already passed `a` when it changes; the old root changes too.
        let walked = scan_of(&root, &state.policy);
        fs::remove_file(root.join("a/x")).unwrap();
        write(&root.join("a/n"), 2_000);
        fs::remove_file(old_root.join("b")).unwrap();
        let emitted = RefCell::new(Vec::new());
        let emit = |ids: Vec<NodeId>| emitted.borrow_mut().push(ids);
        on_batch(&state, dirs(&[&root.join("a")]), emit);
        on_batch(&state, dirs(&[&old_root]), emit);
        assert!(emitted.borrow().is_empty());
        assert_eq!(current(&state), before, "nothing applies while queued");

        *state.scan.lock().unwrap() = Some(walked);
        state.generation.fetch_add(1, Ordering::AcqRel);
        drain(&state, emit);
        assert!(state.pending.lock().unwrap().is_none());
        assert_eq!(emitted.into_inner(), vec![vec![id_of(&state, &root.join("a"))]]);
        assert_fresh(&state, &root);
        fs::remove_dir_all(&old_root).unwrap();
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn queued_batches_drain_as_one_apply() {
        let root = temp_root("queue-merge");
        let state = state_for(&root);
        let (a, sub) = (root.join("a"), root.join("a/sub"));
        let (a_id, sub_id) = (id_of(&state, &a), id_of(&state, &sub));
        *state.pending.lock().unwrap() = Some(Vec::new());

        // Three windows of changes, two naming the same directory.
        fs::remove_file(a.join("x")).unwrap();
        write(&sub.join("w"), 7_000);
        fs::create_dir_all(a.join("new")).unwrap();
        write(&a.join("new/f"), 3_000);
        let emitted = RefCell::new(Vec::new());
        let emit = |ids: Vec<NodeId>| emitted.borrow_mut().push(ids);
        on_batch(&state, dirs(&[&a]), emit);
        on_batch(&state, dirs(&[&sub]), emit);
        on_batch(&state, dirs(&[&a, &a.join("new")]), emit);
        assert_eq!(state.pending.lock().unwrap().as_ref().map(Vec::len), Some(3));

        drain(&state, emit);
        let mut changed = vec![a_id, sub_id];
        changed.sort_unstable();
        assert_eq!(emitted.into_inner(), vec![changed], "one apply, one event for every changed directory");
        assert_fresh(&state, &root);
        assert_ne!(id_of(&state, &a.join("new/f")), NONE, "the new directory was walked and indexed");
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn batches_queued_during_a_cancelled_scan_apply_to_the_scan_that_stays() {
        let root = temp_root("queue-cancel");
        let state = state_for(&root);
        let a_id = id_of(&state, &root.join("a"));
        *state.pending.lock().unwrap() = Some(Vec::new());

        fs::remove_file(root.join("a/y")).unwrap();
        let emitted = RefCell::new(Vec::new());
        let emit = |ids: Vec<NodeId>| emitted.borrow_mut().push(ids);
        on_batch(&state, dirs(&[&root.join("a")]), emit);
        assert!(emitted.borrow().is_empty());

        drain(&state, emit);
        assert_eq!(emitted.into_inner(), vec![vec![a_id]]);
        assert_fresh(&state, &root);
        // No longer queueing: the next batch applies at once.
        write(&root.join("a/y"), 5_000);
        assert_eq!(apply(&state, dirs(&[&root.join("a")])), vec![vec![a_id]]);
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_listing_read_before_a_sweep_does_not_bring_the_swept_item_back() {
        let root = temp_root("swept");
        let state = state_for(&root);
        let a = root.join("a");
        let x = a.join("x");
        let x_id = id_of(&state, &x);
        write(&a.join("n"), 1_000);
        let batch = dirs(&[&a]);

        let snapshot = Snapshot::take(&state, &batch).unwrap();
        let reads = snapshot.read();
        // The sweep lands between the listing, which still holds `x`, and the locked phase.
        fs::remove_file(&x).unwrap();
        let len = {
            let mut guard = state.scan.lock().unwrap();
            let scan = guard.as_mut().unwrap();
            let size = scan.arena.size[x_id as usize];
            finish_sweep(&state.policy, Some(&mut *scan), vec![(x_id, size, Ok(()))], Vec::new());
            scan.arena.len()
        };
        assert!(snapshot.commit(&state, reads, |_| unreachable!("stale listing applied")).is_err());
        assert_eq!(with_scan(&state, |scan| scan.arena.len()), len, "nothing inserted");

        // From the top, as apply_batch retries: a fresh listing without `x`.
        assert_eq!(apply(&state, batch), vec![vec![id_of(&state, &a)]]);
        assert_eq!(id_of(&state, &x), NONE);
        assert_fresh(&state, &root);
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_deleted_root_empties_the_scan() {
        for (tag, rescan) in [("root-gone", false), ("root-gone-rescan", true)] {
            let root = temp_root(tag);
            let state = state_for(&root);
            fs::remove_dir_all(&root).unwrap();
            // FSEvents may report it as a rescan of the root, which covers the listing.
            let batch =
                Batch { dirs: vec![root.clone()], rescan: if rescan { vec![root.clone()] } else { Vec::new() } };
            assert_eq!(apply(&state, batch), vec![vec![0]], "{tag}");
            with_scan(&state, |scan| {
                let a = &scan.arena;
                assert_eq!((a.size[0], a.items[0]), (0, 0), "{tag}");
                assert!((1..a.len() as NodeId).all(|i| is_gone(a, i)), "{tag}");
            });
            assert!(apply(&state, dirs(&[&root])).is_empty(), "{tag}: nothing left to remove");
        }
    }

    #[test]
    fn a_deleted_directory_named_alone_is_detached() {
        let root = temp_root("dir-gone");
        let state = state_for(&root);
        let a = root.join("a");
        let sub = a.join("sub");
        let (a_id, sub_id) = (id_of(&state, &a), id_of(&state, &sub));

        fs::remove_dir_all(&sub).unwrap();
        assert_eq!(apply(&state, dirs(&[&sub])), vec![vec![a_id]]);
        assert!(with_scan(&state, |scan| scan.arena.flags[sub_id as usize].contains(Flags::REMOVED)));
        assert_fresh(&state, &root);
        fs::remove_dir_all(&root).unwrap();
    }

    /// Protects anything named `*.secret` (rule 0) and the file `b` at the root (rule 1).
    fn policy_for(root: &Path) -> Policy {
        Policy::from_toml(&format!(
            r#"
            version = 1
            [[rule]]
            id = "secret"
            kind = "protected"
            reason = "A secret."
            match = {{ suffix = ".secret" }}
            [[rule]]
            id = "b"
            kind = "protected"
            reason = "Keep b."
            match = {{ path = "{}" }}
            "#,
            root.join("b").display()
        ))
        .unwrap()
    }

    #[test]
    fn nodes_added_live_are_matched_against_the_policy() {
        let root = temp_root("policy");
        let state = state_with(&root, policy_for(&root));
        let a = root.join("a");

        write(&a.join("k.secret"), 1_000);
        fs::create_dir_all(a.join("vault")).unwrap();
        write(&a.join("vault/deep.secret"), 2_000);
        apply(&state, dirs(&[&a, &a.join("vault")]));
        // One reconciled in, one grafted with its new directory.
        for path in [a.join("k.secret"), a.join("vault/deep.secret")] {
            let id = id_of(&state, &path);
            assert_ne!(id, NONE, "{}", path.display());
            with_scan(&state, |scan| {
                assert!(scan.arena.flags[id as usize].contains(Flags::PROTECTED), "{}", path.display());
                assert_eq!(scan.hits.rule(id), Some(0), "{}", path.display());
            });
        }
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_protected_path_recreated_after_deletion_is_protected_again() {
        let root = temp_root("policy-pinned");
        let state = state_with(&root, policy_for(&root));
        let b = root.join("b");
        let old = id_of(&state, &b);
        assert!(with_scan(&state, |scan| scan.arena.flags[old as usize].contains(Flags::PROTECTED)));

        fs::remove_file(&b).unwrap();
        apply(&state, dirs(&[&root]));
        write(&b, 4_096);
        apply(&state, dirs(&[&root]));
        let id = id_of(&state, &b);
        assert!(id != NONE && id != old);
        with_scan(&state, |scan| {
            assert!(scan.arena.flags[id as usize].contains(Flags::PROTECTED));
            assert_eq!(scan.hits.rule(id), Some(1));
            let (items, refused) = plan_sweep(&state.policy, scan, vec![id]).unwrap();
            assert!(items.is_empty());
            assert_eq!(refused, vec![format!("{}: protected. Keep b.", b.display())]);
        });
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_new_hard_link_to_a_scanned_file_counts_once() {
        let root = temp_root("hardlink");
        let state = state_for(&root);
        let a = root.join("a");
        let totals = |arena: &Arena| (arena.size[0], arena.items[0]);
        let fresh = || totals(&walk(&root).unwrap().0);
        let dup = |path: &Path| {
            let id = id_of(&state, path);
            with_scan(&state, |scan| scan.arena.flags[id as usize].contains(Flags::HARDLINK_DUP))
        };

        // One link reconciled in, one grafted with its new directory.
        fs::hard_link(a.join("x"), a.join("sub/x2")).unwrap();
        fs::create_dir_all(a.join("linked")).unwrap();
        fs::hard_link(a.join("y"), a.join("linked/y2")).unwrap();
        apply(&state, dirs(&[&a, &a.join("sub"), &a.join("linked")]));
        assert_eq!(with_scan(&state, |scan| totals(&scan.arena)), fresh());
        assert!(dup(&a.join("sub/x2")) && dup(&a.join("linked/y2")));

        // The counted link goes; the registry hands its bytes to the one left.
        fs::remove_file(a.join("x")).unwrap();
        apply(&state, dirs(&[&a]));
        assert_eq!(with_scan(&state, |scan| totals(&scan.arena)), fresh());
        assert!(!dup(&a.join("sub/x2")));
        fs::remove_dir_all(&root).unwrap();
    }

    /// Sweep `path` as `sweep` does once the move succeeded, with the size planned before it.
    fn sweep(state: &AppState, path: &Path) -> crate::TrashResult {
        let id = id_of(state, path);
        let mut guard = state.scan.lock().unwrap();
        let scan = guard.as_mut().unwrap();
        let size = scan.arena.size[id as usize];
        finish_sweep(&state.policy, Some(scan), vec![(id, size, Ok(()))], Vec::new())
    }

    #[test]
    fn sweeping_the_counted_link_hands_its_bytes_to_the_survivor() {
        let root = temp_root("sweep-link");
        fs::hard_link(root.join("a/x"), root.join("a/sub/x2")).unwrap();
        let state = state_for(&root);
        let flags = |path: &Path| {
            let id = id_of(&state, path);
            with_scan(&state, |scan| scan.arena.flags[id as usize])
        };
        // The walker picks whichever link it reaches first.
        let (counted, survivor) = if flags(&root.join("a/x")).contains(Flags::HARDLINK_DUP) {
            (root.join("a/sub/x2"), root.join("a/x"))
        } else {
            (root.join("a/x"), root.join("a/sub/x2"))
        };
        let total = with_scan(&state, |scan| scan.arena.size[0]);

        fs::remove_file(&counted).unwrap();
        let result = sweep(&state, &counted);
        assert_eq!(result.bytes, 0, "emptying the Trash frees nothing while a link survives");
        assert_eq!(with_scan(&state, |scan| scan.arena.size[0]), total);
        assert!(!flags(&survivor).contains(Flags::HARDLINK_DUP));
        assert_fresh(&state, &root);
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_sweep_a_live_update_saw_first_counts_the_planned_size() {
        let root = temp_root("sweep-seen");
        let state = state_for(&root);
        let x = root.join("a/x");
        let x_id = id_of(&state, &x);
        let size = with_scan(&state, |scan| scan.arena.size[x_id as usize]);

        // The move lands, and the watcher reports it before the sweep re-locks.
        fs::remove_file(&x).unwrap();
        apply(&state, dirs(&[&root.join("a")]));
        let total = with_scan(&state, |scan| scan.arena.size[0]);
        let result = {
            let mut guard = state.scan.lock().unwrap();
            finish_sweep(&state.policy, guard.as_mut(), vec![(x_id, size, Ok(()))], Vec::new())
        };
        assert_eq!(result.bytes, size);
        assert_eq!(with_scan(&state, |scan| scan.arena.size[0]), total, "not taken off the parents twice");
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_directory_total_crossing_a_size_rule_is_matched_again() {
        let root = temp_root("policy-size");
        let policy = Policy::from_toml(
            r#"
            version = 1
            [[rule]]
            id = "big"
            kind = "unexpected"
            reason = "Grew."
            match = { name = "sub", dir = true, min_bytes = 50000 }
            "#,
        )
        .unwrap();
        let state = state_with(&root, policy);
        let sub = root.join("a/sub");
        let sub_id = id_of(&state, &sub);
        let hit = || {
            with_scan(&state, |scan| {
                (scan.arena.flags[sub_id as usize].contains(Flags::UNEXPECTED), scan.hits.rule(sub_id))
            })
        };
        assert_eq!(hit(), (false, None));

        write(&sub.join("z"), 100_000);
        apply(&state, dirs(&[&sub]));
        assert_eq!(hit(), (true, Some(0)));

        write(&sub.join("z"), 8_192);
        apply(&state, dirs(&[&sub]));
        assert_eq!(hit(), (false, None));

        // A sweep moves the total too.
        write(&sub.join("z"), 100_000);
        apply(&state, dirs(&[&sub]));
        assert_eq!(hit(), (true, Some(0)));
        fs::remove_file(sub.join("z")).unwrap();
        sweep(&state, &sub.join("z"));
        assert_eq!(hit(), (false, None));
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_recreated_trash_folder_replaces_the_old_one() {
        let root = temp_root("trash-recreated");
        let trash = root.join(".Trash");
        fs::create_dir_all(&trash).unwrap();
        write(&trash.join("old"), 4_096);
        let policy = Policy::from_toml(&format!(
            r#"
            version = 1
            [[rule]]
            id = "trash"
            kind = "trash"
            reason = "The Trash."
            match = {{ path = "{}" }}
            "#,
            trash.display()
        ))
        .unwrap();
        let state = state_with(&root, policy);
        let trash_id = || with_scan(&state, |scan| crate::trash_of(&state.policy, scan).map(|t| t.id));
        let old = id_of(&state, &trash);
        assert_eq!(trash_id(), Some(old));

        fs::remove_dir_all(&trash).unwrap();
        apply(&state, dirs(&[&root]));
        assert_eq!(trash_id(), None, "the detached Trash is not reported");

        fs::create_dir_all(&trash).unwrap();
        write(&trash.join("new"), 4_096);
        apply(&state, dirs(&[&root, &trash]));
        let id = id_of(&state, &trash);
        assert!(id != NONE && id != old);
        assert_eq!(trash_id(), Some(id));
        fs::remove_dir_all(&root).unwrap();
    }
}
