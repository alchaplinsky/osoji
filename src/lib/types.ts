export type Screen = "scan" | "breakdown" | "largest" | "suggest" | "watchers" | "settings";

export type Viz = "sunburst" | "treemap" | "shelf" | "orbit";

export type Volume = { name: string; mount: string; total: number; free: number };

export type HoverId = number | "other" | null;

export type BinItem = { id: number; name: string; path: string; size: number; dir: boolean; ancestors: number[] };

export type FileEntry = { id: number; name: string; path: string; size: number; mtime: number; dir: boolean; ancestors: number[] };

export type Safety = "safe" | "review" | "keep";

export type Suggestion = FileEntry & { safety: Safety; reason: string };

export type Watcher = { path: string; limit: number };

export type WatcherStatus = Watcher & { id: number | null; size: number; dir: boolean };
