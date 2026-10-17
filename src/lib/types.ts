export type Screen = "scan" | "breakdown" | "largest" | "suggest" | "watchers" | "settings";

export type Viz = "sunburst" | "treemap" | "shelf" | "orbit";

export type Volume = { name: string; mount: string; total: number; free: number };

export type HoverId = number | "other" | null;

/** Why a node needs care, from the backend policy: protected items and the Trash never reach the bin, risky ones ask twice. */
export type Guard = { kind: "protected" | "risky" | "unexpected" | "trash"; reason: string };

export type BinItem = { id: number; name: string; path: string; size: number; dir: boolean; ancestors: number[]; guard?: Guard | null };

export type FileEntry = { id: number; name: string; path: string; size: number; mtime: number; dir: boolean; ancestors: number[]; guard?: Guard | null };

export type Safety = "safe" | "review" | "keep";

export type Suggestion = FileEntry & { safety: Safety; reason: string };

export type Watcher = { path: string; limit: number };

export type WatcherStatus = Watcher & { id: number | null; size: number; dir: boolean };
