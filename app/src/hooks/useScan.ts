import { useCallback, useState } from "react";
import { cancelScan, startScan, type ProgressMsg, type ScanSummary } from "../lib/api";

export type ScanStatus = "idle" | "scanning" | "done" | "error";

export function useScan() {
  const [status, setStatus] = useState<ScanStatus>("idle");
  const [progress, setProgress] = useState<ProgressMsg | null>(null);
  const [summary, setSummary] = useState<ScanSummary | null>(null);
  const [error, setError] = useState<string | null>(null);

  const start = useCallback(async (root: string) => {
    setStatus("scanning");
    setError(null);
    setSummary(null);
    setProgress(null);
    try {
      const s = await startScan(root, setProgress);
      setSummary(s);
      setStatus("done");
      return s;
    } catch (e) {
      setError(String(e));
      setStatus("error");
      return null;
    }
  }, []);

  const cancel = useCallback(() => cancelScan(), []);

  return { status, progress, summary, error, start, cancel };
}
