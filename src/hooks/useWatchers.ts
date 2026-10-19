import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { watchStatus, type WatchHit } from "../lib/api";
import type { Watcher, WatcherStatus } from "../lib/types";

const STORAGE_KEY = "osoji.watchers";

export const DEFAULT_WATCH_LIMIT = 10 * 2 ** 30;

export type Watchers = {
  watchers: Watcher[];
  statuses: WatcherStatus[];
  overCount: number;
  add: (path: string, limit?: number) => void;
  remove: (path: string) => void;
  setLimit: (path: string, limit: number) => void;
  refresh: () => Promise<void>;
  reset: () => void;
};

function isWatcher(v: unknown): v is Watcher {
  const w = v as Watcher;
  return typeof w?.path === "string" && typeof w.limit === "number" && w.limit > 0;
}

function load(): Watcher[] {
  try {
    const stored: unknown = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "[]");
    return Array.isArray(stored) ? stored.filter(isWatcher) : [];
  } catch {
    return [];
  }
}

function normalize(path: string): string {
  return path.length > 1 ? path.replace(/\/+$/, "") : path;
}

export function useWatchers(): Watchers {
  const [watchers, setWatchers] = useState<Watcher[]>(load);
  const [hits, setHits] = useState<ReadonlyMap<string, WatchHit> | null>(null);
  const latest = useRef(watchers);
  const generation = useRef(0);

  useEffect(() => {
    latest.current = watchers;
    localStorage.setItem(STORAGE_KEY, JSON.stringify(watchers));
  }, [watchers]);

  const refresh = useCallback(async () => {
    const paths = latest.current.map((w) => w.path);
    const gen = ++generation.current;
    if (paths.length === 0) {
      setHits(new Map());
      return;
    }
    const result = await watchStatus(paths);
    if (gen === generation.current) setHits(new Map(result.map((h) => [h.path, h])));
  }, []);

  const reset = useCallback(() => {
    generation.current++;
    setHits(null);
  }, []);

  useEffect(() => {
    if (hits !== null && watchers.some((w) => !hits.has(w.path))) refresh().catch(console.error);
  }, [hits, watchers, refresh]);

  const add = useCallback((path: string, limit = DEFAULT_WATCH_LIMIT) => {
    const p = normalize(path);
    setWatchers((ws) => (ws.some((w) => w.path === p) ? ws : [...ws, { path: p, limit }]));
  }, []);

  const remove = useCallback((path: string) => {
    setWatchers((ws) => (ws.some((w) => w.path === path) ? ws.filter((w) => w.path !== path) : ws));
  }, []);

  const setLimit = useCallback((path: string, limit: number) => {
    setWatchers((ws) => (ws.some((w) => w.path === path && w.limit !== limit) ? ws.map((w) => (w.path === path ? { ...w, limit } : w)) : ws));
  }, []);

  const statuses = useMemo<WatcherStatus[]>(
    () =>
      watchers.map((w) => {
        const hit = hits?.get(w.path);
        return { ...w, id: hit?.id ?? null, size: hit?.size ?? 0, dir: hit?.dir ?? false };
      }),
    [watchers, hits],
  );

  const overCount = useMemo(() => statuses.filter((s) => s.size > s.limit).length, [statuses]);

  return { watchers, statuses, overCount, add, remove, setLimit, refresh, reset };
}
