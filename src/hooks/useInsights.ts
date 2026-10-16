import { useCallback, useMemo, useRef, useState } from "react";
import { largestFiles, suggestions as fetchSuggestions } from "../lib/api";
import type { FileEntry, Suggestion } from "../lib/types";

export function useInsights() {
  const [largest, setLargest] = useState<FileEntry[]>([]);
  const [suggestions, setSuggestions] = useState<Suggestion[]>([]);
  const generation = useRef(0);

  const load = useCallback(async () => {
    const gen = ++generation.current;
    const [l, s] = await Promise.all([largestFiles(100), fetchSuggestions()]);
    if (gen !== generation.current) return;
    setLargest(l);
    setSuggestions(s);
  }, []);

  const reset = useCallback(() => {
    generation.current++;
    setLargest([]);
    setSuggestions([]);
  }, []);

  const safe = useMemo(() => suggestions.filter((s) => s.safety === "safe"), [suggestions]);
  const safeIds = useMemo<ReadonlySet<number>>(() => new Set(safe.map((s) => s.id)), [safe]);
  const safeTotal = useMemo(() => safe.reduce((sum, s) => sum + s.size, 0), [safe]);

  return { largest, suggestions, safeIds, safeTotal, load, reset };
}
