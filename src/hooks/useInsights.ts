import { useCallback, useMemo, useState } from "react";
import { largestFiles, suggestions as fetchSuggestions } from "../lib/api";
import type { FileEntry, Suggestion } from "../lib/types";

/** Cleanup queries over a finished scan: the largest files and the rule-based suggestions. */
export function useInsights() {
  const [largest, setLargest] = useState<FileEntry[]>([]);
  const [suggestions, setSuggestions] = useState<Suggestion[]>([]);

  const load = useCallback(async () => {
    const [l, s] = await Promise.all([largestFiles(100), fetchSuggestions()]);
    setLargest(l);
    setSuggestions(s);
  }, []);

  const reset = useCallback(() => {
    setLargest([]);
    setSuggestions([]);
  }, []);

  const safe = useMemo(() => suggestions.filter((s) => s.safety === "safe"), [suggestions]);
  const safeIds = useMemo<ReadonlySet<number>>(() => new Set(safe.map((s) => s.id)), [safe]);
  const safeTotal = useMemo(() => safe.reduce((sum, s) => sum + s.size, 0), [safe]);

  return { largest, suggestions, safeIds, safeTotal, load, reset };
}
