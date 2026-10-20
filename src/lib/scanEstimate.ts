import type { ProgressMsg } from "./api";
import type { Volume } from "./types";

/** Highest fraction shown while a scan is still running, so the ring never reads as finished early. */
const RUNNING_CAP = 0.99;

/** What a completed scan of a root found, remembered to pace the next one. */
export type ScanTotal = { bytes: number; entries: number };

/** What a scan is expected to find. `entries` is null when only the byte total can be guessed. */
export type ScanExpectation = { bytes: number; entries: number | null };

/**
 * What a scan of `root` is expected to find, or null when there is nothing to estimate from.
 * A previous scan of the same root is the best guide. Without one, the volume root falls back
 * to the used space reported by statfs, which overshoots: it also counts the other volumes in
 * the APFS container, local snapshots, purgeable space and folders the walk cannot read.
 * The walker itself never knows the total up front.
 */
export function expectedScan(volume: Volume | null, root: string, last: ScanTotal | null): ScanExpectation | null {
  if (last) return { bytes: last.bytes, entries: last.entries };
  if (volume && root === volume.mount && volume.total > volume.free) return { bytes: volume.total - volume.free, entries: null };
  return null;
}

/**
 * Estimated completion of a running scan as a fraction from 0 to 0.99, or null when
 * `expected` is unknown. Scan time goes with entries, not bytes: a few large files put the
 * byte share near the end early, while the walker still has millions of small files to read.
 * So the fraction is the smaller of the two shares, when an entry count is known: never more
 * space than has been found, and never near the end before the count is. Clones, hard links
 * and unreadable folders make both drift from the expected ones, so this is a guide, never
 * an exact figure.
 */
export function scanFraction(found: Pick<ProgressMsg, "bytes" | "files" | "dirs"> | null, expected: ScanExpectation | null): number | null {
  if (expected === null) return null;
  if (found === null) return 0;
  const byBytes = expected.bytes > 0 ? found.bytes / expected.bytes : 1;
  const byEntries = expected.entries ? (found.files + found.dirs) / expected.entries : byBytes;
  return Math.max(0, Math.min(RUNNING_CAP, byBytes, byEntries));
}
