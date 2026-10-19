//! FSEvents watcher that reports which directories changed after a scan, so they can be
//! re-listed instead of rescanned.

use notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_full::{new_debouncer_opt, DebounceEventResult, DebouncedEvent, Debouncer, NoCache};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

const DEBOUNCE: Duration = Duration::from_millis(500);

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
}

impl Watcher {
    /// `on_batch` runs on the debouncer's thread. Do not drop the watcher from inside it:
    /// drop joins that thread.
    pub fn start(root: &Path, on_batch: impl Fn(Batch) + Send + 'static) -> io::Result<Watcher> {
        // FSEvents reports real paths (/private/var, not /var); anything else would never match.
        let root = fs::canonicalize(root)?;
        let handler_root = root.clone();
        let handler = move |res: DebounceEventResult| match res {
            Ok(events) => {
                let batch = batch(&handler_root, &events);
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
        Ok(Watcher { debouncer: Some(debouncer) })
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        // stop() joins the debouncer thread, so on_batch never runs after drop returns.
        if let Some(d) = self.debouncer.take() {
            d.stop();
        }
    }
}

fn batch(root: &Path, events: &[DebouncedEvent]) -> Batch {
    let mut b = Batch::default();
    for ev in events {
        if ev.need_rescan() {
            if ev.paths.is_empty() {
                b.rescan.push(root.to_path_buf());
            }
            for p in &ev.paths {
                if p.starts_with(root) {
                    b.rescan.push(p.clone());
                } else if root.starts_with(p) {
                    b.rescan.push(root.to_path_buf());
                }
            }
            continue;
        }
        // Any change to p, including either side of a rename, changes its parent's listing.
        for p in ev.paths.iter().filter(|p| p.starts_with(root)) {
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
        let b = batch(Path::new("/r"), &events);
        assert_eq!(b.dirs, paths(&["/r", "/r/a", "/r/b/c"]));
        assert!(b.rescan.is_empty());
    }

    #[test]
    fn rescans_are_clipped_to_the_root_and_collapsed() {
        let b = batch(Path::new("/r"), &[rescan(&["/r/a/b"]), rescan(&["/r/a"]), rescan(&["/r/c"]), rescan(&["/x"])]);
        assert_eq!(b.rescan, paths(&["/r/a", "/r/c"]));
        assert!(b.dirs.is_empty());

        assert_eq!(batch(Path::new("/r"), &[rescan(&["/"]), rescan(&["/r/a"])]).rescan, paths(&["/r"]));
        assert_eq!(batch(Path::new("/r"), &[rescan(&[])]).rescan, paths(&["/r"]));
    }

    // FSEvents latency is usually 1-2 s on top of the debounce window.
    const WAIT: Duration = Duration::from_secs(15);

    /// Collects dirs from batches until `want` are all seen, failing after WAIT.
    fn await_dirs(rx: &Receiver<Batch>, root: &Path, want: &[&Path]) {
        let deadline = Instant::now() + WAIT;
        let mut seen: Vec<PathBuf> = Vec::new();
        while !want.iter().all(|w| seen.iter().any(|s| s == w)) {
            let left = deadline.saturating_duration_since(Instant::now());
            match rx.recv_timeout(left) {
                Ok(b) => {
                    for p in b.dirs.iter().chain(&b.rescan) {
                        assert!(p.starts_with(root), "{} outside {}", p.display(), root.display());
                    }
                    seen.extend(b.dirs);
                }
                Err(e) => panic!("{e:?}: wanted {want:?}, saw {seen:?}"),
            }
        }
    }

    fn start(root: &Path) -> (Watcher, Receiver<Batch>) {
        let (tx, rx) = channel();
        let w = Watcher::start(root, move |b| {
            let _ = tx.send(b);
        })
        .unwrap();
        (w, rx)
    }

    #[test]
    fn reports_parents_of_changed_paths() {
        let root = temp_root("watch");
        let (_w, rx) = start(&root);
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
        await_dirs(&rx, &root, &[&root, &sub, &deep]);

        // Let stragglers from the first round land so the second is judged on its own events.
        while rx.recv_timeout(Duration::from_millis(1500)).is_ok() {}
        // The debouncer folds a removed directory's child events into its own Remove, which
        // is enough: re-listing the root drops the whole subtree.
        fs::remove_dir_all(&sub).unwrap();
        await_dirs(&rx, &root, &[&root]);

        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn drop_is_prompt_and_the_root_can_be_watched_again() {
        let root = temp_root("rewatch");
        let (first, first_rx) = start(&root);
        let t = Instant::now();
        drop(first);
        assert!(t.elapsed() < Duration::from_secs(2), "drop took {:?}", t.elapsed());

        let (_second, rx) = start(&root);
        write(&root.join("f"), 10);
        await_dirs(&rx, &root, &[&root]);

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
