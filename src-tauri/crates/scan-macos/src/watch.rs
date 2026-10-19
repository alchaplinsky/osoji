//! FSEvents watcher that reports which directories changed after a scan, so they can be
//! re-listed instead of rescanned.
//!
//! The stream is owned directly rather than through `notify`: notify's FsEventWatcher goes
//! silent after its first callback on GitHub's macOS runners (notify#937) and recreates the
//! stream at SinceNow whenever a watch is added, dropping events (notify#993).

use fsevent_sys as fse;
use std::ffi::{c_char, c_void, CStr, CString, OsStr};
use std::fs;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// fseventsd coalesces events for this long before calling back, so no userland debouncer.
const LATENCY: f64 = 0.5;
/// How long `start` waits for the probe event before declaring the stream dead.
const READY_TIMEOUT: Duration = Duration::from_secs(10);

/// One FSEvents callback's worth of events: path and flags, in event-id order.
type Events = Vec<(PathBuf, fse::FSEventStreamEventFlags)>;

/// One coalesced window of changes, every path inside the watched root.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Batch {
    /// Directories whose listing must be re-read. Sorted, deduplicated.
    pub dirs: Vec<PathBuf>,
    /// Subtrees FSEvents lost track of (coalesced or dropped events) that must be walked
    /// again. Sorted, with subtrees of other entries removed.
    pub rescan: Vec<PathBuf>,
}

impl Batch {
    /// One batch standing for all of `batches`: a listing read for the merged batch sees the disk as
    /// it is after every one of them, so applying it once is the same as applying each in turn.
    pub fn merge(batches: impl IntoIterator<Item = Batch>) -> Batch {
        let mut out = Batch::default();
        for b in batches {
            out.dirs.extend(b.dirs);
            out.rescan.extend(b.rescan);
        }
        out.normalize();
        out
    }

    fn normalize(&mut self) {
        self.dirs.sort();
        self.dirs.dedup();
        // Path ordering is by component, so a subtree sorts directly after its root.
        self.rescan.sort();
        self.rescan.dedup_by(|later, earlier| later.starts_with(earlier));
    }
}

/// Watches a root recursively until dropped.
pub struct Watcher {
    root: PathBuf,
    /// Private directory in the same stream as the root, written once to prove the stream
    /// delivers. Its events never reach `on_batch`.
    probe: PathBuf,
    stream: Option<Stream>,
    worker: Option<JoinHandle<()>>,
}

impl Watcher {
    /// Returns once the stream has delivered a change made after it started, and reports every
    /// change under `root` made after this was called (see `start_in` for why). Takes about one
    /// latency window (0.5 s); fails with `TimedOut` if nothing is delivered within 10 s.
    ///
    /// `on_batch` runs on a dedicated thread. Do not drop the watcher from inside it: drop
    /// joins that thread.
    pub fn start(root: &Path, on_batch: impl Fn(Batch) + Send + 'static) -> io::Result<Watcher> {
        // SAFETY: no preconditions.
        let since = unsafe { fse::FSEventsGetCurrentEventId() };
        Self::start_in(root, &std::env::temp_dir(), since, on_batch)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// `start` with the probe directory under `probe_parent` and the stream replaying from
    /// `since`; tests use these to put the probe inside the root and to observe replay.
    ///
    /// The stream is created at `since`, not SinceNow: fseventsd replays every event after
    /// that id before live ones, in order, so nothing that happens while the stream is being
    /// registered is lost. Root and probe share that one stream, created once and never
    /// re-watched, so the probe's live event arriving proves the stream is delivering, and
    /// everything under the root up to that point has been delivered before it.
    fn start_in(
        root: &Path,
        probe_parent: &Path,
        since: fse::FSEventStreamEventId,
        on_batch: impl Fn(Batch) + Send + 'static,
    ) -> io::Result<Watcher> {
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
        // Built first so any error below tears down whatever exists by then.
        let mut w = Watcher { root: root.clone(), probe: probe.clone(), stream: None, worker: None };

        let (tx, rx) = mpsc::channel();
        let stream = w.stream.insert(Stream::new(&[&root, &probe], since, tx)?);
        let (ready_tx, ready_rx) = mpsc::channel();
        w.worker = Some(
            thread::Builder::new()
                .name("osoji-watch".into())
                .spawn(move || deliver(rx, &root, &probe, ready_tx, on_batch))?,
        );
        if !stream.start() {
            return Err(io::Error::other("FSEventStreamStart failed"));
        }
        fs::write(w.probe.join("marker"), b"")?;
        if ready_rx.recv_timeout(READY_TIMEOUT).is_err() {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!("FSEvents delivered nothing for {} within {READY_TIMEOUT:?}", w.root.display()),
            ));
        }
        Ok(w)
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        // Releasing the stream drops the sender, which ends the worker's loop, so on_batch
        // never runs after drop returns.
        self.stream.take();
        if let Some(t) = self.worker.take() {
            let _ = t.join();
        }
        let _ = fs::remove_dir_all(&self.probe);
    }
}

/// Worker loop. Delivery is kept off the dispatch queue because on_batch may do I/O and take
/// locks, which would stall FSEvents callbacks.
fn deliver(rx: Receiver<Events>, root: &Path, probe: &Path, ready: Sender<()>, on_batch: impl Fn(Batch)) {
    let marker = probe.join("marker");
    while let Ok(mut events) = rx.recv() {
        // Merge callbacks that queued up while on_batch ran.
        while let Ok(more) = rx.try_recv() {
            events.extend(more);
        }
        if events.iter().any(|(p, _)| *p == marker) {
            // Fails once start has stopped listening, which is fine.
            let _ = ready.send(());
        }
        let b = batch(root, probe, &events);
        if !b.dirs.is_empty() || !b.rescan.is_empty() {
            on_batch(b);
        }
    }
}

/// `probe` is excluded explicitly because it may sit inside `root` (root `/` or home).
fn batch(root: &Path, probe: &Path, events: &[(PathBuf, fse::FSEventStreamEventFlags)]) -> Batch {
    const RESCAN: fse::FSEventStreamEventFlags = fse::kFSEventStreamEventFlagMustScanSubDirs
        | fse::kFSEventStreamEventFlagUserDropped
        | fse::kFSEventStreamEventFlagKernelDropped;
    let mut b = Batch::default();
    for (p, flags) in events {
        // HistoryDone only marks the end of replay; its path means nothing.
        if flags & fse::kFSEventStreamEventFlagHistoryDone != 0 || p.starts_with(probe) {
            continue;
        }
        // The root itself was moved or deleted (WatchRoot): nothing below it can be trusted.
        if flags & fse::kFSEventStreamEventFlagRootChanged != 0 {
            b.rescan.push(root.to_path_buf());
        } else if flags & RESCAN != 0 {
            if p.starts_with(root) {
                b.rescan.push(p.clone());
            } else if root.starts_with(p) {
                b.rescan.push(root.to_path_buf());
            }
        } else if p.starts_with(root) {
            // Any change to p, including either side of a rename, changes its parent's listing.
            let dir = if p == root { p } else { p.parent().unwrap_or(root) };
            b.dirs.push(dir.to_path_buf());
        }
    }
    b.normalize();
    b
}

// ---- FFI: everything unsafe lives below. ----

// Missing from fsevent-sys. CoreServices is already linked by it; libdispatch is in libSystem.
#[link(name = "CoreServices", kind = "framework")]
extern "C" {
    fn FSEventStreamSetDispatchQueue(stream: fse::FSEventStreamRef, queue: *mut c_void);
}
extern "C" {
    fn dispatch_queue_create(label: *const c_char, attr: *const c_void) -> *mut c_void;
    fn dispatch_sync_f(queue: *mut c_void, context: *mut c_void, work: extern "C" fn(*mut c_void));
    fn dispatch_release(obj: *mut c_void);
}

/// An FSEvents stream scheduled on its own serial dispatch queue. Its context owns the
/// sender, so releasing the stream closes the channel.
struct Stream {
    stream: fse::FSEventStreamRef,
    queue: *mut c_void,
}

// SAFETY: both refs are owned exclusively by this value and only touched through &mut/&self
// on whichever thread owns the Watcher; FSEvents and libdispatch objects are not tied to the
// thread that created them. The callback reaches the context only through the queue.
unsafe impl Send for Stream {}

impl Stream {
    fn new(paths: &[&Path], since: fse::FSEventStreamEventId, tx: Sender<Events>) -> io::Result<Stream> {
        let cpaths = paths
            .iter()
            .map(|p| CString::new(p.as_os_str().as_bytes()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        // SAFETY: plain CF/dispatch calls on values created here; every ref created is either
        // released before returning or handed to the returned Stream.
        unsafe {
            let array = fse::core_foundation::CFArrayCreateMutable(
                fse::core_foundation::kCFAllocatorDefault,
                0,
                &fse::core_foundation::kCFTypeArrayCallBacks,
            );
            if array.is_null() {
                return Err(io::Error::other("CFArrayCreateMutable failed"));
            }
            for c in &cpaths {
                let s = fse::core_foundation::CFStringCreateWithCString(
                    fse::core_foundation::kCFAllocatorDefault,
                    c.as_ptr(),
                    fse::core_foundation::kCFStringEncodingUTF8,
                );
                if s.is_null() {
                    fse::core_foundation::CFRelease(array);
                    return Err(io::Error::new(io::ErrorKind::InvalidInput, "path is not UTF-8"));
                }
                // The array retains it.
                fse::core_foundation::CFArrayAppendValue(array, s);
                fse::core_foundation::CFRelease(s);
            }
            let ctx = fse::FSEventStreamContext {
                version: 0,
                info: Box::into_raw(Box::new(tx)).cast(),
                retain: None,
                release: Some(release_context),
                copy_description: None,
            };
            let stream = fse::FSEventStreamCreate(
                fse::core_foundation::kCFAllocatorDefault,
                callback,
                &ctx,
                array,
                since,
                LATENCY,
                fse::kFSEventStreamCreateFlagFileEvents | fse::kFSEventStreamCreateFlagWatchRoot,
            );
            // The stream copies the path list.
            fse::core_foundation::CFRelease(array);
            if stream.is_null() {
                // Whether the release callback ran is undocumented, so the sender is leaked
                // rather than risk a double free. The receiver has no worker yet.
                return Err(io::Error::other("FSEventStreamCreate failed"));
            }
            let queue = dispatch_queue_create(c"ai.osoji.watch".as_ptr(), std::ptr::null());
            if queue.is_null() {
                fse::FSEventStreamRelease(stream);
                return Err(io::Error::other("dispatch_queue_create failed"));
            }
            FSEventStreamSetDispatchQueue(stream, queue);
            Ok(Stream { stream, queue })
        }
    }

    fn start(&self) -> bool {
        // SAFETY: the stream is valid and scheduled on a queue.
        unsafe { fse::FSEventStreamStart(self.stream) != 0 }
    }
}

impl Drop for Stream {
    fn drop(&mut self) {
        // SAFETY: valid refs owned by self, torn down once.
        unsafe {
            fse::FSEventStreamStop(self.stream);
            fse::FSEventStreamInvalidate(self.stream);
            // A callback already running on the serial queue still borrows the context; an
            // empty sync block waits it out so the release below cannot free it underneath.
            dispatch_sync_f(self.queue, std::ptr::null_mut(), noop);
            // Drops the context, and with it the sender.
            fse::FSEventStreamRelease(self.stream);
            dispatch_release(self.queue);
        }
    }
}

extern "C" fn noop(_: *mut c_void) {}

extern "C" fn release_context(info: *const c_void) {
    // SAFETY: info is the Box created in Stream::new; FSEvents calls this exactly once.
    drop(unsafe { Box::from_raw(info as *mut Sender<Events>) });
}

/// Runs on the stream's queue. Never unwinds into C: a panic here only loses this callback.
extern "C" fn callback(
    _stream: fse::FSEventStreamRef,
    info: *mut c_void,
    n: usize,
    paths: *mut c_void,
    flags: *const fse::FSEventStreamEventFlags,
    _ids: *const fse::FSEventStreamEventId,
) {
    if n == 0 {
        return;
    }
    let _ = std::panic::catch_unwind(|| {
        // SAFETY: info is the live context (released only after the queue is drained); without
        // UseCFTypes, paths is an array of n C strings and flags an array of n flags, valid for
        // the duration of the callback.
        let (tx, paths, flags) = unsafe {
            (
                &*(info as *const Sender<Events>),
                std::slice::from_raw_parts(paths as *const *const c_char, n),
                std::slice::from_raw_parts(flags, n),
            )
        };
        let events = paths
            .iter()
            .zip(flags)
            // SAFETY: each entry is a NUL-terminated string (see above).
            .map(|(&p, &f)| (PathBuf::from(OsStr::from_bytes(unsafe { CStr::from_ptr(p) }.to_bytes())), f))
            .collect();
        // Fails only while the watcher is being dropped.
        let _ = tx.send(events);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{temp_root, write};
    use fse::{
        kFSEventStreamEventFlagHistoryDone as HISTORY_DONE, kFSEventStreamEventFlagItemCreated as CREATED,
        kFSEventStreamEventFlagItemModified as MODIFIED, kFSEventStreamEventFlagItemRenamed as RENAMED,
        kFSEventStreamEventFlagKernelDropped as KERNEL_DROPPED, kFSEventStreamEventFlagMustScanSubDirs as SCAN_SUBDIRS,
        kFSEventStreamEventFlagRootChanged as ROOT_CHANGED, kFSEventStreamEventFlagUserDropped as USER_DROPPED,
    };
    use std::sync::mpsc::{channel, RecvTimeoutError};
    use std::time::Instant;

    fn paths(ps: &[&str]) -> Vec<PathBuf> {
        ps.iter().map(PathBuf::from).collect()
    }

    fn ev(flags: fse::FSEventStreamEventFlags, p: &str) -> (PathBuf, fse::FSEventStreamEventFlags) {
        (PathBuf::from(p), flags)
    }

    /// Batches events for root /r with the probe outside it.
    fn in_r(events: &[(PathBuf, fse::FSEventStreamEventFlags)]) -> Batch {
        batch(Path::new("/r"), Path::new("/probe"), events)
    }

    #[test]
    fn maps_events_to_parent_dirs_inside_the_root() {
        let b = in_r(&[
            ev(CREATED, "/r/a/f"),
            ev(MODIFIED, "/r/a/g"),
            // FSEvents reports each side of a rename as its own event.
            ev(RENAMED, "/r/a/x"),
            ev(RENAMED, "/r/b/c/x"),
            ev(MODIFIED, "/r"),
            ev(CREATED, "/elsewhere/f"),
            ev(CREATED, "/rr/f"),
            ev(CREATED, "/probe/marker"),
            ev(HISTORY_DONE, ""),
        ]);
        assert_eq!(b.dirs, paths(&["/r", "/r/a", "/r/b/c"]));
        assert!(b.rescan.is_empty());
    }

    #[test]
    fn rescans_are_clipped_to_the_root_and_collapsed() {
        let b = in_r(&[
            ev(SCAN_SUBDIRS, "/r/a/b"),
            ev(USER_DROPPED, "/r/a"),
            ev(KERNEL_DROPPED, "/r/c"),
            ev(SCAN_SUBDIRS, "/x"),
        ]);
        assert_eq!(b.rescan, paths(&["/r/a", "/r/c"]));
        assert!(b.dirs.is_empty());

        assert_eq!(in_r(&[ev(USER_DROPPED, "/"), ev(SCAN_SUBDIRS, "/r/a")]).rescan, paths(&["/r"]));
        assert_eq!(in_r(&[ev(KERNEL_DROPPED, "")]).rescan, paths(&["/r"]));
    }

    #[test]
    fn merge_folds_batches_into_one_normalized_batch() {
        let merged = Batch::merge([
            in_r(&[ev(CREATED, "/r/b/f"), ev(CREATED, "/r/a/f")]),
            in_r(&[ev(MODIFIED, "/r/a/g"), ev(SCAN_SUBDIRS, "/r/c/d")]),
            in_r(&[ev(SCAN_SUBDIRS, "/r/c")]),
        ]);
        assert_eq!(merged.dirs, paths(&["/r/a", "/r/b"]));
        assert_eq!(merged.rescan, paths(&["/r/c"]));
        assert_eq!(Batch::merge([]), Batch::default());
    }

    #[test]
    fn root_changed_rescans_the_root() {
        let b = in_r(&[ev(ROOT_CHANGED, "/r"), ev(SCAN_SUBDIRS, "/r/a")]);
        assert_eq!(b.rescan, paths(&["/r"]));
        assert!(b.dirs.is_empty());
        // The probe is a watched root too; its moves are not the root's.
        assert_eq!(in_r(&[ev(ROOT_CHANGED, "/probe")]), Batch::default());
    }

    #[test]
    fn probe_paths_inside_the_root_are_dropped() {
        let events = [
            ev(CREATED, "/r/p/marker"),
            ev(MODIFIED, "/r/p"),
            ev(RENAMED, "/r/p/x"),
            ev(RENAMED, "/r/a/x"),
            ev(SCAN_SUBDIRS, "/r/p"),
            ev(ROOT_CHANGED, "/r/p"),
            ev(CREATED, "/r/pp/f"),
        ];
        let b = batch(Path::new("/r"), Path::new("/r/p"), &events);
        assert_eq!(b.dirs, paths(&["/r/a", "/r/pp"]));
        assert!(b.rescan.is_empty());
    }

    // FSEvents latency is usually 1-2 s on top of the 0.5 s window.
    const WAIT: Duration = Duration::from_secs(15);

    /// Checks every batch path is inside `root` and outside the probe.
    fn check(b: &Batch, root: &Path, probe: &Path) {
        for p in b.dirs.iter().chain(&b.rescan) {
            assert!(p.starts_with(root), "{} outside {}", p.display(), root.display());
            assert!(!p.starts_with(probe), "probe path {} reported", p.display());
        }
    }

    /// Collects dirs from batches until `want` are all seen, failing after WAIT.
    fn await_dirs(rx: &Receiver<Batch>, w: &Watcher, want: &[&Path]) {
        let t = Instant::now();
        let deadline = t + WAIT;
        let mut seen: Vec<PathBuf> = Vec::new();
        while !want.iter().all(|w| seen.iter().any(|s| s == w)) {
            let left = deadline.saturating_duration_since(Instant::now());
            match rx.recv_timeout(left) {
                Ok(b) => {
                    check(&b, &w.root, &w.probe);
                    seen.extend(b.dirs);
                }
                Err(e) => panic!("{e:?}: wanted {want:?}, saw {seen:?}"),
            }
        }
        eprintln!("saw {want:?} after {:?}", t.elapsed());
    }

    fn start_with(root: &Path, probe_parent: &Path, since: fse::FSEventStreamEventId) -> (Watcher, Receiver<Batch>) {
        let (tx, rx) = channel();
        let t = Instant::now();
        let w = Watcher::start_in(root, probe_parent, since, move |b| {
            let _ = tx.send(b);
        })
        .unwrap();
        eprintln!("start took {:?}", t.elapsed());
        (w, rx)
    }

    fn now() -> fse::FSEventStreamEventId {
        unsafe { fse::FSEventsGetCurrentEventId() }
    }

    fn start(root: &Path) -> (Watcher, Receiver<Batch>) {
        start_with(root, &std::env::temp_dir(), now())
    }

    // Each test writes immediately after start returns: that is the readiness guarantee.

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

    /// The stream is created at an event id captured before these writes, so they reach it
    /// only through fseventsd's replay: the same path that covers changes made while `start`
    /// is still registering the stream.
    #[test]
    fn changes_before_the_stream_is_live_are_replayed() {
        let root = temp_root("replay");
        let since = now();
        let sub = root.join("sub");
        fs::create_dir(&sub).unwrap();
        for i in 0..50 {
            write(&sub.join(format!("f{i}")), 10);
        }
        let (w, rx) = start_with(&root, &std::env::temp_dir(), since);
        await_dirs(&rx, &w, &[&root, &sub]);
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
        let (w, rx) = start_with(&root, &root, now());
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
