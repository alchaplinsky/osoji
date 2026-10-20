//! inotify watcher that reports which directories changed after a scan, so they can be
//! re-listed instead of rescanned. Same contract as `scan_macos::Watcher`.
//!
//! Built on `notify`'s inotify backend: one watch per directory under the root, added for
//! directories created later as well, with a queue overflow flagged as a rescan. inotify has
//! no replay, so a change made before `start` added its watches is never reported; the app
//! starts the watcher before the walk, which covers that.
//!
//! Watches are a per-user kernel resource, `fs.inotify.max_user_watches` (8192 on older
//! kernels). When a tree needs more than are left, `start` fails with an error naming the
//! sysctl; the app logs it and the scan reports `live: false`. Raising it:
//! `sudo sysctl fs.inotify.max_user_watches=524288`.

use notify::event::{AccessKind, AccessMode, ModifyKind};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher as _};
use scan_core::live::Batch;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// How long `start` waits for the probe event before declaring the watcher dead.
const READY_TIMEOUT: Duration = Duration::from_secs(10);
/// inotify delivers one event at a time, and the app runs an index and policy pass per batch.
/// A burst is held until it has been quiet this long so it lands as one batch.
const QUIET: Duration = Duration::from_millis(100);
/// But never longer than this, so a steady stream of changes still gets through.
const MAX_HOLD: Duration = Duration::from_millis(500);

/// What notify's handler receives: an event, or an error from the backend.
type Item = notify::Result<Event>;

/// Watches a root recursively until dropped.
pub struct Watcher {
    root: PathBuf,
    /// Private directory in the same watcher as the root, written once to prove it delivers.
    /// Its events never reach `on_batch`.
    probe: PathBuf,
    inner: Option<RecommendedWatcher>,
    worker: Option<JoinHandle<()>>,
}

impl Watcher {
    /// Returns once the watcher has delivered a change made after it started, and reports
    /// every change under `root` made after this was called. Takes about one quiet window
    /// (0.1 s) on top of adding a watch per directory; fails with `TimedOut` if nothing is
    /// delivered within 10 s.
    ///
    /// `on_batch` runs on a dedicated thread. Do not drop the watcher from inside it: drop
    /// joins that thread.
    pub fn start(root: &Path, on_batch: impl Fn(Batch) + Send + 'static) -> io::Result<Watcher> {
        Self::start_in(root, &std::env::temp_dir(), on_batch)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// `start` with the probe directory under `probe_parent`; tests use it to put the probe
    /// inside the root.
    ///
    /// `inotify_add_watch` is synchronous, so once `watch` returns for the root every change
    /// under it is queued. The probe is watched by the same inotify instance, whose queue is
    /// ordered, so the marker's event arriving proves delivery works and everything under the
    /// root up to that point has been delivered before it.
    fn start_in(root: &Path, probe_parent: &Path, on_batch: impl Fn(Batch) + Send + 'static) -> io::Result<Watcher> {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        // Events are reported as the watched path joined with the entry name, so the root must
        // be the real path for batches to line up with the scan.
        let root = fs::canonicalize(root)?;
        let probe = fs::canonicalize(probe_parent)?.join(format!(
            "osoji-watch-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        // A leftover from a crashed process that had the same pid.
        let _ = fs::remove_dir_all(&probe);
        fs::create_dir(&probe)?;
        // Built first so any error below tears down whatever exists by then.
        let mut w = Watcher { root: root.clone(), probe: probe.clone(), inner: None, worker: None };

        let (tx, rx) = mpsc::channel::<Item>();
        let (ready_tx, ready_rx) = mpsc::channel();
        w.worker = Some(
            thread::Builder::new()
                .name("osoji-watch".into())
                .spawn(move || deliver(rx, &root, &probe, ready_tx, on_batch))?,
        );
        // The handler runs on notify's own thread; it only forwards, so it never blocks that
        // thread on on_batch. Symlinked directories are not followed, as the walkers do not.
        let inner = w.inner.insert(
            RecommendedWatcher::new(
                move |item| {
                    // Fails only while the watcher is being dropped.
                    let _ = tx.send(item);
                },
                notify::Config::default().with_follow_symlinks(false),
            )
            .map_err(io_error)?,
        );
        inner.watch(&w.root, RecursiveMode::Recursive).map_err(io_error)?;
        inner.watch(&w.probe, RecursiveMode::NonRecursive).map_err(io_error)?;
        fs::write(w.probe.join("marker"), b"")?;
        if ready_rx.recv_timeout(READY_TIMEOUT).is_err() {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!("inotify delivered nothing for {} within {READY_TIMEOUT:?}", w.root.display()),
            ));
        }
        Ok(w)
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        // Dropping the notify watcher stops its thread, which drops the handler and with it the
        // sender; that ends the worker's loop, so on_batch never runs after drop returns.
        self.inner.take();
        if let Some(t) = self.worker.take() {
            let _ = t.join();
        }
        let _ = fs::remove_dir_all(&self.probe);
    }
}

/// `notify`'s errors as `io::Error`, with the watch limit spelled out for the log.
fn io_error(e: notify::Error) -> io::Error {
    match e.kind {
        notify::ErrorKind::Io(e) => e,
        notify::ErrorKind::MaxFilesWatch => io::Error::other(format!(
            "inotify watch limit reached (fs.inotify.max_user_watches); raise it with \
             `sudo sysctl fs.inotify.max_user_watches=524288`: {e}"
        )),
        _ => io::Error::other(e.to_string()),
    }
}

/// Worker loop. Delivery is kept off notify's thread because on_batch may do I/O and take
/// locks, which would stall event reads and overflow the inotify queue.
fn deliver(rx: Receiver<Item>, root: &Path, probe: &Path, ready: Sender<()>, on_batch: impl Fn(Batch)) {
    let marker = probe.join("marker");
    while let Ok(first) = rx.recv() {
        let mut items = vec![first];
        // Hold the burst, and merge whatever queued up while on_batch ran.
        let t = Instant::now();
        while t.elapsed() < MAX_HOLD {
            // A disconnect ends the loop on the next recv; what is here is still delivered.
            match rx.recv_timeout(QUIET) {
                Ok(item) => items.push(item),
                Err(_) => break,
            }
        }
        if items.iter().any(|i| matches!(i, Ok(e) if e.paths.contains(&marker))) {
            // Fails once start has stopped listening, which is fine.
            let _ = ready.send(());
        }
        let b = batch(root, probe, &items);
        if !b.dirs.is_empty() || !b.rescan.is_empty() {
            on_batch(b);
        }
    }
}

/// `probe` is excluded explicitly because it may sit inside `root` (root `/` or `/tmp`).
fn batch(root: &Path, probe: &Path, items: &[Item]) -> Batch {
    let mut b = Batch::default();
    for item in items {
        let e = match item {
            Ok(e) => e,
            // The backend lost track of something; nothing below the root can be trusted.
            Err(_) => {
                b.rescan.push(root.to_path_buf());
                continue;
            }
        };
        if e.need_rescan() {
            // Queue overflow carries no path: the whole root.
            if e.paths.is_empty() {
                b.rescan.push(root.to_path_buf());
            }
            for p in e.paths.iter().filter(|p| !p.starts_with(probe)) {
                if p.starts_with(root) {
                    b.rescan.push(p.clone());
                } else if root.starts_with(p) {
                    b.rescan.push(root.to_path_buf());
                }
            }
            continue;
        }
        if !changes_listing(&e.kind) {
            continue;
        }
        for p in e.paths.iter().filter(|p| !p.starts_with(probe) && p.starts_with(root)) {
            if p == root {
                // The root itself was removed or renamed: the watches below it are stale.
                if matches!(e.kind, EventKind::Remove(_) | EventKind::Modify(ModifyKind::Name(_))) {
                    b.rescan.push(root.to_path_buf());
                } else {
                    b.dirs.push(root.to_path_buf());
                }
                continue;
            }
            // Any change to p, including either side of a rename, changes its parent's listing.
            b.dirs.push(p.parent().unwrap_or(root).to_path_buf());
            // Files written into a new directory before its watch was added would be missed.
            if matches!(e.kind, EventKind::Create(notify::event::CreateKind::Folder)) {
                b.rescan.push(p.clone());
            }
        }
    }
    b.normalize();
    b
}

/// Whether an event of this kind changes a directory listing: anything but a read.
fn changes_listing(kind: &EventKind) -> bool {
    match kind {
        EventKind::Any | EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => true,
        EventKind::Access(a) => matches!(a, AccessKind::Close(AccessMode::Write)),
        EventKind::Other => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::{CreateKind, Flag, ModifyKind, RemoveKind, RenameMode};

    fn paths(ps: &[&str]) -> Vec<PathBuf> {
        ps.iter().map(PathBuf::from).collect()
    }

    fn ev(kind: EventKind, ps: &[&str]) -> Item {
        let mut e = Event::new(kind);
        for p in ps {
            e = e.add_path(PathBuf::from(p));
        }
        Ok(e)
    }

    fn rescan(ps: &[&str]) -> Item {
        ev(EventKind::Other, ps).map(|e| e.set_flag(Flag::Rescan))
    }

    const CREATED: EventKind = EventKind::Create(CreateKind::File);
    const MKDIR: EventKind = EventKind::Create(CreateKind::Folder);
    const MODIFIED: EventKind = EventKind::Modify(ModifyKind::Data(notify::event::DataChange::Any));
    const ATTRIB: EventKind = EventKind::Modify(ModifyKind::Metadata(notify::event::MetadataKind::Any));
    const RENAMED_FROM: EventKind = EventKind::Modify(ModifyKind::Name(RenameMode::From));
    const RENAMED_BOTH: EventKind = EventKind::Modify(ModifyKind::Name(RenameMode::Both));
    const REMOVED: EventKind = EventKind::Remove(RemoveKind::Any);
    const CLOSED: EventKind = EventKind::Access(AccessKind::Close(AccessMode::Write));
    const OPENED: EventKind = EventKind::Access(AccessKind::Open(AccessMode::Any));

    /// Batches events for root /r with the probe outside it.
    fn in_r(items: &[Item]) -> Batch {
        batch(Path::new("/r"), Path::new("/probe"), items)
    }

    #[test]
    fn maps_events_to_parent_dirs_inside_the_root() {
        let b = in_r(&[
            ev(CREATED, &["/r/a/f"]),
            ev(MODIFIED, &["/r/a/g"]),
            ev(CLOSED, &["/r/a/g"]),
            ev(REMOVED, &["/r/a/h"]),
            // A rename within one watch comes as one event with both sides.
            ev(RENAMED_BOTH, &["/r/a/x", "/r/b/c/x"]),
            ev(RENAMED_FROM, &["/r/d/x"]),
            ev(ATTRIB, &["/r"]),
            ev(OPENED, &["/r/e/f"]),
            ev(CREATED, &["/elsewhere/f"]),
            ev(CREATED, &["/rr/f"]),
            ev(CREATED, &["/probe/marker"]),
        ]);
        assert_eq!(b.dirs, paths(&["/r", "/r/a", "/r/b/c", "/r/d"]));
        assert!(b.rescan.is_empty());
    }

    #[test]
    fn new_directories_are_rescanned() {
        let b = in_r(&[ev(MKDIR, &["/r/a/new"]), ev(CREATED, &["/r/a/new/f"])]);
        assert_eq!(b.dirs, paths(&["/r/a", "/r/a/new"]));
        assert_eq!(b.rescan, paths(&["/r/a/new"]));
    }

    #[test]
    fn rescans_are_clipped_to_the_root_and_collapsed() {
        let b = in_r(&[rescan(&["/r/a/b"]), rescan(&["/r/a"]), rescan(&["/r/c"]), rescan(&["/x"])]);
        assert_eq!(b.rescan, paths(&["/r/a", "/r/c"]));
        assert!(b.dirs.is_empty());

        // The overflow event carries no path; an error from the backend is as bad.
        assert_eq!(in_r(&[rescan(&[]), rescan(&["/r/a"])]).rescan, paths(&["/r"]));
        assert_eq!(in_r(&[rescan(&["/"]), rescan(&["/r/a"])]).rescan, paths(&["/r"]));
        assert_eq!(in_r(&[Err(notify::Error::generic("lost")), ev(CREATED, &["/r/a/f"])]).rescan, paths(&["/r"]));
    }

    #[test]
    fn root_changed_rescans_the_root() {
        let b = in_r(&[ev(REMOVED, &["/r"]), rescan(&["/r/a"])]);
        assert_eq!(b.rescan, paths(&["/r"]));
        assert!(b.dirs.is_empty());
        assert_eq!(in_r(&[ev(RENAMED_FROM, &["/r"])]).rescan, paths(&["/r"]));
        // The probe is a watched directory too; its moves are not the root's.
        assert_eq!(in_r(&[ev(REMOVED, &["/probe"])]), Batch::default());
    }

    #[test]
    fn probe_paths_inside_the_root_are_dropped() {
        let items = [
            ev(MKDIR, &["/r/p"]),
            ev(CREATED, &["/r/p/marker"]),
            ev(ATTRIB, &["/r/p"]),
            ev(RENAMED_BOTH, &["/r/p/x", "/r/a/x"]),
            rescan(&["/r/p"]),
            ev(REMOVED, &["/r/p"]),
            ev(CREATED, &["/r/pp/f"]),
        ];
        let b = batch(Path::new("/r"), Path::new("/r/p"), &items);
        assert_eq!(b.dirs, paths(&["/r/a", "/r/pp"]));
        // The only path of that rescan was the probe's, so nothing was lost under the root.
        assert!(b.rescan.is_empty());
    }

    // What follows needs inotify timing; the FSEvents backend used on a Mac is not under test.
    #[cfg(target_os = "linux")]
    mod live {
        use super::super::*;
        use std::sync::mpsc::{channel, RecvTimeoutError};

        const WAIT: Duration = Duration::from_secs(15);

        fn write(path: &Path, bytes: usize) {
            fs::write(path, vec![0xA5u8; bytes]).unwrap();
        }

        /// A fresh, empty directory under the temp dir, resolved first since watched paths are.
        fn temp_root(tag: &str) -> PathBuf {
            let root =
                fs::canonicalize(std::env::temp_dir()).unwrap().join(format!("osoji-{tag}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&root);
            fs::create_dir_all(&root).unwrap();
            root
        }

        /// Checks every batch path is inside `root` and outside the probe.
        fn check(b: &Batch, root: &Path, probe: &Path) {
            for p in b.dirs.iter().chain(&b.rescan) {
                assert!(p.starts_with(root), "{} outside {}", p.display(), root.display());
                assert!(!p.starts_with(probe), "probe path {} reported", p.display());
            }
        }

        /// Collects batches until every `want` is covered, failing after WAIT. A directory is
        /// covered by its own listing or by a rescan of a subtree holding it: a directory
        /// created and filled before notify adds its watch is reported only through the latter.
        fn await_dirs(rx: &Receiver<Batch>, w: &Watcher, want: &[&Path]) {
            let t = Instant::now();
            let deadline = t + WAIT;
            let mut seen = Batch::default();
            let covered = |want: &Path, seen: &Batch| {
                seen.dirs.iter().any(|s| s == want) || seen.rescan.iter().any(|r| want.starts_with(r))
            };
            while !want.iter().all(|p| covered(p, &seen)) {
                let left = deadline.saturating_duration_since(Instant::now());
                match rx.recv_timeout(left) {
                    Ok(b) => {
                        check(&b, &w.root, &w.probe);
                        seen.dirs.extend(b.dirs);
                        seen.rescan.extend(b.rescan);
                    }
                    Err(e) => panic!("{e:?}: wanted {want:?}, saw {seen:?}"),
                }
            }
            eprintln!("saw {want:?} after {:?}", t.elapsed());
        }

        fn start_with(root: &Path, probe_parent: &Path) -> (Watcher, Receiver<Batch>) {
            let (tx, rx) = channel();
            let t = Instant::now();
            let w = Watcher::start_in(root, probe_parent, move |b| {
                let _ = tx.send(b);
            })
            .unwrap();
            eprintln!("start took {:?}", t.elapsed());
            (w, rx)
        }

        fn start(root: &Path) -> (Watcher, Receiver<Batch>) {
            start_with(root, &std::env::temp_dir())
        }

        // Each test writes immediately after start returns: that is the readiness guarantee.
        // There is no `changes_before_the_stream_is_live_are_replayed` here: inotify has no
        // replay, so a change made before `start` is simply not reported.

        #[test]
        fn reports_parents_of_changed_paths() {
            let root = temp_root("changes");
            let (w, rx) = start(&root);
            assert_eq!(w.root(), root);
            let sub = root.join("sub");
            let deep = sub.join("deep");

            let f = root.join("f");
            write(&f, 100);
            std::thread::sleep(Duration::from_millis(100));
            write(&f, 200_000);
            std::thread::sleep(Duration::from_millis(100));
            fs::remove_file(&f).unwrap();
            fs::create_dir_all(&deep).unwrap();
            write(&deep.join("x"), 10);
            await_dirs(&rx, &w, &[&root, &sub, &deep]);

            // Let stragglers from the first round land so the second is judged on its own events.
            while rx.recv_timeout(Duration::from_millis(1500)).is_ok() {}
            // The removed directory itself is reported against the root, which is enough:
            // re-listing the root drops the whole subtree.
            fs::remove_dir_all(&sub).unwrap();
            await_dirs(&rx, &w, &[&root]);

            drop(w);
            fs::remove_dir_all(&root).unwrap();
        }

        #[test]
        fn a_burst_right_after_start_is_reported() {
            let root = temp_root("burst");
            let sub = root.join("sub");
            fs::create_dir(&sub).unwrap();
            let (w, rx) = start(&root);
            for i in 0..50 {
                write(&sub.join(format!("f{i}")), 10);
            }
            await_dirs(&rx, &w, &[&sub]);
            drop(w);
            fs::remove_dir_all(&root).unwrap();
        }

        #[test]
        fn probe_inside_the_root_is_never_reported() {
            let root = temp_root("probe-in-root");
            // A subdirectory, not the root: the probe's creation already changes the root's listing.
            let a = root.join("a");
            fs::create_dir(&a).unwrap();
            let (w, rx) = start_with(&root, &root);
            assert!(w.probe.starts_with(&root));
            write(&a.join("f"), 10);
            await_dirs(&rx, &w, &[&a]);
            // The probe's creation and marker events may still be queued, so drain and check those too.
            while let Ok(b) = rx.recv_timeout(Duration::from_millis(1500)) {
                check(&b, &root, &w.probe);
            }
            drop(w);
            fs::remove_dir_all(&root).unwrap();
        }

        #[test]
        fn drop_removes_the_probe() {
            let root = temp_root("probe-drop");
            let (w, _rx) = start(&root);
            let probe = w.probe.clone();
            assert!(probe.is_dir());
            drop(w);
            assert!(!probe.exists());
            fs::remove_dir_all(&root).unwrap();
        }

        #[test]
        fn drop_is_prompt_and_the_root_can_be_watched_again() {
            let root = temp_root("rewatch");
            let (first, first_rx) = start(&root);
            let t = Instant::now();
            drop(first);
            assert!(t.elapsed() < Duration::from_secs(2), "drop took {:?}", t.elapsed());

            let (second, rx) = start(&root);
            write(&root.join("f"), 10);
            await_dirs(&rx, &second, &[&root]);

            // The first watcher's on_batch, and with it the sender, is gone once anything it
            // delivered before the drop is drained.
            let end = loop {
                if let Err(e) = first_rx.recv_timeout(Duration::from_millis(100)) {
                    break e;
                }
            };
            assert_eq!(end, RecvTimeoutError::Disconnected);

            drop(second);
            fs::remove_dir_all(&root).unwrap();
        }
    }
}
