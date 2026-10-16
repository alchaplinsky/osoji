//! Tauri backend: owns the scan state and exposes bounded views to the UI.
//! The arena never crosses IPC; commands return only what is on screen.

use scan_core::{Arena, Cancel, ChildIndex, Flags, NodeId, Progress, ScanConfig, NONE};
use serde::Serialize;
use std::borrow::Cow;
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::ffi::{CStr, CString};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::ipc::Channel;
use tauri::State;

struct Scan {
    root: PathBuf,
    arena: Arena,
    index: ChildIndex,
}

#[derive(Default)]
struct AppState {
    scan: Mutex<Option<Scan>>,
    cancel: Mutex<Option<Arc<Cancel>>>,
    running: AtomicBool,
}

#[derive(Serialize, Clone)]
struct ProgressMsg {
    files: u64,
    dirs: u64,
    bytes: u64,
    errors: u64,
    elapsed_ms: u64,
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
    /// Bytes of children merged away because they were too small or past top_k.
    other: u64,
    other_count: u32,
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
}

#[derive(Serialize)]
struct TrashResult {
    freed: u64,
    count: usize,
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

/// Node ids from the root down to the parent of `id`.
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

fn file_entry(arena: &Arena, id: NodeId) -> FileEntry {
    let i = id as usize;
    FileEntry {
        id,
        name: name_of(arena, id),
        path: arena.path(id).to_string_lossy().into_owned(),
        size: arena.size[i],
        mtime: arena.mtime[i],
        dir: arena.flags[i].contains(Flags::DIR),
        ancestors: ancestors_of(arena, id),
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

#[tauri::command]
async fn start_scan(
    state: State<'_, AppState>,
    root: String,
    threads: Option<usize>,
    on_progress: Channel<ProgressMsg>,
) -> Result<ScanSummary, String> {
    if state.running.swap(true, Ordering::AcqRel) {
        return Err("a scan is already running".into());
    }
    let cancel = Arc::new(AtomicBool::new(false));
    *state.cancel.lock().unwrap() = Some(cancel.clone());

    let cfg = ScanConfig {
        root: PathBuf::from(&root),
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
                });
            }
        })
    };

    let result = {
        let (cfg, progress, cancel) = (cfg.clone(), progress.clone(), cancel.clone());
        tauri::async_runtime::spawn_blocking(move || scan_macos::walk(&cfg, &progress, &cancel))
            .await
            .map_err(|e| e.to_string())?
    };
    done.store(true, Ordering::Release);
    let _ = poller.join();
    state.running.store(false, Ordering::Release);

    let arena = result.map_err(|e| e.to_string())?;
    let index = ChildIndex::build(&arena);
    let summary = ScanSummary {
        root: root.clone(),
        entries: arena.len(),
        bytes: arena.size[0],
        errors: progress.errors.load(Ordering::Relaxed),
        elapsed_ms: t0.elapsed().as_millis() as u64,
        cancelled: cancel.load(Ordering::Relaxed),
    };
    *state.scan.lock().unwrap() = Some(Scan { root: cfg.root, arena, index });
    Ok(summary)
}

#[tauri::command]
fn cancel_scan(state: State<'_, AppState>) {
    if let Some(c) = state.cancel.lock().unwrap().as_ref() {
        c.store(true, Ordering::Release);
    }
}

fn build_view(scan: &Scan, id: NodeId, depth: u8, top_k: usize, min_size: u64) -> ViewNode {
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
            node.children.push(build_view(scan, c, depth - 1, top_k, min_size));
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
    Ok(build_view(scan, node, depth, top_k, min_size.max(1)))
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
    })
}

/// The `limit` biggest files and packages, largest first.
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
        // Nothing inside a node is bigger than the node itself.
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
    Ok(heap.into_sorted_vec().into_iter().map(|Reverse((_, id))| file_entry(a, id)).collect())
}

const DAY: i64 = 86_400;
const MIB: u64 = 1024 * 1024;
const UNDER_HOME: u8 = 1;
const UNDER_XCODE: u8 = 2;

/// Well-known locations resolved to node ids once per `suggestions` call.
struct Places {
    home: NodeId,
    xcode: NodeId,
    caches: NodeId,
    sim_caches: NodeId,
    downloads: NodeId,
    var_folders: NodeId,
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
        if id == p.caches {
            return rule("safe", "Apps rebuild their caches automatically. They may open a little slower the first time after.");
        }
        if id == p.sim_caches {
            return rule("safe", "Simulator caches. Rebuilt on demand.");
        }
        if is_package_cache(a, p, id, ctx) {
            return rule("safe", PACKAGE_CACHE);
        }
        if id == p.var_folders {
            return rule("safe", "Temporary files macOS clears on restart.");
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

/// Cleanup candidates by safety, largest first. Flagged nodes are not descended into.
#[tauri::command]
fn suggestions(state: State<'_, AppState>) -> Result<Vec<Suggestion>, String> {
    let guard = state.scan.lock().unwrap();
    let scan = guard.as_ref().ok_or("no scan loaded")?;
    let a = &scan.arena;
    if a.is_empty() {
        return Ok(Vec::new());
    }
    let home = PathBuf::from(std::env::var("HOME").unwrap_or_default());
    let has_home = home.is_absolute();
    let at = |rel: &str| if has_home { find_node(scan, &home.join(rel)) } else { NONE };
    let xcode_path = home.join("Library/Developer/Xcode");
    let from_slash = scan.root == Path::new("/");
    let places = Places {
        home: if has_home { find_node(scan, &home) } else { NONE },
        xcode: at("Library/Developer/Xcode"),
        caches: at("Library/Caches"),
        sim_caches: at("Library/Developer/CoreSimulator/Caches"),
        downloads: at("Downloads"),
        var_folders: if from_slash { find_node(scan, Path::new("/private/var/folders")) } else { NONE },
        var_vm: find_node(scan, Path::new("/private/var/vm")),
        system: find_node(scan, Path::new("/System")),
    };
    // A scan rooted inside home (or Xcode) never contains the home node itself.
    let mut root_ctx = 0;
    if has_home && scan.root.starts_with(&home) {
        root_ctx |= UNDER_HOME;
    }
    if has_home && scan.root.starts_with(&xcode_path) {
        root_ctx |= UNDER_XCODE;
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64);

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
        stack.extend(scan.index.children(id).iter().map(|&c| (c, child_ctx)));
    }
    found.sort_unstable_by(|x, y| y.0.cmp(&x.0));
    found.truncate(200);
    Ok(found
        .into_iter()
        .map(|(_, id, safety, reason)| Suggestion { entry: file_entry(a, id), safety, reason: reason.into_owned() })
        .collect())
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

/// Size of each watched folder in the latest scan; `id: None` when it is outside the scan, missing or trashed.
#[tauri::command]
fn watch_status(state: State<'_, AppState>, paths: Vec<String>) -> Result<Vec<WatchHit>, String> {
    let guard = state.scan.lock().unwrap();
    let scan = guard.as_ref().ok_or("no scan loaded")?;
    let a = &scan.arena;
    Ok(paths
        .into_iter()
        .map(|path| {
            let id = find_node(scan, Path::new(&path));
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
        scan.arena.path(node)
    };
    std::process::Command::new("open")
        .arg("-R")
        .arg(&path)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Move nodes to the Trash and subtract them from the tree. Space is only
/// reclaimed once the Trash is emptied; the UI should say so.
#[tauri::command]
fn trash_nodes(state: State<'_, AppState>, nodes: Vec<NodeId>) -> Result<TrashResult, String> {
    let mut guard = state.scan.lock().unwrap();
    let scan = guard.as_mut().ok_or("no scan loaded")?;
    let mut freed = 0u64;
    let mut count = 0usize;
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
        trash::delete(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        freed += scan.arena.mark_trashed(id);
        count += 1;
        let parent = scan.arena.parent[id as usize];
        let mut p = parent;
        while p != NONE {
            scan.index.resort(&scan.arena, p);
            p = scan.arena.parent[p as usize];
        }
    }
    Ok(TrashResult { freed, count })
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
    let mount = unsafe { CStr::from_ptr(st.f_mntonname.as_ptr()) }.to_string_lossy().into_owned();
    let bsize = st.f_bsize as u64;
    Ok(VolumeInfo {
        name: volume_name(&mount),
        total: st.f_blocks as u64 * bsize,
        free: st.f_bavail as u64 * bsize,
        mount,
    })
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

pub fn run() {
    scan_macos::disable_dataless_materialization();
    scan_macos::raise_fd_limit();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
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
            volume_info
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
