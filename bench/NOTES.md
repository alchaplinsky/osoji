# Benchmark notes (2026-10-14, M3 Pro 12-core, macOS 27.0, APFS, warm cache)

## How to run
```
cargo build --release
./target/release/scan-cli ~ --walker bulk --threads 8          # human output
./target/release/scan-cli ~ --walker jwalk --json              # one JSON line
DUA=/path/to/dua bench/run_home.sh ~ 3 4 8 12                  # harness
bench/report.py bench/results/<file>.jsonl                     # markdown table
```
Cold-cache runs need `sudo purge` and are not covered by the harness yet.

## Findings that change the spec

- **getattrlistbulk record layout is not fixed, even with FSOPT_PACK_INVAL_ATTRS.**
  The common group is packed in full (ATTR_CMN_ERROR occupies 4 bytes right
  after the returned-attrs bitmap, name ref at 28, DEVID 36, OBJTYPE 40,
  MODTIME 44 as a 16-byte timespec, FILEID 60). The dir group is only present
  for directories and the file group only for files. Parse the tail with a
  cursor driven by the returned-attrs bitmap. See scan-macos/src/lib.rs.
- **ATTR_DIR_MOUNTSTATUS works in bulk mode** and flags every mount point
  under /System/Volumes, including Data. No fstat per directory is needed to
  stop at mounts.
- **The system and data volumes share one st_dev** on this macOS (16777232
  for /, /System/Volumes/Data, /Users). The firmlink concern in the spec
  review does not apply here: a dev-based "do not cross mounts" check keeps
  /Users reachable from /. Mount-status still reports Data as a mount point,
  so the walker must treat the Data mount specially when scanning from /.
- **Dirs report no file group**, so directory allocated size is 0. du agrees
  on APFS. Totals vs `du -skx ~` matched within 0.02% on a 9M-entry tree
  (the residual is files changing during the scan).
- **Full Disk Access**: without it, ~/Library/Mail, Safari, Messages,
  Cookies, HomeKit, TCC, the Photos library and Notes group container return
  EPERM. The walker records the directory with the ERROR flag and continues.
- **Hard links**: 83k duplicate inodes in the home tree, deduplicated by
  (DEVID, FILEID) in a mutex-protected set touched only when nlink > 1.

## Results: home directory, warm cache, 3 measured runs after 1 warm-up
8.93M entries, 500 GB allocated. Rows exclude Full Disk Access folders.

| walker | threads | scan median s | scan p90 s | vs bulk | peak RSS MB |
|---|---|---|---|---|---|
| bulk  | 4  | 37.9 | 40.6  | 1.00x | 228 |
| dua   | 4  | 45.7 | 46.4  | 1.21x | 116 |
| jwalk | 4  | 58.4 | 100.8 | 1.54x | 257 |
| bulk  | 8  | 23.7 | 24.4  | 1.00x | 236 |
| dua   | 8  | 32.1 | 37.4  | 1.35x | 116 |
| jwalk | 8  | 43.5 | 47.0  | 1.83x | 476 |
| bulk  | 12 | 22.3 | 24.1  | 1.00x | 248 |
| dua   | 12 | 27.3 | 29.9  | 1.22x | 104 |
| jwalk | 12 | 40.1 | 41.0  | 1.80x | 860 |

Reference: `du -skx ~` 227 s single-threaded, 45 MB.

Observations
- Bulk clears the 1.5x bar against the jwalk reference (B) at every thread
  count but not against dua-cli (C): 1.2x to 1.35x.
- Bulk is CPU bound on a warm cache: 8 -> 12 threads gains 6%.
- The jwalk variant's RSS grows with threads (257 -> 860 MB) because jwalk's
  producers run ahead of the single-threaded consumer that builds the arena.
  dua avoids this; our B variant is a weaker reference than dua itself.
- Bulk RSS of ~235 MB for 8.93M nodes is ~26 B/node including names, well
  under the 60 B/node budget.
- All three agree on the total within 0.02%, the residual being files that
  change during a scan of a live home directory.

## Follow-ups measured the same evening

**Thread QoS (user-initiated on workers): no effect on a warm cache.**

| config | threads | median s | p90 s |
|---|---|---|---|
| bulk, default QoS | 8  | 22.7 | 23.2 |
| bulk, user-initiated QoS | 8  | 22.3 | 22.4 |
| bulk, default QoS | 12 | 24.7 | 24.8 |
| bulk, user-initiated QoS | 12 | 23.9 | 26.7 |

Within noise. Busy threads at default QoS already land on P-cores. Kept as a
flag (`--no-qos` to disable) in case it matters under load or on a cold cache.

**Full-disk scan from `/`, 8 threads, warm cache:**
10.64M entries, 601.5 GB allocated, 29.6 s, 324 MB peak RSS, 871 EPERM dirs
(no Full Disk Access). `df` reports 631 GB used; the ~30 GB gap is purgeable
space, local Time Machine snapshots, swap, Preboot, and the protected folders.

**Firmlink double counting.** Descending into the `/System/Volumes/Data`
mount from `/` double counts: Users, Applications, Library, private, opt and
usr/local are firmlinks into Data and already appear at the top level. The
first attempt reported 20.8M entries and 1.15 TB. Rule: never descend into a
mount point by default, even when it shares the root's device ID. The
user-facing root is `/`, and the sealed system volume's own files (~35 GB
under /System) come along without special handling.

For comparison DaisyDisk was observed at 67 s for a full-disk scan on this
machine, cache state unknown. A fair comparison needs cold vs cold
(`sudo purge` first) or DaisyDisk run twice back to back.

## Cold cache, full disk from `/`, 8 threads (2026-10-15, `sudo purge` before each)
| run | elapsed | peak RSS |
|---|---|---|
| 1 | 33.6 s | 321 MB |
| 2 | 34.1 s | 332 MB |
| 3 | 36.7 s | 341 MB |

Warm was 29.6 s, so a purged cache costs only 15 to 25%. APFS metadata on
the internal NVMe is fast enough that the scan stays mostly CPU bound.
DaisyDisk's observed 67 s is about twice our cold time; its warm number is
still unmeasured.

## DaisyDisk 4.34.2 on the same disk, timed externally with bench/time_app_scan.sh
| run | elapsed | peak CPU |
|---|---|---|
| cold (`sudo purge` first) | 79.0 s | 382% |
| warm | 80.7 s | 770% |

Full disk, Macintosh HD, DaisyDisk has Full Disk Access so it also walks the
protected folders we skip. Our bulk walker on the same scan: 34 s cold,
30 s warm, roughly 2.4x faster. DaisyDisk shows no warm-cache benefit at all,
which suggests its time is dominated by its own tree building, not I/O.
