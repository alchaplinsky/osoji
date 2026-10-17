import { useCallback, useEffect, useState } from "react";

const STORAGE_KEY = "unshift.scanTotals";

/** Bytes each root was found to hold by its last completed scan, kept across launches. */
export type ScanTotals = {
  get: (root: string) => number | null;
  remember: (root: string, bytes: number) => void;
};

function load(): Record<string, number> {
  try {
    const stored: unknown = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "{}");
    if (!stored || typeof stored !== "object") return {};
    return Object.fromEntries(Object.entries(stored).filter(([, v]) => typeof v === "number" && v > 0)) as Record<string, number>;
  } catch {
    return {};
  }
}

export function useScanTotals(): ScanTotals {
  const [totals, setTotals] = useState<Record<string, number>>(load);

  useEffect(() => {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(totals));
  }, [totals]);

  const get = useCallback((root: string) => totals[root] ?? null, [totals]);
  const remember = useCallback((root: string, bytes: number) => {
    if (bytes > 0) setTotals((t) => (t[root] === bytes ? t : { ...t, [root]: bytes }));
  }, []);

  return { get, remember };
}
