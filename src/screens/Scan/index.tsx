import type { ScanStatus } from "../../hooks/useScan";
import type { ProgressMsg, ScanSummary } from "../../lib/api";
import type { Volume } from "../../lib/types";
import { humanBytes, humanCount, humanDuration } from "../../lib/format";
import { Button, StatTile } from "../../components/ui";
import ScanRing from "./ScanRing";
import styles from "./Scan.module.css";

type ScanScreenProps = {
  volume: Volume | null;
  rootLabel: string;
  status: ScanStatus;
  progress: ProgressMsg | null;
  summary: ScanSummary | null;
  error: string | null;
  onStart: () => void;
  onStop: () => void;
};

const SHORT_VALUE = 6;

function titleFor(status: ScanStatus, rootLabel: string) {
  switch (status) {
    case "scanning":
      return `Scanning ${rootLabel}`;
    case "done":
      return "Scan again?";
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
    case "error":
      return error ?? "Something went wrong while scanning.";
    default:
      return "A full scan takes about a minute. Nothing is removed without your say-so.";
  }
}

export default function ScanScreen({ volume, rootLabel, status, progress, summary, error, onStart, onStop }: ScanScreenProps) {
  const scanning = status === "scanning";
  const used = volume && volume.total > 0 ? (volume.total - volume.free) / volume.total : 0;
  const found = humanBytes(progress?.bytes ?? 0);
  const unreadable = status === "done" && summary ? summary.errors : 0;

  return (
    <section className={styles.screen}>
      <ScanRing
        used={used}
        scanning={scanning}
        value={scanning ? found : volume ? `${Math.round(used * 100)}%` : "—"}
        valueSize={scanning && found.length > SHORT_VALUE ? 28 : 42}
        label={scanning ? "found so far" : "used"}
      />
      <div className={styles.title}>{titleFor(status, rootLabel)}</div>
      <div className={styles.sub}>{subFor(status, summary, error)}</div>

      {scanning ? (
        <>
          <div className={styles.stats}>
            <StatTile label="Files scanned" value={humanCount(progress?.files ?? 0)} />
            <StatTile label="Found so far" value={found} />
          </div>
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
          <div className={styles.note}>{status === "done" ? "Last scan: just now" : "No scan yet"}</div>
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
