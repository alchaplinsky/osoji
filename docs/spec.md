# Spec: Open-source DaisyDisk-like disk scanner (Rust + Tauri 2)

## Goals
- Scan about 10M entries on macOS faster than dua-cli, using ≤ 60 B/node (hot+cold, names not counted).
- First pixel from a cached snapshot in under 300 ms. Live freshness via FSEvents.
- Interactive sunburst with drill-down, a cleanup collector, and deletes to the Trash.
- macOS first, then Linux, then Windows.

## Non-goals (v1)
- Reading file contents (duplicate hashing is opt-in and runs outside the scan).
- Raw APFS or block-device parsing.
- Scanning network volumes by default.

## Workspace layout
```
src/                 # TS, canvas/WebGL sunburst, d3-hierarchy
src-tauri/           # Cargo workspace root; commands, channels, URI scheme
  crates/
    scan-core/       # arena, interner, aggregation, views, snapshot
    scan-macos/      # getattrlistbulk walker, FSEvents, TCC checks
    scan-linux/      # getdents64 + statx (rustix)
    scan-windows/    # FindFirstFileExW; opt-in MFT reader
    scan-cli/        # headless engine binary for benchmarks
tools/bench/         # corpus generator, harness, reports
```

## Crates
- Core: rkyv (with validation), memmap2, crossbeam-deque, crossbeam-channel, parking_lot, globset, bitflags, thiserror, tracing.
- macOS: libc, notify, notify-debouncer-full, trash.
- Linux: rustix.
- Windows: windows-sys, plus an MFT reader (opt-in).
- App: tauri v2, serde.
- UI: d3-hierarchy, Canvas2D first, regl for WebGL later.

## Core types
```rust
pub type NodeId = u32; // u32::MAX = NONE
bitflags! { pub struct Flags: u8 {
  const DIR=1; const SYMLINK=2; const HARDLINK_DUP=4; const MOUNT=8;
  const EXCLUDED=16; const ERROR=32; const CLONED=64; const SMALL_BUCKET=128; } }

pub struct Arena {
  // HOT
  pub size: Vec<u64>,           // allocated bytes, aggregated for dirs
  pub parent: Vec<NodeId>,
  pub first_child: Vec<NodeId>, // children contiguous
  pub child_count: Vec<u32>,
  // COLD
  pub name_off: Vec<u32>, pub name_len: Vec<u16>,
  pub mtime: Vec<i64>, pub items: Vec<u32>, pub flags: Vec<Flags>,
}
pub struct Interner { buf: Vec<u8> }
pub struct ScanConfig { root: PathBuf, excludes: GlobSet, cross_mounts: bool,
  small_file_bucket: u64, threads: usize, clone_pass: bool }
pub struct ScanCtl { cancel: Arc<AtomicBool>, progress: Arc<Progress> }
pub struct Progress { files: AtomicU64, bytes: AtomicU64, dirs: AtomicU64, errors: AtomicU64 }
```

## Invariants
- A directory's children occupy one contiguous block, sorted by size descending after aggregation.
- Hard links: entries with nlink > 1 are deduplicated by (dev, ino). The first occurrence counts its size; later ones get HARDLINK_DUP and size 0.
- Size always means allocated size.
- Never follow symlinks.
- Never cross st_dev unless cross_mounts is set.
- Skip network and autofs filesystems by default, detected via statfs f_fstypename.

## macOS walker
- Each thread has a 256 KB buffer. The attrlist requests RETURNED_ATTRS, NAME, OBJTYPE, DEVID, FILEID, MODTIME, FILE_LINKCOUNT and FILE_ALLOCSIZE, with FSOPT_PACK_INVAL_ATTRS.
- A task is a directory NodeId plus an fd from openat(O_DIRECTORY|O_NOFOLLOW).
- Each worker pops a task from the crossbeam-deque (stealing from others when empty), then:
  1. Loops getattrlistbulk until it returns 0.
  2. Parses entries straight from the buffer, copying name bytes into a thread-local slab (no String or PathBuf allocations).
  3. Reserves one contiguous arena block and writes the children into it, committing names with one memcpy per block.
  4. Pushes the subdirectories as new tasks.
- EACCES and EPERM set the ERROR flag and increment the error counter; they never abort the scan.
- Check the cancel flag between batches.
- Run inside spawn_blocking with min(cores, 8) workers (tunable).

## Aggregation
- Each directory keeps an atomic count of pending subdirectories. When the last one finishes, sum sizes and items, sort the child block, and propagate to the parent. This is post-order aggregation during the scan, so no separate pass.
- Incremental updates rescan a subtree, compute the size delta and apply it along the parent chain, then re-sort the affected siblings.

## Optional clone-accounting pass
- Off by default.
- Queries clone/private-size attrs and reports shared vs unique bytes.

## Snapshot and cache
- Format: an rkyv archive of Arena + Interner, with a header {version, root, scan_time, fsevents_event_id, config_hash}.
- Location: ~/Library/Caches/<app>/snapshots/<hash>.rkyv.
- Load by mmap, validate once, and serve views directly from the archived data. Copy into a mutable arena only when an update arrives.
- Write atomically: temp file, then rename.

## Freshness
- notify (FSEvents) through notify-debouncer-full with a 500 ms window.
- Map event paths to nodes, mark those directories dirty, and rescan them shallowly.
- On startup, replay FSEvents from the saved event ID. If that's unavailable, fall back to walking directory mtimes.
- On MustScanSubDirs or an overflow event, rescan the full subtree.

## TCC / Full Disk Access
- Probe a protected path at startup. On EPERM, show a banner that opens x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles.
- Unreadable directories show a lock badge, and the app never crashes on them.

## Tauri bridge
Commands:
- start_scan streams progress over an ipc::Channel at 10–20 Hz.
- cancel_scan
- get_view(node, depth, top_k) returns ipc::Response bytes.
- node_info, reveal, trash, suggestions, compare

View binary format (little-endian):
```
header: u32 count, u32 names_len
node: u32 id, u32 parent_idx, u64 size, u32 items, u8 flags, u8 depth, u16 name_len, u32 name_off
names: utf8 blob
```
- Each directory returns its top K children; everything else is merged into one synthetic "other" node.
- Targets: K=64, depth=4, at most 5k nodes and 2 ms on the backend.
- Alternative transport: a scan:// URI scheme read via fetch() as an ArrayBuffer. Use whichever benchmarks faster.

## Frontend
- Decode the view into typed arrays.
- Lay out with d3 partition for the sunburst and squarified treemap() for the treemap toggle.
- Draw with Canvas2D. Move to WebGL if a 5k-node frame takes over 8 ms.
- Interactions: hover, drill-down, breadcrumbs, back/forward, collector.
- Compare mode colors the diff by growth and shrink.

## Cleanup rules
- Developer caches: node_modules, target/, DerivedData, .gradle, ~/Library/Caches, Docker.raw, and package-manager caches.
- Files over 500 MB not modified in 180 days.
- Downloads older than 90 days.
- Duplicates, on demand: group by size, then a 4 KB blake3 prefix hash, then a full blake3 hash.
- Each suggestion shows a reason and the bytes it would free. Nothing is deleted automatically.

## Deletion
- trash::delete_all only, with a confirmation that shows the total size, then apply a negative size delta to the tree.
- No permanent delete in v1.

## Linux
- getdents64 plus statx(AT_STATX_DONT_SYNC; blocks, ino, nlink, mtime, type).
- inotify via notify. When max_user_watches is hit, degrade to the mtime walk and show that in the UI.

## Windows
- FindFirstFileExW(FindExInfoBasic, LARGE_FETCH).
- Opt-in MFT reader for elevated scans of NTFS volumes.
- ReadDirectoryChangesW via notify, with a subtree rescan on overflow.

## Acceptance criteria
1. Totals match du -sk (with hard links deduplicated) within 0.1% on the corpus.
2. No symlink is followed and no mount is crossed by default.
3. Cancel stops the workers in under 100 ms.
4. Progress updates at 10–20 Hz without blocking the UI.
5. Warm first pixel is under 300 ms at 10M nodes.
6. View p99 is under 16 ms end to end.
7. Peak RSS is ≤ 1.0 GB at 10M nodes.
8. A change under a 1M-node subtree shows in the UI within 2 s.
9. Deletes go to the Trash and the totals update.
10. Without Full Disk Access, the app still runs and shows the locked directories plus a prompt.
11. The scanner never reads file contents (checked with fs_usage).

## Milestones
- M1: core, with a jwalk reference walker, the CLI and tests.
- M2: macOS getattrlistbulk walker with in-scan aggregation. It must beat M1.
- M3: Tauri app with progress, binary views and the sunburst.
- M4: mmap snapshot, FSEvents, mtime fallback and TCC handling.
- M5: cleanup rules, collector, trash and compare.
- M6: clone accounting.
- M7: Linux.
- M8: Windows, including MFT.
