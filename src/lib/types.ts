export type Screen = "scan" | "breakdown" | "largest" | "suggest" | "settings";

/** A mounted volume as reported by the `volume_info` command. Sizes in bytes. */
export type Volume = { name: string; mount: string; total: number; free: number };

/** Identity of a hovered chart slice: a node id, the merged "other" slice, or nothing. */
export type HoverId = number | "other" | null;
