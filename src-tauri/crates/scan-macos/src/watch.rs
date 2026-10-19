//! FSEvents watcher that reports which directories changed after a scan, so they can be
//! re-listed instead of rescanned.

use notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_full::{new_debouncer_opt, DebounceEventResult, DebouncedEvent, Debouncer, NoCache};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::time::Duration;

const DEBOUNCE: Duration = Duration::from_millis(500);
/// How long `start` waits for the probe event before giving up on the guarantee.
const READY_TIMEOUT: Duration = Duration::from_secs(10);

/// One debounced window of changes, every path inside the watched root.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Batch {
    /// Directories whose listing must be re-read. Sorted, deduplicated.
    pub dirs: Vec<PathBuf>,
    /// Subtrees FSEvents lost track of (coalesced or dropped events) that must be walked
    /// again. Sorted, with subtrees of other entries removed.
    pub rescan: Vec<PathBuf>,
}

/// Watches a root recursively until dropped.
pub struct Watcher {
    debouncer: Option<Debouncer<RecommendedWatcher, NoCache>>,
    /// Private directory in the same FSEvents stream as the root, written once to observe
    /// when the stream is live. Its events never reach `on_batch`.
    probe: PathBuf,
}

impl Watcher {
    /// Returns once FSEvents is delivering for `root`: any change made after this returns is
    /// reported. That is observed through the debouncer, so this blocks for about one debounce
    /// window (500 ms). If readiness is not seen within 10 s the watcher is returned anyway.
    ///
    /// `on_batch` runs on the debouncer's thread. Do not drop the watcher from inside it:
    /// drop joins that thread.
    pub fn start(root: &Path, on_batch: impl Fn(Batch) + Send + 'static) -> io::Result<Watcher> {
        Self::start_in(root, &std::env::temp_dir(), on_batch)
    }

    /// `start` with the probe directory created under `probe_parent`; tests use it to put the
    /// probe inside the watched root.
    fn start_in(root: &Path, probe_parent: &Path, on_batch: impl Fn(Batch) + Send + 'static) -> io::Result<Watcher> {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        // FSEvents reports real paths (/private/var, not /var); anything else would never match.
        let root = fs::canonicalize(root)?;
        let probe = fs::canonicalize(probe_parent)?.join(format!(
            "osoji-watch-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        // A leftover from a crashed process that had the same pid.
        let _ = fs::remove_dir_all(&probe);
        fs::create_dir(&probe)?;
        // Built before the debouncer so an error below still removes the probe.
        let mut w = Watcher { debouncer: None, probe: probe.clone() };

        let (ready_tx, ready_rx) = mpsc::channel();
        let handler_root = root.clone();
        let handler = move |res: DebounceEventResult| match res {
            Ok(events) => {
                if events.iter().flat_map(|e| &e.paths).any(|p| p.starts_with(&probe)) {
                    // Fails once start has stopped listening, which is fine.
                    let _ = ready_tx.send(());
                }
                let batch = batch(&handler_root, &probe, &events);
                if !batch.dirs.is_empty() || !batch.rescan.is_empty() {
                    on_batch(batch);
                }
            }
            Err(errors) => {
                for e in errors {
                    eprintln!("watch {}: {e}", handler_root.display());
                }
            }
        };
        // NoCache: the file-id cache would stat the whole tree up front just to stitch renames
        // together, and renames need no stitching here since both parents are reported anyway.
        let mut debouncer =
            new_debouncer_opt(DEBOUNCE, None, handler, NoCache, notify::Config::default()).map_err(io::Error::other)?;
        debouncer.watch(&root, RecursiveMode::Recursive).map_err(io::Error::other)?;
        // notify rebuilds a single stream covering both paths. fseventsd makes a stream effective
        // asynchronously but delivers in log order, so once the marker's event arrives, any
        // later change under the root is reported too.
        debouncer.watch(&w.probe, RecursiveMode::Recursive).map_err(io::Error::other)?;
        w.debouncer = Some(debouncer);
        fs::write(w.probe.join("marker"), b"")?;
        if ready_rx.recv_timeout(READY_TIMEOUT).is_err() {
            eprintln!("watch {}: FSEvents not confirmed live after {READY_TIMEOUT:?}", root.display());
        }
        Ok(w)
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        // stop() joins the debouncer thread, so on_batch never runs after drop returns.
        if let Some(d) = self.debouncer.take() {
            d.stop();
        }
        let _ = fs::remove_dir_all(&self.probe);
    }
}

/// `probe` is excluded explicitly because it may sit inside `root` (root `/` or home).
fn batch(root: &Path, probe: &Path, events: &[DebouncedEvent]) -> Batch {
    let mut b = Batch::default();
    for ev in events {
        let paths = ev.paths.iter().filter(|p| !p.starts_with(probe));
        if ev.need_rescan() {
            if ev.paths.is_empty() {
                b.rescan.push(root.to_path_buf());
            }
            for p in paths {
                if p.starts_with(root) {
                    b.rescan.push(p.clone());
                } else if root.starts_with(p) {
                    b.rescan.push(root.to_path_buf());
                }
            }
            continue;
        }
        // Any change to p, including either side of a rename, changes its parent's listing.
        for p in paths.filter(|p| p.starts_with(root)) {
            let dir = if p == root { p } else { p.parent().unwrap_or(root) };
            b.dirs.push(dir.to_path_buf());
        }
    }
    b.dirs.sort();
    b.dirs.dedup();
    // Path ordering is by component, so a subtree sorts directly after its root.
    b.rescan.sort();
    b.rescan.dedup_by(|later, earlier| later.starts_with(earlier));
    b
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{temp_root, write};
    use notify::event::{CreateKind, EventKind, Flag, ModifyKind, RenameMode};
    use notify::Event;
    use std::sync::mpsc::{channel, Receiver, RecvTimeoutError};
    use std::time::Instant;

    fn paths(ps: &[&str]) -> Vec<PathBuf> {
        ps.iter().map(PathBuf::from).collect()
    }

    fn debounced(mut e: Event, ps: &[&str]) -> DebouncedEvent {
        e.paths = paths(ps);
        DebouncedEvent::new(e, Instant::now())
    }

    fn ev(kind: EventKind, ps: &[&str]) -> DebouncedEvent {
        debounced(Event::new(kind), ps)
    }

    fn rescan(ps: &[&str]) -> DebouncedEvent {
        debounced(Event::new(EventKind::Other).set_flag(Flag::Rescan), ps)
    }

    #[test]
    fn maps_events_to_parent_dirs_inside_the_root() {
        let events = [
            ev(EventKind::Create(CreateKind::File), &["/r/a/f"]),
            ev(EventKind::Modify(ModifyKind::Any), &["/r/a/g"]),
            ev(EventKind::Modify(ModifyKind::Name(RenameMode::Both)), &["/r/a/x", "/r/b/c/x"]),
            ev(EventKind::Modify(ModifyKind::Any), &["/r"]),
            ev(EventKind::Create(CreateKind::File), &["/elsewhere/f"]),
            ev(EventKind::Create(CreateKind::File), &["/rr/f"]),
        ];
        let b = in_r(&events);
        assert_eq!(b.dirs, paths(&["/r", "/r/a", "/r/b/c"]));
        assert!(b.rescan.is_empty());
    }

    #[test]
    fn rescans_are_clipped_to_the_root_and_collapsed() {
        let b = in_r(&[rescan(&["/r/a/b"]), rescan(&["/r/a"]), rescan(&["/r/c"]), rescan(&["/x"])]);
        assert_eq!(b.rescan, paths(&["/r/a", "/r/c"]));
        assert!(b.dirs.is_empty());

        assert_eq!(in_r(&[rescan(&["/"]), rescan(&["/r/a"])]).rescan, paths(&["/r"]));
        assert_eq!(in_r(&[rescan(&[])]).rescan, paths(&["/r"]));
    }

    /// Batches events for root /r with the probe outside it.
    fn in_r(events: &[DebouncedEvent]) -> Batch {
        batch(Path::new("/r"), Path::new("/probe"), events)
    }

    #[test]
    fn probe_paths_inside_the_root_are_dropped() {
        let events = [
            ev(EventKind::Create(CreateKind::File), &["/r/p/marker"]),
            ev(EventKind::Modify(ModifyKind::Any), &["/r/p"]),
            ev(EventKind::Modify(ModifyKind::Name(RenameMode::Both)), &["/r/p/x", "/r/a/x"]),
            rescan(&["/r/p"]),
            ev(EventKind::Create(CreateKind::File), &["/r/pp/f"]),
        ];
        let b = batch(Path::new("/r"), Path::new("/r/p"), &events);
        assert_eq!(b.dirs, paths(&["/r/a", "/r/pp"]));
        assert!(b.rescan.is_empty());
    }

    // FSEvents latency is usually 1-2 s on top of the debounce window.
    const WAIT: Duration = Duration::from_secs(15);

    /// Checks every batch path is inside `root` and outside the probe.
    fn check(b: &Batch, root: &Path, probe: &Path) {
        for p in b.dirs.iter().chain(&b.rescan) {
            assert!(p.starts_with(root), "{} outside {}", p.display(), root.display());
            assert!(!p.starts_with(probe), "probe path {} reported", p.display());
        }
    }

    /// Collects dirs from batches until `want` are all seen, failing after WAIT.
    fn await_dirs(rx: &Receiver<Batch>, w: &Watcher, root: &Path, want: &[&Path]) {
        let deadline = Instant::now() + WAIT;
        let mut seen: Vec<PathBuf> = Vec::new();
        while !want.iter().all(|w| seen.iter().any(|s| s == w)) {
            let left = deadline.saturating_duration_since(Instant::now());
            match rx.recv_timeout(left) {
                Ok(b) => {
                    check(&b, root, &w.probe);
                    seen.extend(b.dirs);
                }
                Err(e) => panic!("{e:?}: wanted {want:?}, saw {seen:?}"),
            }
        }
    }

    fn start_in(root: &Path, probe_parent: &Path) -> (Watcher, Receiver<Batch>) {
        let (tx, rx) = channel();
        let t = Instant::now();
        let w = Watcher::start_in(root, probe_parent, move |b| {
            let _ = tx.send(b);
        })
        .unwrap();
        eprintln!("start took {:?}", t.elapsed());
        assert!(t.elapsed() < READY_TIMEOUT, "probe event never arrived");
        (w, rx)
    }

    fn start(root: &Path) -> (Watcher, Receiver<Batch>) {
        start_in(root, &std::env::temp_dir())
    }

    // Each test writes immediately after start returns: that is the readiness guarantee.

    #[test]
    fn reports_parents_of_changed_paths() {
        let root = temp_root("changes");
        let (w, rx) = start(&root);
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
        await_dirs(&rx, &w, &root, &[&root, &sub, &deep]);

        // Let stragglers from the first round land so the second is judged on its own events.
        while rx.recv_timeout(Duration::from_millis(1500)).is_ok() {}
        // The debouncer folds a removed directory's child events into its own Remove, which
        // is enough: re-listing the root drops the whole subtree.
        fs::remove_dir_all(&sub).unwrap();
        await_dirs(&rx, &w, &root, &[&root]);

        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn probe_inside_the_root_is_never_reported() {
        let root = temp_root("probe-in-root");
        let (w, rx) = start_in(&root, &root);
        assert!(w.probe.starts_with(&root));
        write(&root.join("f"), 10);
        await_dirs(&rx, &w, &root, &[&root]);
        // The marker's batch precedes the readiness signal, so anything still queued is checked
        // too.
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
        await_dirs(&rx, &second, &root, &[&root]);

        // The first watcher's callback, and with it the sender, is gone once anything it
        // delivered before the drop is drained.
        let end = loop {
            if let Err(e) = first_rx.recv_timeout(Duration::from_millis(100)) {
                break e;
            }
        };
        assert_eq!(end, RecvTimeoutError::Disconnected);

        fs::remove_dir_all(&root).unwrap();
    }
}
