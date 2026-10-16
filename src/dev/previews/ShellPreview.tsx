import { useEffect, useState } from "react";
import Sidebar, { type RootChoice } from "../../components/Sidebar";
import Toolbar, { ToolbarTitle } from "../../components/Toolbar";
import { Button } from "../../components/ui";
import type { ScanStatus } from "../../hooks/useScan";
import type { ProgressMsg, ScanSummary } from "../../lib/api";
import type { Screen, Volume } from "../../lib/types";
import ScanScreen from "../../screens/Scan";
import SettingsScreen from "../../screens/Settings";
import styles from "./ShellPreview.module.css";

const GB = 1024 ** 3;
const HOME = "/Users/alex";
const VOLUME: Volume = { name: "Macintosh HD", mount: "/", total: 1024 * GB, free: 412 * GB };
const ROOTS: Record<RootChoice, string> = { disk: "/", home: HOME, folder: `${HOME}/Downloads` };
const STATUSES: ScanStatus[] = ["idle", "scanning", "done", "error"];
const SCREENS: Screen[] = ["scan", "breakdown", "largest", "suggest", "settings"];

const MID_SCAN: ProgressMsg = { files: 412_388, dirs: 38_120, bytes: 214.6 * GB, errors: 3, elapsed_ms: 17_400 };

function rootLabel(root: string) {
  if (root === VOLUME.mount) return VOLUME.name;
  if (root === HOME) return "your home folder";
  return root.startsWith(HOME) ? `~${root.slice(HOME.length)}` : root;
}

function summaryOf(root: string, p: ProgressMsg, cancelled: boolean): ScanSummary {
  return { root, entries: p.files + p.dirs, bytes: p.bytes, errors: p.errors, elapsed_ms: p.elapsed_ms, cancelled };
}

const DONE: ProgressMsg = { files: 1_198_421, dirs: 86_482, bytes: 588.3 * GB, errors: 37, elapsed_ms: 48_200 };

function fromQuery<T extends string>(key: string, allowed: readonly T[], fallback: T): T {
  const v = new URLSearchParams(location.search).get(key);
  return allowed.includes(v as T) ? (v as T) : fallback;
}

/** Full-window mock of the app shell: sidebar + Scan / Settings screens with a fake scan. */
export default function ShellPreview() {
  const [screen, setScreen] = useState<Screen>(() => fromQuery("screen", SCREENS, "scan"));
  const [scanRoot, setScanRoot] = useState("/");
  const [status, setStatus] = useState<ScanStatus>(() => fromQuery("status", STATUSES, "idle"));
  const [progress, setProgress] = useState<ProgressMsg | null>(() => (status === "scanning" ? MID_SCAN : null));
  const [summary, setSummary] = useState<ScanSummary | null>(() => (status === "done" ? summaryOf("/", DONE, false) : null));

  useEffect(() => {
    if (status !== "scanning") return;
    const id = setInterval(() => {
      setProgress((p) => {
        const base = p ?? { files: 0, dirs: 0, bytes: 0, errors: 0, elapsed_ms: 0 };
        return {
          files: base.files + 1800 + Math.round(Math.random() * 1400),
          dirs: base.dirs + 120 + Math.round(Math.random() * 90),
          bytes: base.bytes + (0.4 + Math.random() * 1.2) * GB,
          errors: base.errors + (Math.random() < 0.03 ? 1 : 0),
          elapsed_ms: base.elapsed_ms + 120,
        };
      });
    }, 120);
    return () => clearInterval(id);
  }, [status]);

  const goTo = (next: ScanStatus) => {
    setStatus(next);
    setProgress(next === "scanning" ? MID_SCAN : null);
    setSummary(next === "done" ? summaryOf(scanRoot, DONE, false) : null);
    if (next === "scanning" || next === "error") setScreen("scan");
  };

  const start = () => {
    setStatus("scanning");
    setProgress(null);
    setSummary(null);
    setScreen("scan");
  };

  const stop = () => {
    setSummary(summaryOf(scanRoot, progress ?? DONE, true));
    setStatus("done");
  };

  const pickRoot = (choice: RootChoice) => {
    setScanRoot(ROOTS[choice]);
    setStatus("idle");
    setProgress(null);
    setSummary(null);
    setScreen("scan");
  };

  const cycle = () => goTo(STATUSES[(STATUSES.indexOf(status) + 1) % STATUSES.length]);

  return (
    <div className={styles.window}>
      <Sidebar volume={VOLUME} scanRoot={scanRoot} onPickRoot={pickRoot} scanned={status === "done"} screen={screen} onNavigate={setScreen} />
      <main className={styles.main}>
        {screen === "scan" && (
          <ScanScreen
            volume={VOLUME}
            rootLabel={rootLabel(scanRoot)}
            status={status}
            progress={progress}
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
