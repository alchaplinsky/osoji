//! Tauri backend: owns the scan state and exposes bounded views to the UI.
//! The arena never crosses IPC; commands return only what is on screen.

#[cfg(target_os = "macos")]
mod zoom;

use scan_core::{Arena, Cancel, ChildIndex, Flags, NodeId, Progress, ScanConfig, NONE};
use scan_policy::{Policy, PolicyContext, PolicyHits};
use serde::Serialize;
use std::borrow::Cow;
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::ffi::{CStr, CString};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::ipc::Channel;
use tauri::State;

struct Scan {
    root: PathBuf,
    arena: Arena,
    index: ChildIndex,
    hits: PolicyHits,
}

#[derive(Default)]
struct AppState {
    scan: Mutex<Option<Scan>>,
    cancel: Mutex<Option<Arc<Cancel>>>,
    running: AtomicBool,
    policy: Policy,
    /// Bumped each time `scan` is replaced, so a sweep can tell its arena went stale.
    generation: AtomicU64,
    sweeping: AtomicBool,
}

/// Why the UI should treat a node with care: "protected", "risky" or "unexpected".
#[derive(Serialize, Debug, PartialEq)]
struct Guard {
    kind: &'static str,
    reason: String,
}

const POLICY_FLAGS: Flags = Flags::PROTECTED.union(Flags::RISKY).union(Flags::UNEXPECTED);

fn guard_of(policy: &Policy, scan: &Scan, id: NodeId) -> Option<Guard> {
    if !scan.arena.flags[id as usize].intersects(POLICY_FLAGS) {
        return None;
    }
    let rule = policy.rule(scan.hits.rule(id)?);
    Some(Guard { kind: rule.kind.as_str(), reason: rule.reason.clone() })
}

#[derive(Serialize, Clone)]
struct TopProgress {
    name: String,
    bytes: u64,
}

#[derive(Serialize, Clone)]
struct ProgressMsg {
    files: u64,
    dirs: u64,
    bytes: u64,
    errors: u64,
    elapsed_ms: u64,
    /// Bytes found under each of the root's first direct children, in listing order.
    tops: Vec<TopProgress>,
    /// Bytes under children past the tracked ones.
    rest: u64,
}

#[derive(Serialize)]
struct ScanSummary {
    root: String,
    entries: usize,
    bytes: u64,
    errors: u64,
    elapsed_ms: u64,
    cancelled: bool,
}

#[derive(Serialize)]
struct ViewNode {
    id: NodeId,
    name: String,
    size: u64,
    items: u32,
    dir: bool,
    error: bool,
    children: Vec<ViewNode>,
    other: u64,
    other_count: u32,
    guard: Option<Guard>,
}

#[derive(Serialize)]
struct Crumb {
    id: NodeId,
    name: String,
}

#[derive(Serialize)]
struct NodeInfo {
    id: NodeId,
    name: String,
    path: String,
    size: u64,
    items: u32,
    dir: bool,
    error: bool,
    trashed: bool,
    mtime: i64,
    ancestors: Vec<Crumb>,
    guard: Option<Guard>,
}

#[derive(Serialize)]
struct TrashResult {
    bytes: u64,
    count: usize,
    trashed: Vec<NodeId>,
    failed: Vec<String>,
}

#[derive(Serialize, Clone)]
struct TrashProgress {
    done: usize,
    total: usize,
    bytes: u64,
    /// Name of the item being moved.
    current: String,
}

#[derive(Serialize)]
struct FileEntry {
    id: NodeId,
    name: String,
    path: String,
    size: u64,
    mtime: i64,
    dir: bool,
    ancestors: Vec<NodeId>,
    guard: Option<Guard>,
}

#[derive(Serialize)]
struct Suggestion {
    #[serde(flatten)]
    entry: FileEntry,
    safety: &'static str,
    reason: String,
}

fn name_of(arena: &Arena, id: NodeId) -> String {
    String::from_utf8_lossy(arena.name(id)).into_owned()
}

fn ancestors_of(arena: &Arena, id: NodeId) -> Vec<NodeId> {
    let mut out = Vec::new();
    let mut p = arena.parent[id as usize];
    while p != NONE {
        out.push(p);
        p = arena.parent[p as usize];
    }
    out.reverse();
    out
}

fn file_entry(policy: &Policy, scan: &Scan, id: NodeId) -> FileEntry {
    let arena = &scan.arena;
    let i = id as usize;
    FileEntry {
        id,
        name: name_of(arena, id),
        path: arena.path(id).to_string_lossy().into_owned(),
        size: arena.size[i],
        mtime: arena.mtime[i],
        dir: arena.flags[i].contains(Flags::DIR),
        ancestors: ancestors_of(arena, id),
        guard: guard_of(policy, scan, id),
    }
}

/// Directories macOS shows as a single file. Treated as one item, never descended into.
const PACKAGE_SUFFIXES: [&[u8]; 11] = [
    b".app",
    b".photoslibrary",
    b".musiclibrary",
    b".fcpbundle",
    b".imovielibrary",
    b".pvm",
    b".vmwarevm",
    b".logicx",
    b".bundle",
    b".framework",
    b".xcarchive",
];

fn is_package(arena: &Arena, id: NodeId) -> bool {
    let name = arena.name(id);
    PACKAGE_SUFFIXES.iter().any(|s| name.ends_with(s))
}

/// Resolve an absolute path to a node of the scan, or NONE when it is outside the scan or missing.
fn find_node(scan: &Scan, path: &Path) -> NodeId {
    use std::os::unix::ffi::OsStrExt;
    let Ok(rest) = path.strip_prefix(&scan.root) else {
        return NONE;
    };
    let mut id: NodeId = 0;
    for part in rest.components() {
        let name = part.as_os_str().as_bytes();
        match scan.index.children(id).iter().find(|&&c| scan.arena.name(c) == name) {
            Some(&c) => id = c,
            None => return NONE,
        }
    }
    id
}

/// The walker refuses symlinks anywhere in a path, so scan the real path; and fail loudly on an
/// unreadable root instead of returning an empty scan.
fn resolve_root(root: &str) -> Result<PathBuf, String> {
    let path = std::fs::canonicalize(root).map_err(|e| format!("Can't open {root}: {e}"))?;
    std::fs::read_dir(&path).map_err(|e| format!("Can't read {root}: {e}"))?;
    Ok(path)
}

#[tauri::command]
async fn start_scan(
    state: State<'_, AppState>,
    root: String,
    threads: Option<usize>,
    on_progress: Channel<ProgressMsg>,
) -> Result<ScanSummary, String> {
    if state.sweeping.load(Ordering::Acquire) {
        return Err("wait for the sweep to finish".into());
    }
    if state.running.swap(true, Ordering::AcqRel) {
        return Err("a scan is already running".into());
    }
    let canonical = match resolve_root(&root) {
        Ok(p) => p,
        Err(e) => {
            state.running.store(false, Ordering::Release);
            return Err(e);
        }
    };
    let cancel = Arc::new(AtomicBool::new(false));
    *state.cancel.lock().unwrap() = Some(cancel.clone());

    let cfg = ScanConfig {
        root: canonical,
        threads: threads.unwrap_or_else(|| std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(8)),
        cross_mounts: false,
        qos: true,
    };
    let progress = Arc::new(Progress::default());
    let done = Arc::new(AtomicBool::new(false));
    let t0 = Instant::now();

    let poller = {
        let (progress, done) = (progress.clone(), done.clone());
        std::thread::spawn(move || {
            while !done.load(Ordering::Acquire) {
                std::thread::sleep(Duration::from_millis(100));
                let _ = on_progress.send(ProgressMsg {
                    files: progress.files.load(Ordering::Relaxed),
                    dirs: progress.dirs.load(Ordering::Relaxed),
                    bytes: progress.bytes.load(Ordering::Relaxed),
                    errors: progress.errors.load(Ordering::Relaxed),
                    elapsed_ms: t0.elapsed().as_millis() as u64,
                    tops: progress.tops().into_iter().map(|(name, bytes)| TopProgress { name, bytes }).collect(),
                    rest: progress.rest(),
                });
            }
        })
    };

    let task = {
        let (cfg, progress, cancel) = (cfg.clone(), progress.clone(), cancel.clone());
        tauri::async_runtime::spawn_blocking(move || scan_macos::walk(&cfg, &progress, &cancel))
            .await
    };
    done.store(true, Ordering::Release);
    let _ = poller.join();
    state.running.store(false, Ordering::Release);
    state.cancel.lock().unwrap().take();

    let result = task.map_err(|e| e.to_string())?;
    let mut arena = result.map_err(|e| e.to_string())?;
    let cancelled = cancel.load(Ordering::Relaxed);
    let summary = ScanSummary {
        root: root.clone(),
        entries: arena.len(),
        bytes: arena.size[0],
        errors: progress.errors.load(Ordering::Relaxed),
        elapsed_ms: t0.elapsed().as_millis() as u64,
        cancelled,
    };
    // A stopped walk leaves every unfinished directory under-counted, so it never replaces the
    // last complete scan.
    if !cancelled {
        let index = ChildIndex::build(&arena);
        let ctx = PolicyContext { home: home_path(), now: unix_now() };
        let hits = state.policy.resolve(&arena, &index, &ctx).stamp(&mut arena);
        let mut slot = state.scan.lock().unwrap();
        *slot = Some(Scan { root: cfg.root, arena, index, hits });
        state.generation.fetch_add(1, Ordering::AcqRel);
    }
    Ok(summary)
}

/// Canonical, because `Scan.root` is canonical and a home reached through a symlink must still match.
fn home_path() -> PathBuf {
    let home = PathBuf::from(std::env::var("HOME").unwrap_or_default());
    std::fs::canonicalize(&home).unwrap_or(home)
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

#[tauri::command]
fn cancel_scan(state: State<'_, AppState>) {
    if let Some(c) = state.cancel.lock().unwrap().as_ref() {
        c.store(true, Ordering::Release);
    }
}

fn build_view(policy: &Policy, scan: &Scan, id: NodeId, depth: u8, top_k: usize, min_size: u64) -> ViewNode {
    let a = &scan.arena;
    let i = id as usize;
    let flags = a.flags[i];
    let mut node = ViewNode {
        id,
        name: name_of(a, id),
        size: a.size[i],
        items: a.items[i],
        dir: flags.contains(Flags::DIR),
        error: flags.contains(Flags::ERROR),
        children: Vec::new(),
        other: 0,
        other_count: 0,
        guard: guard_of(policy, scan, id),
    };
    if depth == 0 || !node.dir {
        return node;
    }
    for &c in scan.index.children(id) {
        let ci = c as usize;
        if a.flags[ci].contains(Flags::TRASHED) {
            continue;
        }
        if node.children.len() < top_k && a.size[ci] >= min_size {
            node.children.push(build_view(policy, scan, c, depth - 1, top_k, min_size));
        } else {
            node.other += a.size[ci];
            node.other_count += 1;
        }
    }
    node
}

#[tauri::command]
fn get_view(
    state: State<'_, AppState>,
    node: NodeId,
    depth: u8,
    top_k: usize,
    min_fraction: f64,
) -> Result<ViewNode, String> {
    let guard = state.scan.lock().unwrap();
    let scan = guard.as_ref().ok_or("no scan loaded")?;
    if (node as usize) >= scan.arena.len() {
        return Err("unknown node".into());
    }
    let min_size = (scan.arena.size[node as usize] as f64 * min_fraction) as u64;
    Ok(build_view(&state.policy, scan, node, depth, top_k, min_size.max(1)))
}

#[tauri::command]
fn node_info(state: State<'_, AppState>, node: NodeId) -> Result<NodeInfo, String> {
    let guard = state.scan.lock().unwrap();
    let scan = guard.as_ref().ok_or("no scan loaded")?;
    let a = &scan.arena;
    if (node as usize) >= a.len() {
        return Err("unknown node".into());
    }
    let ancestors = ancestors_of(a, node).into_iter().map(|id| Crumb { id, name: name_of(a, id) }).collect();
    let f = a.flags[node as usize];
    Ok(NodeInfo {
        id: node,
        name: name_of(a, node),
        path: a.path(node).to_string_lossy().into_owned(),
        size: a.size[node as usize],
        items: a.items[node as usize],
        dir: f.contains(Flags::DIR),
        error: f.contains(Flags::ERROR),
        trashed: f.contains(Flags::TRASHED),
        mtime: a.mtime[node as usize],
        ancestors,
        guard: guard_of(&state.policy, scan, node),
    })
}

#[tauri::command]
fn largest_files(state: State<'_, AppState>, limit: usize) -> Result<Vec<FileEntry>, String> {
    let guard = state.scan.lock().unwrap();
    let scan = guard.as_ref().ok_or("no scan loaded")?;
    let a = &scan.arena;
    if limit == 0 || a.is_empty() {
        return Ok(Vec::new());
    }
    let mut heap: BinaryHeap<Reverse<(u64, NodeId)>> = BinaryHeap::with_capacity(limit + 1);
    let mut stack: Vec<NodeId> = vec![0];
    while let Some(id) = stack.pop() {
        let i = id as usize;
        let f = a.flags[i];
        let size = a.size[i];
        if f.contains(Flags::TRASHED) || size == 0 {
            continue;
        }
        if heap.len() == limit && heap.peek().is_some_and(|m| size <= m.0 .0) {
            continue;
        }
        if !f.contains(Flags::DIR) || (id != 0 && is_package(a, id)) {
            heap.push(Reverse((size, id)));
            if heap.len() > limit {
                heap.pop();
            }
            continue;
        }
        stack.extend(scan.index.children(id).iter().rev());
    }
    Ok(heap.into_sorted_vec().into_iter().map(|Reverse((_, id))| file_entry(&state.policy, scan, id)).collect())
}

const DAY: i64 = 86_400;
const MIB: u64 = 1024 * 1024;
const UNDER_HOME: u8 = 1;
const UNDER_XCODE: u8 = 2;
const UNDER_CACHES: u8 = 4;

struct Places {
    home: NodeId,
    xcode: NodeId,
    caches: NodeId,
    sim_caches: NodeId,
    downloads: NodeId,
    var_vm: NodeId,
    system: NodeId,
}

const PACKAGE_CACHE: &str = "Package manager cache. Downloaded again when needed.";

fn parent_named(a: &Arena, id: NodeId, name: &[u8]) -> NodeId {
    let p = a.parent[id as usize];
    if p != NONE && a.name(p) == name {
        p
    } else {
        NONE
    }
}

fn has_child(scan: &Scan, id: NodeId, name: &[u8]) -> bool {
    scan.index.children(id).iter().any(|&c| scan.arena.name(c) == name)
}

fn plural(n: i64, unit: &str) -> String {
    if n == 1 {
        format!("1 {unit}")
    } else {
        format!("{n} {unit}s")
    }
}

fn span(days: i64) -> String {
    if days < 60 {
        plural(days, "day")
    } else if days < 365 {
        plural(days / 30, "month")
    } else {
        plural(days / 365, "year")
    }
}

fn is_package_cache(a: &Arena, p: &Places, id: NodeId, ctx: u8) -> bool {
    match a.name(id) {
        b".gradle" => ctx & UNDER_HOME != 0,
        b".cache" => p.home != NONE && a.parent[id as usize] == p.home,
        b"_cacache" => parent_named(a, id, b".npm") != NONE,
        b"cache" => {
            let reg = parent_named(a, id, b"registry");
            let inst = parent_named(a, id, b"install");
            (reg != NONE && parent_named(a, reg, b".cargo") != NONE)
                || (inst != NONE && parent_named(a, inst, b".bun") != NONE)
        }
        _ => false,
    }
}

fn classify(scan: &Scan, p: &Places, id: NodeId, ctx: u8, now: i64) -> Option<(&'static str, Cow<'static, str>)> {
    let a = &scan.arena;
    let i = id as usize;
    let dir = a.flags[i].contains(Flags::DIR);
    let name = a.name(id);
    let parent = a.parent[i];
    let rule = |safety, reason: &'static str| Some((safety, Cow::Borrowed(reason)));

    if dir && name.ends_with(b".photoslibrary") {
        return rule("keep", "Your photos. Unshift never suggests removing these.");
    }
    if id == p.var_vm {
        return rule("keep", "Swap files. macOS manages these itself.");
    }
    if id == p.system {
        return rule("keep", "Protected by macOS and cannot be changed.");
    }
    if ctx & UNDER_CACHES != 0 && (parent == p.caches || parent == p.sim_caches) {
        return rule("safe", "App cache. Rebuilt automatically the next time the app runs.");
    }
    if dir {
        if name == b"node_modules" {
            return rule("safe", "Installed packages. Reinstalled by npm, pnpm or bun the next time you run install.");
        }
        if name == b"target" && parent != NONE && has_child(scan, parent, b"Cargo.toml") {
            return rule("safe", "Rust build output. Rebuilt by cargo.");
        }
        if ctx & UNDER_XCODE != 0 && name == b"DerivedData" {
            return rule("safe", "Build leftovers. Xcode recreates them the next time you open a project.");
        }
        if ctx & UNDER_XCODE != 0 && name == b"iOS DeviceSupport" {
            return rule("safe", "Debug files for devices you may no longer connect. Xcode re-downloads them if needed.");
        }
        if is_package_cache(a, p, id, ctx) {
            return rule("safe", PACKAGE_CACHE);
        }
        if name.ends_with(b".pvm") || name.ends_with(b".vmwarevm") {
            return rule("review", "A virtual machine. Remove only if you no longer need it.");
        }
    }
    if name == b"Docker.raw" {
        return rule("review", "Docker's disk image. Prune images and containers from Docker instead of removing it.");
    }
    let mtime = a.mtime[i];
    if mtime <= 0 {
        return None;
    }
    let days = (now - mtime) / DAY;
    if p.downloads != NONE && parent == p.downloads && days > 90 {
        return Some(("review", Cow::Owned(format!("In Downloads for {}, and probably already used.", span(days)))));
    }
    if !dir && a.size[i] >= 500 * MIB && days > 180 {
        return Some(("review", Cow::Owned(format!("Large file not modified in {}.", span(days)))));
    }
    None
}

/// Whether the current user may create or remove entries in `dir`, which moving a child to the Trash requires.
fn is_writable(dir: &Path) -> bool {
    use std::os::unix::ffi::OsStrExt;
    let Ok(c) = CString::new(dir.as_os_str().as_bytes()) else {
        return false;
    };
    unsafe { libc::access(c.as_ptr(), libc::W_OK) == 0 }
}

#[tauri::command]
fn suggestions(state: State<'_, AppState>) -> Result<Vec<Suggestion>, String> {
    let guard = state.scan.lock().unwrap();
    let scan = guard.as_ref().ok_or("no scan loaded")?;
    Ok(collect_suggestions(&state.policy, scan, &home_path(), unix_now()))
}

fn collect_suggestions(policy: &Policy, scan: &Scan, home: &Path, now: i64) -> Vec<Suggestion> {
    let a = &scan.arena;
    if a.is_empty() {
        return Vec::new();
    }
    let has_home = home.is_absolute();
    let at = |rel: &str| if has_home { find_node(scan, &home.join(rel)) } else { NONE };
    let xcode_path = home.join("Library/Developer/Xcode");
    let places = Places {
        home: if has_home { find_node(scan, home) } else { NONE },
        xcode: at("Library/Developer/Xcode"),
        caches: at("Library/Caches"),
        sim_caches: at("Library/Developer/CoreSimulator/Caches"),
        downloads: at("Downloads"),
        var_vm: find_node(scan, Path::new("/private/var/vm")),
        system: find_node(scan, Path::new("/System")),
    };
    // A scan rooted inside home (or Xcode) never contains the home node itself.
    let mut root_ctx = 0;
    if has_home && scan.root.starts_with(home) {
        root_ctx |= UNDER_HOME;
    }
    if has_home && scan.root.starts_with(&xcode_path) {
        root_ctx |= UNDER_XCODE;
    }

    let mut found: Vec<(u64, NodeId, &'static str, Cow<'static, str>)> = Vec::new();
    let mut stack: Vec<(NodeId, u8)> = vec![(0, root_ctx)];
    while let Some((id, ctx)) = stack.pop() {
        let i = id as usize;
        let f = a.flags[i];
        if f.contains(Flags::TRASHED) {
            continue;
        }
        if id != 0 {
            if let Some((safety, reason)) = classify(scan, &places, id, ctx, now) {
                if a.size[i] > 0 {
                    found.push((a.size[i], id, safety, reason));
                }
                continue;
            }
            if !f.contains(Flags::DIR) || is_package(a, id) {
                continue;
            }
        }
        let mut child_ctx = ctx;
        if id == places.home {
            child_ctx |= UNDER_HOME;
        }
        if id == places.xcode {
            child_ctx |= UNDER_XCODE;
        }
        if id == places.caches || id == places.sim_caches {
            child_ctx |= UNDER_CACHES;
        }
        stack.extend(scan.index.children(id).iter().map(|&c| (c, child_ctx)));
    }
    found.sort_unstable_by(|x, y| y.0.cmp(&x.0));
    found.truncate(200);
    for (_, id, safety, reason) in &mut found {
        let parent = a.parent[*id as usize];
        if parent != NONE && !is_writable(&a.path(parent)) {
            *safety = "keep";
            *reason = Cow::Borrowed("Protected by macOS and cannot be moved by Unshift.");
        }
    }
    found
        .into_iter()
        .map(|(_, id, safety, reason)| Suggestion { entry: file_entry(policy, scan, id), safety, reason: reason.into_owned() })
        .collect()
}

#[derive(Serialize)]
struct WatchHit {
    path: String,
    id: Option<NodeId>,
    size: u64,
    dir: bool,
}

fn in_trash(a: &Arena, mut id: NodeId) -> bool {
    while id != NONE {
        if a.flags[id as usize].contains(Flags::TRASHED) {
            return true;
        }
        id = a.parent[id as usize];
    }
    false
}

#[tauri::command]
fn watch_status(state: State<'_, AppState>, paths: Vec<String>) -> Result<Vec<WatchHit>, String> {
    let guard = state.scan.lock().unwrap();
    let scan = guard.as_ref().ok_or("no scan loaded")?;
    let a = &scan.arena;
    Ok(paths
        .into_iter()
        .map(|path| {
            // The scan root is canonical; a path that no longer resolves is gone.
            let id = std::fs::canonicalize(&path).map_or(NONE, |p| find_node(scan, &p));
            if id == NONE || in_trash(a, id) {
                return WatchHit { path, id: None, size: 0, dir: false };
            }
            let i = id as usize;
            WatchHit { path, id: Some(id), size: a.size[i], dir: a.flags[i].contains(Flags::DIR) }
        })
        .collect())
}

#[tauri::command]
fn reveal(state: State<'_, AppState>, node: NodeId) -> Result<(), String> {
    let path = {
        let guard = state.scan.lock().unwrap();
        let scan = guard.as_ref().ok_or("no scan loaded")?;
        if (node as usize) >= scan.arena.len() {
            return Err("unknown node".into());
        }
        scan.arena.path(node)
    };
    std::process::Command::new("open")
        .arg("-R")
        .arg(&path)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

fn has_ancestor(a: &Arena, id: NodeId, ancestor: NodeId) -> bool {
    let mut p = a.parent[id as usize];
    while p != NONE {
        if p == ancestor {
            return true;
        }
        p = a.parent[p as usize];
    }
    false
}

/// Why `id` must not go to the Trash: it is protected, or it holds a protected node that would go with it.
fn protection(policy: &Policy, scan: &Scan, id: NodeId) -> Option<String> {
    let a = &scan.arena;
    let reason = |node: NodeId| scan.hits.rule(node).map_or("", |r| policy.rule(r).reason.as_str());
    if a.flags[id as usize].contains(Flags::PROTECTED) {
        return Some(format!("{}: protected. {}", a.path(id).display(), reason(id)));
    }
    let inner = scan
        .hits
        .iter()
        .map(|(node, _)| node)
        .filter(|&node| a.flags[node as usize].contains(Flags::PROTECTED) && has_ancestor(a, node, id))
        .min()?;
    Some(format!("{}: protected. It contains {}. {}", a.path(id).display(), a.path(inner).display(), reason(inner)))
}

/// One node a sweep will move: id, path, display name and size before the move.
type SweepItem = (NodeId, PathBuf, String, u64);

/// Decide which of `nodes` may go to the Trash. Protected nodes are refused with a reason, unknown
/// and already trashed ids are skipped. Pure, so the guard is testable without Tauri.
fn plan_sweep(policy: &Policy, scan: &Scan, nodes: Vec<NodeId>) -> Result<(Vec<SweepItem>, Vec<String>), String> {
    let mut items = Vec::new();
    let mut refused = Vec::new();
    for id in nodes {
        if (id as usize) >= scan.arena.len() || id == 0 {
            continue;
        }
        if scan.arena.flags[id as usize].contains(Flags::TRASHED) {
            continue;
        }
        let path = scan.arena.path(id);
        if !path.starts_with(&scan.root) {
            return Err(format!("refusing to trash {} outside the scan root", path.display()));
        }
        if let Some(refusal) = protection(policy, scan, id) {
            refused.push(refusal);
            continue;
        }
        items.push((id, path, name_of(&scan.arena, id), scan.arena.size[id as usize]));
    }
    Ok((items, refused))
}

/// Fold the delete outcomes into the tree. `live` is None when another scan replaced the arena
/// meanwhile: the items are still reported as moved, but nothing is marked.
fn finish_sweep(mut live: Option<&mut Scan>, outcomes: Vec<(NodeId, u64, Result<(), String>)>, mut failed: Vec<String>) -> TrashResult {
    let mut bytes = 0u64;
    let mut trashed = Vec::new();
    for (id, size, outcome) in outcomes {
        if let Err(e) = outcome {
            failed.push(e);
            continue;
        }
        trashed.push(id);
        let Some(scan) = live.as_deref_mut() else {
            bytes += size;
            continue;
        };
        bytes += scan.arena.mark_trashed(id);
        let mut p = scan.arena.parent[id as usize];
        while p != NONE {
            scan.index.resort(&scan.arena, p);
            p = scan.arena.parent[p as usize];
        }
    }
    TrashResult { bytes, count: trashed.len(), trashed, failed }
}

#[tauri::command]
async fn trash_nodes(
    state: State<'_, AppState>,
    nodes: Vec<NodeId>,
    on_progress: Channel<TrashProgress>,
) -> Result<TrashResult, String> {
    if state.sweeping.swap(true, Ordering::AcqRel) {
        return Err("a sweep is already running".into());
    }
    let result = sweep(&state, nodes, on_progress).await;
    state.sweeping.store(false, Ordering::Release);
    result
}

/// Moves nodes to the Trash without holding the scan lock during the slow part:
/// validate under the lock, delete off the main thread, then re-lock to mark.
async fn sweep(state: &AppState, nodes: Vec<NodeId>, on_progress: Channel<TrashProgress>) -> Result<TrashResult, String> {
    let (generation, items, refused) = {
        let guard = state.scan.lock().unwrap();
        let scan = guard.as_ref().ok_or("no scan loaded")?;
        let (items, refused) = plan_sweep(&state.policy, scan, nodes)?;
        (state.generation.load(Ordering::Acquire), items, refused)
    };

    let total = items.len();
    let outcomes = tauri::async_runtime::spawn_blocking(move || {
        let mut bytes = 0u64;
        let mut current = String::new();
        let mut outcomes = Vec::with_capacity(total);
        for (done, (id, path, name, size)) in items.into_iter().enumerate() {
            current = name;
            let _ = on_progress.send(TrashProgress { done, total, bytes, current: current.clone() });
            let outcome = trash::delete(&path).map_err(|e| format!("{}: {e}", path.display()));
            if outcome.is_ok() {
                bytes += size;
            }
            outcomes.push((id, size, outcome));
        }
        let _ = on_progress.send(TrashProgress { done: total, total, bytes, current });
        outcomes
    })
    .await
    .map_err(|e| e.to_string())?;

    let mut guard = state.scan.lock().unwrap();
    // A scan that finished meanwhile replaced the arena: the ids no longer point at what was deleted.
    let live = guard.as_mut().filter(|_| state.generation.load(Ordering::Acquire) == generation);
    Ok(finish_sweep(live, outcomes, refused))
}

#[tauri::command]
fn home_dir() -> String {
    std::env::var("HOME").unwrap_or_else(|_| "/".into())
}

#[derive(Serialize)]
struct VolumeInfo {
    name: String,
    mount: String,
    total: u64,
    free: u64,
}

#[tauri::command]
fn volume_info(path: String) -> Result<VolumeInfo, String> {
    let c = CString::new(path).map_err(|e| e.to_string())?;
    let mut st: libc::statfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statfs(c.as_ptr(), &mut st) } != 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    Ok(volume_from(&st))
}

/// User-visible local volumes: the boot disk at `/` plus whatever is mounted under
/// `/Volumes`. That prefix already leaves out `/dev` and the read-only system volumes
/// under `/System/Volumes`, including the boot disk's Data volume, which `/` stands for.
#[tauri::command]
fn volumes() -> Result<Vec<VolumeInfo>, String> {
    let mut buf: *mut libc::statfs = std::ptr::null_mut();
    let n = unsafe { libc::getmntinfo(&mut buf, libc::MNT_NOWAIT) };
    if n <= 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    // getmntinfo owns the buffer and reuses it on the next call; copy out before returning.
    let mounts = unsafe { std::slice::from_raw_parts(buf, n as usize) };
    let mut out: Vec<VolumeInfo> = mounts
        .iter()
        .filter(|st| {
            let flags = st.f_flags as libc::c_int;
            let mount = unsafe { CStr::from_ptr(st.f_mntonname.as_ptr()) }.to_bytes();
            flags & libc::MNT_LOCAL != 0
                && flags & libc::MNT_DONTBROWSE == 0
                && (mount == b"/" || mount.starts_with(b"/Volumes/"))
        })
        .map(volume_from)
        .collect();
    out.sort_by(|a, b| (a.mount != "/").cmp(&(b.mount != "/")).then_with(|| a.name.cmp(&b.name)));
    Ok(out)
}

fn volume_from(st: &libc::statfs) -> VolumeInfo {
    let mount = unsafe { CStr::from_ptr(st.f_mntonname.as_ptr()) }.to_string_lossy().into_owned();
    let bsize = u64::from(st.f_bsize);
    VolumeInfo {
        name: volume_name(&mount),
        total: st.f_blocks * bsize,
        free: st.f_bavail * bsize,
        mount,
    }
}

/// `/Volumes` holds the user-visible names; the boot volume is a symlink to `/`.
/// The boot disk's Data volume (home folders live there) shares the boot disk's name.
fn volume_name(mount: &str) -> String {
    let target = Path::new(if mount == "/System/Volumes/Data" { "/" } else { mount });
    let listed = std::fs::read_dir("/Volumes").ok().and_then(|entries| {
        entries
            .flatten()
            .find(|e| std::fs::canonicalize(e.path()).is_ok_and(|p| p == target))
            .map(|e| e.file_name().to_string_lossy().into_owned())
    });
    listed.unwrap_or_else(|| match target.file_name() {
        Some(n) => n.to_string_lossy().into_owned(),
        None => "Startup Disk".into(),
    })
}

/// Folders under `$HOME` that macOS only lets an app list once it has Full Disk Access.
const TCC_PROBES: [&str; 3] = ["Library/Mail", "Library/Safari", "Library/Messages"];
const PRIVACY_ALL_FILES_URL: &str = "x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles";

/// Granted when any protected folder that exists can be listed; with none present there is nothing to block.
#[tauri::command]
fn full_disk_access() -> bool {
    let home = PathBuf::from(home_dir());
    let mut denied = false;
    for dir in TCC_PROBES.map(|p| home.join(p)) {
        match std::fs::read_dir(&dir) {
            Ok(_) => return true,
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => denied = true,
            Err(_) => {}
        }
    }
    !denied
}

#[tauri::command]
fn open_privacy_settings() -> Result<(), String> {
    std::process::Command::new("open")
        .arg(PRIVACY_ALL_FILES_URL)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

pub fn run() {
    scan_macos::disable_dataless_materialization();
    scan_macos::raise_fd_limit();
    let policy = Policy::for_current_os().unwrap_or_else(|e| panic!("bundled {e}"));
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState { policy, ..AppState::default() })
        .setup(|app| {
            // Double-click on a drag region zooms the window; route that zoom through the
            // animator so the page follows the frame instead of freezing (zoom.rs).
            #[cfg(target_os = "macos")]
            {
                use tauri::Manager;
                match app.get_webview_window("main").map(|w| w.ns_window()) {
                    Some(Ok(ns_window)) => zoom::install(ns_window),
                    Some(Err(e)) => eprintln!("zoom: no NSWindow for main ({e}); zoom stays native"),
                    None => eprintln!("zoom: no main window at setup; zoom stays native"),
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            start_scan,
            cancel_scan,
            get_view,
            node_info,
            largest_files,
            suggestions,
            watch_status,
            reveal,
            trash_nodes,
            home_dir,
            volume_info,
            full_disk_access,
            open_privacy_settings,
            volumes
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scan_fixture() -> (Scan, NodeId, NodeId) {
        let mut arena = Arena::default();
        let root = arena.push(NONE, b"/tmp/root", 0, 0, Flags::DIR);
        let folder = arena.push(root, b"folder", 0, 0, Flags::DIR);
        let file = arena.push(folder, b"file.bin", 10, 0, Flags::empty());
        arena.aggregate();
        let index = ChildIndex::build(&arena);
        (Scan { root: PathBuf::from("/tmp/root"), arena, index, hits: PolicyHits::default() }, folder, file)
    }

    #[test]
    fn resolves_only_paths_inside_the_scan() {
        let (scan, folder, file) = scan_fixture();

        assert_eq!(find_node(&scan, Path::new("/tmp/root")), 0);
        assert_eq!(find_node(&scan, Path::new("/tmp/root/folder")), folder);
        assert_eq!(find_node(&scan, Path::new("/tmp/root/folder/file.bin")), file);
        assert_eq!(find_node(&scan, Path::new("/tmp/other")), NONE);
    }

    #[test]
    fn treats_descendants_of_trashed_nodes_as_gone() {
        let (mut scan, folder, file) = scan_fixture();

        assert_eq!(scan.arena.mark_trashed(folder), 10);
        assert!(in_trash(&scan.arena, folder));
        assert!(in_trash(&scan.arena, file));
        assert_eq!(scan.arena.size[0], 0);
        assert_eq!(scan.arena.items[0], 0);
    }

    const GUARD_POLICY: &str = r#"
        version = 1
        [[rule]]
        id = "keep"
        kind = "protected"
        reason = "Keep it."
        match = { path = "/tmp/root/keep" }
        [[rule]]
        id = "vault"
        kind = "protected"
        reason = "A vault."
        match = { name = ".vault", dir = true }
        [[rule]]
        id = "big"
        kind = "risky"
        reason = "Big file."
        match = { name = "loose.bin" }
    "#;

    #[test]
    fn sweep_refuses_protected_nodes_and_trashes_the_rest() {
        let mut arena = Arena::default();
        let root = arena.push(NONE, b"/tmp/root", 0, 0, Flags::DIR);
        let keep = arena.push(root, b"keep", 0, 0, Flags::DIR);
        let boxed = arena.push(root, b"box", 0, 0, Flags::DIR);
        let loose = arena.push(root, b"loose.bin", 7, 0, Flags::empty());
        let inner = arena.push(keep, b"inner.bin", 3, 0, Flags::empty());
        arena.push(boxed, b".vault", 0, 0, Flags::DIR);
        arena.aggregate();
        let index = ChildIndex::build(&arena);
        let policy = Policy::from_toml(GUARD_POLICY).unwrap();
        let ctx = PolicyContext { home: PathBuf::new(), now: 0 };
        let hits = policy.resolve(&arena, &index, &ctx).stamp(&mut arena);
        let mut scan = Scan { root: PathBuf::from("/tmp/root"), arena, index, hits };

        assert_eq!(guard_of(&policy, &scan, keep), Some(Guard { kind: "protected", reason: "Keep it.".into() }));
        assert_eq!(guard_of(&policy, &scan, loose), Some(Guard { kind: "risky", reason: "Big file.".into() }));
        assert_eq!(guard_of(&policy, &scan, inner), None);

        let (items, refused) = plan_sweep(&policy, &scan, vec![keep, boxed, loose, inner]).unwrap();
        let paths: Vec<PathBuf> = items.iter().map(|(_, p, _, _)| p.clone()).collect();
        assert_eq!(paths, vec![PathBuf::from("/tmp/root/loose.bin"), PathBuf::from("/tmp/root/keep/inner.bin")]);
        assert_eq!(
            refused,
            vec![
                "/tmp/root/keep: protected. Keep it.".to_string(),
                "/tmp/root/box: protected. It contains /tmp/root/box/.vault. A vault.".to_string(),
            ]
        );

        let outcomes = items.into_iter().map(|(id, _, _, size)| (id, size, Ok(()))).collect();
        let result = finish_sweep(Some(&mut scan), outcomes, refused.clone());

        assert_eq!(result.trashed, vec![loose, inner]);
        assert_eq!(result.bytes, 10);
        assert_eq!(result.failed, refused);
        assert!(!scan.arena.flags[keep as usize].contains(Flags::TRASHED));
        assert!(!scan.arena.flags[boxed as usize].contains(Flags::TRASHED));
        assert!(scan.arena.flags[loose as usize].contains(Flags::TRASHED));
    }

    #[test]
    fn scans_a_root_reached_through_a_symlink() {
        use std::fs;
        // Not canonicalized: on macOS the temp dir itself sits under the /var symlink.
        let base = std::env::temp_dir().join(format!("unshift-symroot-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(base.join("real/sub")).unwrap();
        fs::write(base.join("real/sub/f"), [0u8; 4096]).unwrap();
        std::os::unix::fs::symlink(base.join("real"), base.join("link")).unwrap();
        let link = base.join("link");

        let root = resolve_root(link.to_str().unwrap()).unwrap();
        assert_eq!(root, fs::canonicalize(base.join("real")).unwrap());
        let cfg = ScanConfig { root: root.clone(), threads: 2, cross_mounts: false, qos: false };
        let arena = scan_macos::walk(&cfg, &Progress::default(), &Cancel::new(false)).unwrap();
        let index = ChildIndex::build(&arena);
        let scan = Scan { root, arena, index, hits: PolicyHits::default() };

        let f = find_node(&scan, &fs::canonicalize(link.join("sub/f")).unwrap());
        assert_ne!(f, NONE);
        assert_eq!(scan.arena.name(f), b"f");
        assert!(!scan.arena.flags[0].contains(Flags::ERROR));
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn rejects_missing_or_unreadable_roots() {
        use std::os::unix::fs::PermissionsExt;
        let base = std::env::temp_dir().join(format!("unshift-badroot-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();

        let missing = base.join("missing");
        let err = resolve_root(missing.to_str().unwrap()).unwrap_err();
        assert!(err.starts_with(&format!("Can't open {}", missing.display())), "{err}");

        std::fs::set_permissions(&base, std::fs::Permissions::from_mode(0o000)).unwrap();
        let err = resolve_root(base.to_str().unwrap()).unwrap_err();
        std::fs::set_permissions(&base, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(err.starts_with(&format!("Can't read {}", base.display())), "{err}");
        let _ = std::fs::remove_dir_all(&base);
    }

    fn scan_of(root: &Path, arena: Arena) -> Scan {
        let index = ChildIndex::build(&arena);
        Scan { root: root.to_path_buf(), arena, index, hits: PolicyHits::default() }
    }

    fn by_name<'a>(found: &'a [Suggestion], name: &str) -> Option<&'a Suggestion> {
        found.iter().find(|s| s.entry.name == name)
    }

    #[test]
    fn suggests_each_app_cache_but_not_the_caches_folder() {
        // Real directories, so the writability guard sees writable parents.
        let base = std::env::temp_dir().join(format!("unshift-caches-{}", std::process::id()));
        let home = base.join("home");
        std::fs::create_dir_all(home.join("Library/Caches")).unwrap();
        std::fs::create_dir_all(home.join("Library/Developer/CoreSimulator/Caches")).unwrap();

        let mut a = Arena::default();
        let root = a.push(NONE, base.as_os_str().as_encoded_bytes(), 0, 0, Flags::DIR);
        let h = a.push(root, b"home", 0, 0, Flags::DIR);
        let lib = a.push(h, b"Library", 0, 0, Flags::DIR);
        let caches = a.push(lib, b"Caches", 0, 0, Flags::DIR);
        let app = a.push(caches, b"com.example.app", 0, 0, Flags::DIR);
        a.push(app, b"blob", 10, 0, Flags::empty());
        a.push(caches, b"loose.db", 5, 0, Flags::empty());
        let dev = a.push(lib, b"Developer", 0, 0, Flags::DIR);
        let sim = a.push(dev, b"CoreSimulator", 0, 0, Flags::DIR);
        let sim_caches = a.push(sim, b"Caches", 0, 0, Flags::DIR);
        let dyld = a.push(sim_caches, b"dyld", 0, 0, Flags::DIR);
        a.push(dyld, b"shared", 7, 0, Flags::empty());
        a.aggregate();
        let scan = scan_of(&base, a);

        let found = collect_suggestions(&Policy::default(), &scan, &home, 0);
        std::fs::remove_dir_all(&base).unwrap();

        for name in ["com.example.app", "loose.db", "dyld"] {
            let s = by_name(&found, name).unwrap_or_else(|| panic!("{name} not suggested"));
            assert_eq!(s.safety, "safe");
            assert_eq!(s.reason, "App cache. Rebuilt automatically the next time the app runs.");
        }
        assert!(by_name(&found, "Caches").is_none());
        assert!(by_name(&found, "blob").is_none());
        assert!(by_name(&found, "shared").is_none());
    }

    #[test]
    fn skips_var_folders_and_keeps_items_in_protected_directories() {
        let mut a = Arena::default();
        let root = a.push(NONE, b"/", 0, 0, Flags::DIR);
        let private = a.push(root, b"private", 0, 0, Flags::DIR);
        let var = a.push(private, b"var", 0, 0, Flags::DIR);
        let folders = a.push(var, b"folders", 0, 0, Flags::DIR);
        a.push(folders, b"tmpfile", 10, 0, Flags::empty());
        a.push(var, b"Docker.raw", 10, 0, Flags::empty());
        a.aggregate();
        let scan = scan_of(Path::new("/"), a);

        let found = collect_suggestions(&Policy::default(), &scan, Path::new("/nonexistent-home"), 0);

        assert!(by_name(&found, "folders").is_none());
        let docker = by_name(&found, "Docker.raw").expect("Docker.raw suggested");
        assert_eq!(docker.safety, "keep");
        assert_eq!(docker.reason, "Protected by macOS and cannot be moved by Unshift.");
    }

    #[test]
    fn lists_the_boot_volume_first() {
        let list = volumes().unwrap();
        let boot = &list[0];
        assert_eq!(boot.mount, "/");
        assert!(!boot.name.is_empty());
        assert!(boot.total > 0);
        assert!(list.iter().all(|v| v.mount == "/" || v.mount.starts_with("/Volumes/")));
    }
}
