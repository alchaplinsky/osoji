# unshift

Open-source disk explorer and cleaner for macOS: a DaisyDisk-style sunburst
on top of a scanner that walks 10M entries in about 30 s. See `docs/spec.md`
for the design, `docs/bench.md` for the benchmark plan and
`tools/bench/NOTES.md` for results.

```
src/                          React + Vite UI: design tokens (src/design), primitives (src/components/ui), screens (src/screens)
src-tauri/                    Tauri 2 backend and Cargo workspace root: owns the scan, serves bounded views, reveal, trash
src-tauri/crates/scan-core    arena, flags, child index, hard-link dedup, progress, jwalk reference walker
src-tauri/crates/scan-macos   getattrlistbulk walker with a work-stealing deque (the fast one)
src-tauri/crates/scan-cli     headless scanner used for benchmarks
tools/bench/                  harness, report, external app timer
docs/                         spec and benchmark plan
```

## Run the app
```
bun install
bun tauri dev           # dev build with hot reload
bun tauri build         # .app and .dmg under src-tauri/target/release/bundle
```

## Run the scanner alone
```
cargo build --release -p scan-cli --manifest-path src-tauri/Cargo.toml
src-tauri/target/release/scan-cli ~ --threads 8
src-tauri/target/release/scan-cli / --json
```

The app links the scanner crates directly. Nothing of the tree crosses IPC;
`get_view` returns only the slices that are on screen. Trashing goes through
the system Trash and never deletes permanently.
