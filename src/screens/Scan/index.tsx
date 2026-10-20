import type { ThemeName } from "../../design/theme";
import type { ScanStatus } from "../../hooks/useScan";
import { useSmoothed } from "../../hooks/useSmoothed";
import type { ProgressMsg, ScanSummary } from "../../lib/api";
import type { Volume } from "../../lib/types";
import { humanBytes, humanCount, humanDuration } from "../../lib/format";
import { scanFraction, type ScanExpectation } from "../../lib/scanEstimate";
import { useTheme } from "../../hooks/useTheme";
import { neutral, tone } from "../../design/palette";
import { Button, SegmentBar, StatTile, type Segment } from "../../components/ui";
import ScanRing from "./ScanRing";
import styles from "./Scan.module.css";

type ScanScreenProps = {
  volume: Volume | null;
  /** Path being scanned; the ring spans the disk when it is `volume.mount`, else the folder. */
  scanRoot: string;
  rootLabel: string;
  status: ScanStatus;
  progress: ProgressMsg | null;
  /** What the scan is expected to find; null leaves the ring indeterminate. See `expectedScan`. */
  expected: ScanExpectation | null;
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

/** The scan status, with the rest at 100% between a completed scan and its breakdown as a phase of its own. */
type Phase = ScanStatus | "finishing";

function statsOf(summary: ScanSummary) {
  return `${humanCount(summary.entries)} items · ${humanBytes(summary.bytes)} in ${humanDuration(summary.elapsed_ms)}.`;
}

function titleFor(phase: Phase, rootLabel: string) {
  switch (phase) {
    case "scanning":
      return `Scanning ${rootLabel}`;
    case "finishing":
      return "Scan complete";
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

function subFor(phase: Phase, summary: ScanSummary | null, error: string | null) {
  switch (phase) {
    case "scanning":
      return "Osoji is reading every folder. Nothing is removed during a scan.";
    case "finishing":
      return summary ? statsOf(summary) : "Nothing is removed without your say-so.";
    case "done": {
      const tail = "If you have moved files around, a fresh scan takes about a minute.";
      return summary ? `${statsOf(summary)} ${tail}` : tail;
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

/** A byte figure for the ring's centre, in the smaller size when it would not fit. */
function bytesCentre(value: string, label: string) {
  return { value, label, size: value.length > SHORT_VALUE ? 28 : 42 };
}

/** The resting ring reads a disk's used share, or a folder's size from its last scan. */
function restingCentre(isVolume: boolean, used: number, lastBytes: number | null) {
  if (isVolume) return { value: `${Math.round(used * 100)}%`, label: "used", size: 42 };
  return bytesCentre(lastBytes ? humanBytes(lastBytes) : "—", "last scan");
}

function ringCentre(scanning: boolean, fraction: number | null, found: string, resting: ReturnType<typeof restingCentre>) {
  if (!scanning) return resting;
  if (fraction !== null) return { value: `${Math.round(fraction * 100)}%`, label: "scanned", size: 42 };
  return bytesCentre(found, "found so far");
}

export default function ScanScreen({ volume, scanRoot, rootLabel, status, progress, expected, finishing = false, summary, error, onStart, onStop }: ScanScreenProps) {
  const { theme } = useTheme();
  const scanning = status === "scanning" || finishing;
  const phase: Phase = finishing ? "finishing" : status;
  const isVolume = volume !== null && volume.total > 0 && scanRoot === volume.mount;
  const used = isVolume ? (volume.total - volume.free) / volume.total : 0;
  // The ring is the item being scanned: a disk shows its used share with free space left open, a
  // folder has no free space and fills the whole ring. The scan's 100% is the end of that arc.
  const span = isVolume ? used : 1;
  const found = humanBytes(progress?.bytes ?? 0);
  const unreadable = status === "done" && summary ? summary.errors : 0;

  // The bar is the whole disk, as in the capacity bar: segments only ever grow to the share of
  // it the scan has found, so a finished scan rests at the used space rather than edge to edge.
  const barTotal = volume && volume.total > 0 ? volume.total : null;
  const share = (bytes: number) => (barTotal ? Math.min(1, bytes / barTotal) : 0);
  const tops = rankedTops(progress);
  const targetFraction = finishing ? 1 : scanning ? scanFraction(progress, expected) : null;
  const targets = [targetFraction ?? 0, share(progress?.rest ?? 0), ...tops.map((t) => share(t.bytes))];
  const [fractionNow, restShare, ...topShares] = useSmoothed(targets, SMOOTH_TAU_MS, !scanning || progress === null);
  const fraction = targetFraction === null ? null : fractionNow;
  // For a folder `expected` is only ever its last scan; a disk falls back to statfs, so it is not shown.
  const centre = ringCentre(scanning, fraction, found, restingCentre(isVolume, used, expected?.bytes ?? null));

  return (
    <section className={styles.screen}>
      <div className={styles.dragStrip} data-tauri-drag-region="deep" />
      <ScanRing span={span} scanning={scanning} progress={fraction} value={centre.value} valueSize={centre.size} label={centre.label} />
      <div className={styles.title}>{titleFor(phase, rootLabel)}</div>
      <div className={styles.sub}>{subFor(phase, summary, error)}</div>

      {scanning ? (
        <>
          <div className={styles.stats}>
            <StatTile label="Files scanned" value={humanCount(progress?.files ?? 0)} />
            <StatTile label="Found so far" value={found} />
          </div>
          {barTotal !== null && (
            <SegmentBar segments={segmentsOf(tops, topShares, restShare, theme)} total={1} height={8} className={styles.bar} />
          )}
          {/* The breakdown waits on the backend's wind-down; saying so keeps a long rest from reading as stuck. */}
          <div className={styles.path}>
            {finishing ? "Preparing the breakdown…" : `Reading ${rootLabel}… ${humanCount(progress?.dirs ?? 0)} folders`}
          </div>
          {!finishing && (
            <Button muted className={styles.stop} onClick={onStop}>
              Stop
            </Button>
          )}
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
