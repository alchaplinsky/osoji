#!/usr/bin/env python3
"""Summarize a results .jsonl into a markdown table: median and p90 per walker/threads."""
import json, sys, statistics
from collections import defaultdict

path = sys.argv[1]
rows = [json.loads(l) for l in open(path) if l.strip()]
groups = defaultdict(list)
for r in rows:
    label = r["walker"] + ("+qos" if r.get("qos") else "")
    groups[(label, r["threads"])].append(r)

def pct(xs, p):
    xs = sorted(xs)
    k = (len(xs) - 1) * p
    lo, hi = int(k), min(int(k) + 1, len(xs) - 1)
    return xs[lo] + (xs[hi] - xs[lo]) * (k - lo)

base = {}
for (w, t), rs in groups.items():
    if w == "bulk":
        base[t] = statistics.median(r["elapsed_ms"] for r in rs)

print("| walker | threads | runs | scan median s | scan p90 s | vs bulk | RSS MB | entries | total bytes |")
print("|---|---|---|---|---|---|---|---|---|")
for (w, t) in sorted(groups, key=lambda k: (k[1], k[0] != "bulk", k[0])):
    rs = groups[(w, t)]
    el = [r["elapsed_ms"] for r in rs]
    med = statistics.median(el)
    ratio = f"{med / base[t]:.2f}x" if t in base else ""
    rss = statistics.median(r.get("max_rss_mb", 0) for r in rs)
    ent = rs[0].get("entries", "")
    tot = rs[0].get("bytes", "")
    print(f"| {w} | {t} | {len(rs)} | {med/1000:.1f} | {pct(el, 0.9)/1000:.1f} | {ratio} | {rss:.0f} | {ent} | {tot} |")
