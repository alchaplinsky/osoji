import type { ThemeName } from "../../design/theme";
import type { ScanStatus } from "../../hooks/useScan";
import { useSmoothed } from "../../hooks/useSmoothed";
import type { ProgressMsg, ScanSummary } from "../../lib/api";
import type { Volume } from "../../lib/types";
import { humanBytes, humanCount, humanDuration } from "../../lib/format";
import { scanFraction } from "../../lib/scanEstimate";
import { useTheme } from "../../hooks/useTheme";
import { neutral, tone } from "../../design/palette";
import { Button, SegmentBar, StatTile, type Segment } from "../../components/ui";
import ScanRing from "./ScanRing";
import styles from "./Scan.module.css";

type ScanScreenProps = {
  volume: Volume | null;
  rootLabel: string;
  status: ScanStatus;
  progress: ProgressMsg | null;
  /** Bytes the scan is expected to find; null leaves the ring indeterminate. See `expectedBytes`. */
  expectedBytes: number | null;
  /** The scan has completed and the screen is resting at 100% before the breakdown takes over. */
  finishing?: boolean;
  summary: ScanSummary | null;
  error: string | null;
  onStart: () => void;
  onStop: () => void;
};

const SHORT_VALUE = 6;
/** Time constant of the progress easing; comfortably above the 100 ms between progress messages. */
const SMOOTH_TAU_MS = 120;
/** How long a finished scan rests at 100% before the breakdown takes over: enough for the easing to land. */
export const SCAN_SETTLE_MS = 700;

function titleFor(status: ScanStatus, rootLabel: string) {
  switch (status) {
    case "scanning":
      return `Scanning ${rootLabel}`;
    case "done":
      return "Scan again?";
    case "stopped":
      return "Scan stopped";
    case "error":
      return "Scan failed";
    default:
      return `Ready to sweep ${rootLabel}`;
  }
}

function subFor(status: ScanStatus, summary: ScanSummary | null, error: string | null) {
  switch (status) {
    case "scanning":
      return "Unshift is reading every folder. Nothing is removed during a scan.";
    case "done": {
      const tail = "If you have moved files around, a fresh scan takes about a minute.";
      if (!summary) return tail;
      return `${humanCount(summary.entries)} items · ${humanBytes(summary.bytes)} in ${humanDuration(summary.elapsed_ms)}. ${tail}`;
    }
    case "stopped":
      return "Nothing was kept from the stopped scan. Start again to see the breakdown.";
    case "error":
      return error ?? "Something went wrong while scanning.";
    default:
      return "A full scan takes about a minute. Nothing is removed without your say-so.";
  }
}

/** The root's reported children largest first; slots are keyed and coloured by rank, not by folder. */
function rankedTops(progress: ProgressMsg | null) {
  return progress ? [...progress.tops].sort((a, b) => b.bytes - a.bytes) : [];
}

/**
 * One segment per top-level child the scanner reports, largest first, plus a neutral tail for
 * the rest. Slots are keyed and coloured by rank: the biggest share is always the first tone,
 * so folders swap slots as they grow while the coloured bands stay put, the same order the
 * capacity bar settles on. `shares` are each slot's share of the bar, already eased.
 */
function segmentsOf(tops: ProgressMsg["tops"], shares: number[], restShare: number, theme: ThemeName): Segment[] {
  const slots = tops.map((t, rank) => ({ key: rank, size: shares[rank] ?? 0, fill: tone(theme, rank, tops.length), title: t.name }));
  return [...slots, { key: "rest", size: restShare, fill: neutral(theme), title: "Everything else" }];
}

function ringCentre(scanning: boolean, fraction: number | null, found: string, used: number, volume: Volume | null) {
  if (!scanning) return { value: volume ? `${Math.round(used * 100)}%` : "—", label: "used", size: 42 };
  if (fraction !== null) return { value: `${Math.round(fraction * 100)}%`, label: "scanned", size: 42 };
  return { value: found, label: "found so far", size: found.length > SHORT_VALUE ? 28 : 42 };
}

export default function ScanScreen({ volume, rootLabel, status, progress, expectedBytes, finishing = false, summary, error, onStart, onStop }: ScanScreenProps) {
  const { theme } = useTheme();
  const scanning = status === "scanning" || finishing;
  const used = volume && volume.total > 0 ? (volume.total - volume.free) / volume.total : 0;
  const found = humanBytes(progress?.bytes ?? 0);
  const unreadable = status === "done" && summary ? summary.errors : 0;

  // The bar is the whole disk, as in the capacity bar: segments only ever grow to the share of
  // it the scan has found, so a finished scan rests at the used space rather than edge to edge.
  const barTotal = volume && volume.total > 0 ? volume.total : null;
  const share = (bytes: number) => (barTotal ? Math.min(1, bytes / barTotal) : 0);
  const tops = rankedTops(progress);
  const targetFraction = finishing ? 1 : scanning ? scanFraction(progress?.bytes ?? 0, expectedBytes) : null;
  const targets = [targetFraction ?? 0, share(progress?.rest ?? 0), ...tops.map((t) => share(t.bytes))];
  const [fractionNow, restShare, ...topShares] = useSmoothed(targets, SMOOTH_TAU_MS, !scanning || progress === null);
  const fraction = targetFraction === null ? null : fractionNow;
  const centre = ringCentre(scanning, fraction, found, used, volume);

  return (
    <section className={styles.screen}>
      <div className={styles.dragStrip} data-tauri-drag-region="deep" />
      <ScanRing used={used} scanning={scanning} progress={fraction} value={centre.value} valueSize={centre.size} label={centre.label} />
      <div className={styles.title}>{titleFor(status, rootLabel)}</div>
      <div className={styles.sub}>{subFor(status, summary, error)}</div>

      {scanning ? (
        <>
          <div className={styles.stats}>
            <StatTile label="Files scanned" value={humanCount(progress?.files ?? 0)} />
            <StatTile label="Found so far" value={found} />
          </div>
          {barTotal !== null && (
            <SegmentBar segments={segmentsOf(tops, topShares, restShare, theme)} total={1} height={8} className={styles.bar} />
          )}
          <div className={styles.path}>
            Reading {rootLabel}… {humanCount(progress?.dirs ?? 0)} folders
          </div>
          <Button muted className={styles.stop} onClick={onStop}>
            Stop
          </Button>
        </>
      ) : (
        <>
          <Button variant="primary" size="xl" className={styles.start} onClick={onStart}>
            Scan {rootLabel}
          </Button>
          <div className={styles.note}>{status === "done" ? "Last scan: just now" : status === "stopped" ? "Last scan: stopped" : "No scan yet"}</div>
          {unreadable > 0 && (
            <div className={styles.warn}>
              {humanCount(unreadable)} {unreadable === 1 ? "folder" : "folders"} could not be read. Grant Full Disk Access to include them.
            </div>
          )}
        </>
      )}
    </section>
  );
}
