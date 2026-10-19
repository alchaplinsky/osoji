const UNITS = ["B", "KB", "MB", "GB", "TB", "PB"];

export function humanBytes(bytes: number): string {
  let v = bytes;
  let i = 0;
  while (v >= 1024 && i < UNITS.length - 1) {
    v /= 1024;
    i++;
  }
  return i === 0 ? `${bytes} B` : `${v.toFixed(v < 10 ? 2 : 1)} ${UNITS[i]}`;
}

export function humanCount(n: number): string {
  return n.toLocaleString();
}

export function humanDuration(ms: number): string {
  return ms < 1000 ? `${ms} ms` : `${(ms / 1000).toFixed(1)} s`;
}

/** A share in percent: one decimal below 10%, whole numbers above. */
export function humanPercent(pct: number): string {
  return `${pct.toFixed(pct < 10 ? 1 : 0)}%`;
}
