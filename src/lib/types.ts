export type Screen = "scan" | "breakdown" | "largest" | "suggest" | "watchers" | "settings";

export type Viz = "sunburst" | "treemap" | "shelf" | "orbit";

/** A mounted volume as reported by the `volume_info` command. Sizes in bytes. */
export type Volume = { name: string; mount: string; total: number; free: number };

/** Identity of a hovered chart slice: a node id, the merged "other" slice, or nothing. */
export type HoverId = number | "other" | null;

/** A node queued for removal. `ancestors` are node ids from the scan root down to the parent. */
export type BinItem = { id: number; name: string; path: string; size: number; dir: boolean; ancestors: number[] };

/** A node returned by a cleanup query. `mtime` is Unix seconds; `ancestors` as in BinItem. */
export type FileEntry = { id: number; name: string; path: string; size: number; mtime: number; dir: boolean; ancestors: number[] };

export type Safety = "safe" | "review" | "keep";

export type Suggestion = FileEntry & { safety: Safety; reason: string };

/** A folder the user keeps an eye on. `limit` in bytes. */
export type Watcher = { path: string; limit: number };

/** The watcher's folder as found in the latest scan; `id` is null when the path is outside or missing. */
export type WatcherStatus = Watcher & { id: number | null; size: number; dir: boolean };
