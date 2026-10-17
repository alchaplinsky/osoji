import type { CSSProperties } from "react";
import type { ViewNode } from "../../../lib/api";
import type { Volume } from "../../../lib/types";
import { humanCount } from "../../../lib/format";

/** The "Not scanned" entry the bar and legend draw after the scanned slices. */
export type NotScanned = { size: number; title: string; color: string; dimmed: boolean };

/** Stripe colour for the `.hatched` class in CapacityBar.module.css. */
export const hatch = (color: string) => ({ "--hatch": color }) as CSSProperties;

/**
 * Used space a volume-root scan could not see: local snapshots, purgeable space, swap, Preboot and
 * folders unreadable without Full Disk Access. 0 when `root` is not the volume root, or when the
 * scan counted more than `used` (clones and hard links counted once per path).
 */
export function hiddenBytes(volume: Volume, root: ViewNode): number {
  if (root.name !== volume.mount) return 0;
  return Math.max(0, volume.total - volume.free - root.size);
}

/** Tooltip for the hidden segment; mentions the unreadable folder count when there is one. */
export function hiddenNote(unreadable?: number): string {
  const note = "Not in the scan: local snapshots, purgeable space, swap and folders Unshift could not read.";
  if (!unreadable || unreadable <= 0) return note;
  return `${note} ${humanCount(unreadable)} ${unreadable === 1 ? "folder" : "folders"} could not be read.`;
}
