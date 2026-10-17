import type { Volume } from "./types";

/** Highest fraction shown while a scan is still running, so the ring never reads as finished early. */
const RUNNING_CAP = 0.99;

/**
 * Bytes a scan of `root` is expected to find, or null when there is nothing to estimate from.
 * A previous scan of the same root is the best guide. Without one, the volume root falls back
 * to the used space reported by statfs, which overshoots: it also counts the other volumes in
 * the APFS container, local snapshots, purgeable space and folders the walk cannot read.
 * The walker itself never knows the total up front.
 */
export function expectedBytes(volume: Volume | null, root: string, lastBytes: number | null): number | null {
  if (lastBytes) return lastBytes;
  if (volume && root === volume.mount && volume.total > volume.free) return volume.total - volume.free;
  return null;
}

/**
 * Estimated completion of a running scan as a fraction from 0 to 0.99, or null when
 * `expected` is unknown. Clones, hard links and unreadable folders make the found
 * total drift from the expected one, so this is a guide, never an exact figure.
 */
export function scanFraction(found: number, expected: number | null): number | null {
  if (expected === null) return null;
  return Math.max(0, Math.min(RUNNING_CAP, found / expected));
}
