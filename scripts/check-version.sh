#!/usr/bin/env bash
#
# Assert that the one product version is the same everywhere it is declared,
# and print it. `src-tauri/tauri.conf.json` is the authority: the Release
# workflow derives the tag (`v__VERSION__`) from it and the updater compares
# against it. The other two are what something else reports — the Rust
# workspace version is what the binary carries, package.json is what npm
# tooling shows — so one left behind is a release that reports a number no
# release ever had.
#
# Runs in CI on every PR and at the top of the Release workflow and
# scripts/release-local.sh. Pure shell + jq, no toolchain needed.

set -euo pipefail

cd "$(dirname "$0")/.."

version="$(jq -r .version src-tauri/tauri.conf.json)"
if [[ -z "$version" || "$version" == "null" ]]; then
  echo "error: no version in src-tauri/tauri.conf.json" >&2
  exit 1
fi

status=0

# [workspace.package] version in src-tauri/Cargo.toml; every crate inherits it.
cargo_version="$(awk '/^\[workspace\.package\]/{p=1;next} /^\[/{p=0} p&&/^version *=/{gsub(/^version *= *"|"$/,"");print;exit}' src-tauri/Cargo.toml)"
if [[ "$cargo_version" != "$version" ]]; then
  echo "error: src-tauri/Cargo.toml [workspace.package] is $cargo_version, but src-tauri/tauri.conf.json is $version" >&2
  status=1
fi

npm_version="$(jq -r .version package.json)"
if [[ "$npm_version" != "$version" ]]; then
  echo "error: package.json is $npm_version, but src-tauri/tauri.conf.json is $version" >&2
  status=1
fi

if [[ $status -ne 0 ]]; then
  echo "All three must agree; see docs/RELEASING.md." >&2
  exit $status
fi

echo "$version"
