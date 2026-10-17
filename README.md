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

Releases are published to GitHub Releases and installed apps update themselves
from there; see `docs/RELEASING.md`.

## Lint and test
```
bun run lint            # eslint + tsc
bun run test            # vitest unit tests (src/**/*.test.ts)
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo clippy --workspace --all-targets --manifest-path src-tauri/Cargo.toml -- -D warnings
cargo test --workspace --manifest-path src-tauri/Cargo.toml
bun run test:e2e:build  # debug build, then WebdriverIO drives the real app (tests/e2e)
```

The e2e suite talks to a WebDriver server that `tauri-plugin-webdriver` runs
on port 4445 in builds made with `--features webdriver`, never in release
bundles. It starts Vite and the app itself and points the app's `HOME` at a
small fixture folder. `.github/workflows/ci.yml` runs
all of the above on pull requests and on `main`. The frontend checks and the
`scan-core` / `scan-policy` crates run on Linux; the app crate and
`scan-macos` only build on macOS (the walker is `getattrlistbulk`), so their
tests and the e2e suite run on a macOS runner.

## Run the scanner alone
```
cargo build --release -p scan-cli --manifest-path src-tauri/Cargo.toml
src-tauri/target/release/scan-cli ~ --threads 8
src-tauri/target/release/scan-cli / --json
```

The app links the scanner crates directly. Nothing of the tree crosses IPC;
`get_view` returns only the slices that are on screen. Trashing goes through
the system Trash and never deletes permanently.
