import { useEffect, useState } from "react";
import Sidebar, { type RootChoice } from "../../components/Sidebar";
import Toolbar, { ToolbarTitle } from "../../components/Toolbar";
import { Button } from "../../components/ui";
import type { ScanStatus } from "../../hooks/useScan";
import type { ProgressMsg, ScanSummary } from "../../lib/api";
import { GB } from "../../lib/format";
import { expectedBytes } from "../../lib/scanEstimate";
import type { Screen, Volume } from "../../lib/types";
import ScanScreen, { SCAN_SETTLE_MS } from "../../screens/Scan";
import SettingsScreen from "../../screens/Settings";
import styles from "./ShellPreview.module.css";

const HOME = "/Users/alex";
const VOLUME: Volume = { name: "Macintosh HD", mount: "/", total: 994.7 * GB, free: 412 * GB };
const USED = VOLUME.total - VOLUME.free;
const VOLUMES: Volume[] = [VOLUME, { name: "Backup", mount: "/Volumes/Backup", total: 2000 * GB, free: 1310 * GB }];
const rootFor = (choice: RootChoice) =>
  choice.kind === "volume" ? choice.mount : choice.kind === "home" ? HOME : `${HOME}/Downloads`;
const STATUSES: ScanStatus[] = ["idle", "scanning", "done", "stopped", "error"];
const SCREENS: Screen[] = ["scan", "breakdown", "largest", "suggest", "settings"];

const MOCK_SCAN_MS = 8_000;
const TICK_MS = 120;
const FULL_SCAN = { files: 1_198_421, dirs: 86_482, errors: 37 };
/** The root's children as a share of used space; each one starts a little after the previous. */
const TOPS: [string, number][] = [
  ["Users", 0.42],
  ["Applications", 0.16],
  ["private", 0.09],
  ["Library", 0.08],
  ["System", 0.06],
  ["Local snapshots", 0.05],
  ["usr", 0.04],
  ["opt", 0.02],
];
const REST_SHARE = 1 - TOPS.reduce((sum, [, share]) => sum + share, 0);
const TOP_LAG = 0.07;

/** Progress of a mock scan at fraction `f` of its run, with top-level folders filling in staggered. */
function mockProgress(f: number): ProgressMsg {
  const lagged = (i: number) => Math.max(0, Math.min(1, (f - i * TOP_LAG) / (1 - i * TOP_LAG)));
  const tops = TOPS.map(([name, share], i) => ({ name, bytes: share * USED * lagged(i) }));
  const rest = REST_SHARE * USED * lagged(TOPS.length);
  return {
    files: Math.round(FULL_SCAN.files * f),
    dirs: Math.round(FULL_SCAN.dirs * f),
    bytes: tops.reduce((sum, t) => sum + t.bytes, rest),
    errors: Math.floor(FULL_SCAN.errors * f),
    elapsed_ms: Math.round(f * MOCK_SCAN_MS),
    tops,
    rest,
  };
}

const MID_SCAN = mockProgress(0.35);
const DONE = mockProgress(1);

function rootLabel(root: string) {
  const volume = VOLUMES.find((v) => v.mount === root);
  if (volume) return volume.name;
  if (root === HOME) return "your home folder";
  return root.startsWith(HOME) ? `~${root.slice(HOME.length)}` : root;
}

function summaryOf(root: string, p: ProgressMsg, cancelled: boolean): ScanSummary {
  return { root, entries: p.files + p.dirs, bytes: p.bytes, errors: p.errors, elapsed_ms: p.elapsed_ms, cancelled, live: true };
}

function fromQuery<T extends string>(key: string, allowed: readonly T[], fallback: T): T {
  const v = new URLSearchParams(location.search).get(key);
  return allowed.includes(v as T) ? (v as T) : fallback;
}

export default function ShellPreview() {
  const [screen, setScreen] = useState<Screen>(() => fromQuery("screen", SCREENS, "scan"));
  const [scanRoot, setScanRoot] = useState("/");
  const [status, setStatus] = useState<ScanStatus>(() => fromQuery("status", STATUSES, "idle"));
  const [progress, setProgress] = useState<ProgressMsg | null>(() => (status === "scanning" ? MID_SCAN : null));
  const [summary, setSummary] = useState<ScanSummary | null>(() => (status === "done" ? summaryOf("/", DONE, false) : null));
  const [finishing, setFinishing] = useState(false);

  useEffect(() => {
    if (status !== "scanning") return;
    const id = setInterval(() => {
      setProgress((p) => (p && p.elapsed_ms >= MOCK_SCAN_MS ? p : mockProgress(Math.min(1, ((p?.elapsed_ms ?? 0) + TICK_MS) / MOCK_SCAN_MS))));
    }, TICK_MS);
    return () => clearInterval(id);
  }, [status]);

  // A complete mock scan rests at 100% for a beat, as the app does before showing the breakdown.
  useEffect(() => {
    if (status !== "scanning" || !progress || progress.elapsed_ms < MOCK_SCAN_MS) return;
    setFinishing(true);
    const id = setTimeout(() => {
      setSummary(summaryOf(scanRoot, progress, false));
      setStatus("done");
      setFinishing(false);
    }, SCAN_SETTLE_MS);
    return () => clearTimeout(id);
  }, [status, progress, scanRoot]);

  const goTo = (next: ScanStatus) => {
    setStatus(next);
    setProgress(next === "scanning" ? MID_SCAN : null);
    setSummary(next === "done" ? summaryOf(scanRoot, DONE, false) : next === "stopped" ? summaryOf(scanRoot, MID_SCAN, true) : null);
    if (next === "scanning" || next === "stopped" || next === "error") setScreen("scan");
  };

  const start = () => {
    setStatus("scanning");
    setProgress(null);
    setSummary(null);
    setScreen("scan");
  };

  const stop = () => {
    setSummary(summaryOf(scanRoot, progress ?? MID_SCAN, true));
    setStatus("stopped");
  };

  const pickRoot = (choice: RootChoice) => {
    setScanRoot(rootFor(choice));
    setStatus("idle");
    setProgress(null);
    setSummary(null);
    setScreen("scan");
  };

  const cycle = () => goTo(STATUSES[(STATUSES.indexOf(status) + 1) % STATUSES.length]);

  return (
    <div className={styles.window}>
      <Sidebar volume={VOLUME} volumes={VOLUMES} scanRoot={scanRoot} home={HOME} onPickRoot={pickRoot} scanned={status === "done"} screen={screen} onNavigate={setScreen} />
      <main className={styles.main}>
        {screen === "scan" && (
          <ScanScreen
            volume={VOLUME}
            rootLabel={rootLabel(scanRoot)}
            status={status}
            progress={progress}
            expectedBytes={expectedBytes(VOLUME, scanRoot, summary && !summary.cancelled ? summary.bytes : null)}
            finishing={finishing}
            summary={summary}
            error={status === "error" ? "Permission denied (os error 13) while reading /private/var/db" : null}
            onStart={start}
            onStop={stop}
          />
        )}
        {screen === "settings" && <SettingsScreen />}
        {screen !== "scan" && screen !== "settings" && (
          <section className={styles.placeholder}>
            <Toolbar>
              <ToolbarTitle>{screen[0].toUpperCase() + screen.slice(1)}</ToolbarTitle>
            </Toolbar>
            <div className={styles.empty}>Rendered by the Breakdown preview.</div>
          </section>
        )}
        <div className={styles.devbar}>
          <span>status: {status}</span>
          <Button size="sm" onClick={cycle}>
            Next state
          </Button>
        </div>
      </main>
    </div>
  );
}
