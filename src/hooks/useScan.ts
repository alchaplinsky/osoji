import { useCallback, useRef, useState } from "react";
import { cancelScan, startScan, type ProgressMsg, type ScanSummary } from "../lib/api";

/** "stopped": the user cancelled the scan; nothing from it was kept. */
export type ScanStatus = "idle" | "scanning" | "done" | "stopped" | "error";

export function useScan() {
  const [status, setStatus] = useState<ScanStatus>("idle");
  const [progress, setProgress] = useState<ProgressMsg | null>(null);
  const [summary, setSummary] = useState<ScanSummary | null>(null);
  const [error, setError] = useState<string | null>(null);
  const generation = useRef(0);

  const start = useCallback(async (root: string) => {
    const gen = ++generation.current;
    setStatus("scanning");
    setError(null);
    setSummary(null);
    setProgress(null);
    try {
      const s = await startScan(root, (next) => {
        if (gen === generation.current) setProgress(next);
      });
      if (gen !== generation.current) return null;
      setSummary(s);
      setStatus(s.cancelled ? "stopped" : "done");
      return s;
    } catch (e) {
      if (gen !== generation.current) return null;
      setError(String(e));
      setStatus("error");
      return null;
    }
  }, []);

  const cancel = useCallback(() => cancelScan(), []);
  const reset = useCallback(() => {
    generation.current++;
    setStatus("idle");
    setProgress(null);
    setSummary(null);
    setError(null);
  }, []);

  /** After the unreadable folders were retried: how many the scan still cannot read. */
  const setErrors = useCallback((errors: number) => setSummary((s) => (s ? { ...s, errors } : s)), []);

  return { status, progress, summary, error, start, cancel, reset, setErrors };
}
