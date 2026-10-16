//! Tauri backend: owns the scan state and exposes bounded views to the UI.
//! The arena never crosses IPC; commands return only what is on screen.

use scan_core::{Arena, Cancel, ChildIndex, Flags, NodeId, Progress, ScanConfig, NONE};
use serde::Serialize;
use std::path::PathBuf;
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

fn name_of(arena: &Arena, id: NodeId) -> String {
    String::from_utf8_lossy(arena.name(id)).into_owned()
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
    let mut ancestors = Vec::new();
    let mut p = a.parent[node as usize];
    while p != NONE {
        ancestors.push(Crumb { id: p, name: name_of(a, p) });
        p = a.parent[p as usize];
    }
    ancestors.reverse();
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
            reveal,
            trash_nodes,
            home_dir
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
