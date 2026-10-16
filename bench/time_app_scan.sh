#!/usr/bin/env bash
# Time a GUI app's scan from the outside by sampling its CPU usage with top.
# The scan "starts" at the first 1 s sample where the app (plus any helper)
# uses >= START_CPU percent, and "ends" after END_SAMPLES consecutive samples
# below END_CPU percent. Precision is about +/- 1 s.
#
#   bench/time_app_scan.sh                 # DaisyDisk (default)
#   bench/time_app_scan.sh DaisyDisk 30 5  # name, start threshold, end threshold
#
# Run it, then click Scan in the app.
set -u
NAME="${1:-DaisyDisk}"
START_CPU="${2:-30}"
END_CPU="${3:-5}"
END_SAMPLES=3

pids=$(pgrep -f "$NAME" | grep -v "^$$\$" | tr '\n' ' ')
[ -z "$pids" ] && { echo "$NAME is not running" >&2; exit 1; }
args=(); n=0; for p in $pids; do args+=(-pid "$p"); n=$((n+1)); done
now() { perl -MTime::HiRes=time -e 'print time'; }
sample() { top -l 2 -s 1 -stats pid,cpu "${args[@]}" 2>/dev/null | tail -n "$n" | awk '$1 ~ /^[0-9]+$/ {s+=$2} END {printf "%d", s}'; }

echo "watching $NAME pids: $pids (start >= ${START_CPU}%, end < ${END_CPU}% for ${END_SAMPLES} s). Click Scan." >&2
while :; do c=$(sample); [ "$c" -ge "$START_CPU" ] && break; done
start=$(now); echo "scan started (cpu ${c}%)" >&2
idle=0; peak=$c
while :; do
  c=$(sample)
  [ "$c" -gt "$peak" ] && peak=$c
  if [ "$c" -lt "$END_CPU" ]; then idle=$((idle+1)); else idle=0; fi
  [ "$idle" -ge "$END_SAMPLES" ] && break
done
end=$(now)
perl -e "printf(\"scan finished: %.1f s (peak cpu %d%%)\n\", $end - $start - $END_SAMPLES, $peak)"
