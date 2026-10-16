# unshift

Open-source disk explorer and cleaner for macOS: a DaisyDisk-style sunburst
on top of a scanner that walks 10M entries in about 30 s. See `spec.md` for
the design, `bench.md` for the benchmark plan and `bench/NOTES.md` for results.

```
crates/scan-core    arena, flags, child index, hard-link dedup, progress, jwalk reference walker
crates/scan-macos   getattrlistbulk walker with a work-stealing deque (the fast one)
crates/scan-cli     headless scanner used for benchmarks
app/src-tauri       Tauri 2 backend: owns the scan, serves bounded views, reveal, trash
app/src             React + Vite UI: scan controls, breadcrumbs, canvas sunburst, node details
bench/              harness, report, external app timer
```

## Run the app
```
cd app
pnpm install
pnpm tauri dev          # dev build with hot reload
pnpm tauri build        # .app and .dmg under target/release/bundle
```

## Run the scanner alone
```
cargo build --release
./target/release/scan-cli ~ --threads 8
./target/release/scan-cli / --json
```

The app links the scanner crates directly. Nothing of the tree crosses IPC;
`get_view` returns only the slices that are on screen. Trashing goes through
the system Trash and never deletes permanently.
