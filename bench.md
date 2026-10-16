# Benchmark plan

## Corpus (reproducible)
- bench/gen_corpus.rs uses a seeded RNG and writes onto a dedicated APFS sparse image (hdiutil create -size 200g -fs APFS -type SPARSE).
- About 10M entries in a power-law tree: depth up to 20, with a few directories holding 50k+ entries.
- File sizes follow a log-normal distribution and are written sparsely, so allocated size differs from logical size.
- Special cases:
  - 2% hard links (nlink 2–5)
  - 1% clones made with cp -c
  - symlinks, including loops
  - a nested mounted image
  - chmod 000 directories
  - node_modules, target and DerivedData trees
- The manifest JSON records the expected logical, allocated and deduplicated totals.

## Contenders
- A: the new scanner (scan-cli).
- B: a jwalk-only variant (--walker=jwalk).
- C: dua-cli in aggregate mode.
- D: DaisyDisk, timed manually and reported separately.
- Optional references: pdu and du -sk.

## Conditions
- Cold cache: sudo purge before each run. Warm cache: one discarded run first.
- Thread counts of 4, 8 and 16 for A and B.
- 7 runs per condition, reporting the median and p90.
- Record the machine, OS and SSD, run on AC power with Low Power Mode off.

## Metrics
- Scan wall time with hyperfine (--prepare 'sudo purge' -r 7 --export-json).
- Peak RSS from /usr/bin/time -l.
- Time from process start to first frame, both from a cached snapshot and with no cache.
- View latency p50 and p99 over 10k in-process get_view calls plus 500 IPC calls, recorded with hdrhistogram.
- Snapshot load time: mmap, validate and first view.
- Watcher rescan time, from a filesystem change of 1, 1k or 100k files to the "tree updated" event.
- Correctness: totals must match the manifest within 0.1%.

## Output
bench/run.sh writes results/<date>-<host>.csv with the columns:
contender,condition,threads,metric,median,p90,p99,unit,runs,correct

bench/report.py renders this table:

| contender | cache | threads | scan s | RSS MB | first pixel ms | view p50/p99 ms | correct |
|---|---|---|---|---|---|---|---|

## Pass bar for M2
- A is at least 1.5× faster than both B and C on a cold scan.
- Peak RSS stays at or under 1 GB.
- All correctness checks pass.
