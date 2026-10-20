#!/usr/bin/env bash
# Warm-cache benchmark of the scan-cli walkers (and optionally dua-cli) on a
# directory. Writes one JSON line per run to tools/bench/results/<date>-<host>.jsonl.
#
#   tools/bench/run_home.sh [ROOT] [RUNS] [THREADS...]
#   DUA=/path/to/dua tools/bench/run_home.sh ~ 5 4 8 12
#   WALKERS="" DUA=/path/to/dua tools/bench/run_home.sh ~ 3 4      # dua only
set -euo pipefail
ROOT="${1:-$HOME}"
RUNS="${2:-5}"
shift 2 2>/dev/null || true
THREADS=("${@:-4 8 12}")
WALKERS="${WALKERS-bulk jwalk}"
[ ${#THREADS[@]} -eq 1 ] && read -ra THREADS <<<"${THREADS[0]}"

HERE="$(cd "$(dirname "$0")" && pwd)"
BIN="$HERE/../../src-tauri/target/release/scan-cli"
OUT="$HERE/results/$(date +%Y%m%d-%H%M)-$(hostname -s).jsonl"
mkdir -p "$HERE/results"
cargo build --release -p scan-cli --manifest-path "$HERE/../../src-tauri/Cargo.toml" >/dev/null

echo "root=$ROOT runs=$RUNS threads=${THREADS[*]} out=$OUT" >&2
for t in "${THREADS[@]}"; do
  for w in $WALKERS; do
    "$BIN" "$ROOT" --walker "$w" --threads "$t" --json >/dev/null   # warm-up, discarded
    for i in $(seq "$RUNS"); do
      "$BIN" "$ROOT" --walker "$w" --threads "$t" --json | tee -a "$OUT" | cut -c1-110 >&2
    done
  done
  if [ -n "${DUA:-}" ]; then
    "$DUA" -t "$t" -f bytes "$ROOT" >/dev/null 2>&1 || true            # warm-up
    for i in $(seq "$RUNS"); do
      start=$(python3 -c 'import time;print(time.time())')
      line=$( { /usr/bin/time -l "$DUA" -t "$t" -f bytes "$ROOT" >/dev/null || true; } 2>&1 )
      end=$(python3 -c 'import time;print(time.time())')
      rss=$(echo "$line" | awk '/maximum resident/ {print $1}')
      ms=$(python3 -c "print(round(($end-$start)*1000,1))")
      mb=$(python3 -c "print(round($rss/1000000,1))")   # decimal MB, as scan-cli reports its own
      echo "{\"walker\":\"dua\",\"threads\":$t,\"elapsed_ms\":$ms,\"max_rss_mb\":$mb}" | tee -a "$OUT" >&2
    done
  fi
done
echo "$OUT"
