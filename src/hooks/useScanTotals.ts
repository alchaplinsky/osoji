import { useCallback, useEffect, useState } from "react";
import type { ScanTotal } from "../lib/scanEstimate";

const STORAGE_KEY = "osoji.scanTotals";

/** What each root was found to hold by its last completed scan, kept across launches. */
export type ScanTotals = {
  get: (root: string) => ScanTotal | null;
  remember: (root: string, total: ScanTotal) => void;
};

function isTotal(v: unknown): v is ScanTotal {
  return typeof v === "object" && v !== null && typeof (v as ScanTotal).bytes === "number" && typeof (v as ScanTotal).entries === "number";
}

/** Entries from before the entry count was kept were plain byte totals; those start over. */
function load(): Record<string, ScanTotal> {
  try {
    const stored: unknown = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "{}");
    if (!stored || typeof stored !== "object") return {};
    return Object.fromEntries(Object.entries(stored).filter((e): e is [string, ScanTotal] => isTotal(e[1]) && e[1].bytes > 0 && e[1].entries > 0));
  } catch {
    return {};
  }
}

export function useScanTotals(): ScanTotals {
  const [totals, setTotals] = useState<Record<string, ScanTotal>>(load);

  useEffect(() => {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(totals));
  }, [totals]);

  const get = useCallback((root: string) => totals[root] ?? null, [totals]);
  const remember = useCallback((root: string, total: ScanTotal) => {
    if (total.bytes <= 0 || total.entries <= 0) return;
    setTotals((t) => (t[root]?.bytes === total.bytes && t[root]?.entries === total.entries ? t : { ...t, [root]: total }));
  }, []);

  return { get, remember };
}
