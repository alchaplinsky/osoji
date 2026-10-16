import { useCallback, useEffect, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import Sidebar from "./components/Sidebar";
import ReviewSheet from "./components/ReviewSheet";
import { Toast } from "./components/ui";
import ScanScreen from "./screens/Scan";
import BreakdownScreen from "./screens/Breakdown";
import LargestScreen from "./screens/Largest";
import SuggestionsScreen from "./screens/Suggestions";
import WatchersScreen from "./screens/Watchers";
import SettingsScreen from "./screens/Settings";
import { useScan } from "./hooks/useScan";
import { useExplorer } from "./hooks/useExplorer";
import { useInsights } from "./hooks/useInsights";
import { useWatchers } from "./hooks/useWatchers";
import { useBin } from "./hooks/useBin";
import { useTheme } from "./hooks/useTheme";
import { useSettings } from "./hooks/useSettings";
import { homeDir, nodeInfo, reveal, trashNodes, volumeInfo } from "./lib/api";
import { humanBytes } from "./lib/format";
import { pathTo } from "./lib/tree";
import type { BinItem, HoverId, Screen, Viz, Volume, WatcherStatus } from "./lib/types";
import styles from "./App.module.css";

const TOAST_MS = 4500;
const BIG_ITEM = 1024 ** 3;

export default function App() {
  const scan = useScan();
  const explorer = useExplorer();
  const insights = useInsights();
  const watchers = useWatchers();
  const bin = useBin();
  const { theme } = useTheme();
  const { confirmBig, watchers: watchersOn } = useSettings();
  const [screen, setScreen] = useState<Screen>("scan");
  const [viz, setViz] = useState<Viz>("sunburst");
  const [home, setHome] = useState("");
  const [scanRoot, setScanRoot] = useState("/");
  const [volume, setVolume] = useState<Volume | null>(null);
  const [hoverId, setHoverId] = useState<HoverId>(null);
  const [listOpen, setListOpen] = useState(true);
  const [review, setReview] = useState(false);
  const [sweeping, setSweeping] = useState(false);
  const [sweepError, setSweepError] = useState<string | null>(null);
  const [toast, setToast] = useState<{ title: string; sub: string } | null>(null);
  const toastTimer = useRef<number>();

  useEffect(() => {
    homeDir().then(setHome).catch(console.error);
  }, []);

  useEffect(() => {
    volumeInfo(scanRoot).then(setVolume).catch(console.error);
  }, [scanRoot]);

  const { load, reset } = explorer;
  const { load: loadInsights, reset: resetInsights } = insights;
  const { refresh: refreshWatchers, reset: resetWatchers } = watchers;
  const { clear: clearBin } = bin;
  useEffect(() => {
    if (scan.status === "scanning") {
      reset();
      resetInsights();
      resetWatchers();
      clearBin();
    }
    if (scan.status === "done") {
      Promise.all([load(0), loadInsights(), refreshWatchers()])
        .then(() => setScreen("breakdown"))
        .catch(console.error);
    }
  }, [scan.status, load, reset, loadInsights, resetInsights, refreshWatchers, resetWatchers, clearBin]);

  const pickRoot = useCallback(
    async (choice: "disk" | "home" | "folder") => {
      if (choice === "disk") return setScanRoot("/");
      if (choice === "home") return setScanRoot(home || (await homeDir()));
      const dir = await open({ directory: true, multiple: false, defaultPath: scanRoot });
      if (typeof dir === "string") setScanRoot(dir);
    },
    [home, scanRoot],
  );

  const rootLabel = scanRoot === "/" ? (volume?.name ?? "Macintosh HD") : home && scanRoot.startsWith(home) ? "~" + scanRoot.slice(home.length) : scanRoot;
  const scanned = scan.status === "done" && explorer.view !== null;

  const startScan = () => {
    setScreen("scan");
    scan.start(scanRoot);
  };

  const navigate = (id: number) => {
    setHoverId(null);
    explorer.load(id).catch(console.error);
  };

  /** Describe a node below the current view as a bin item, with its full path and ancestry. */
  const describe = (id: number): BinItem | null => {
    const { view, info } = explorer;
    if (!view || !info) return null;
    const path = pathTo(view, id);
    if (path.length < 2) return null;
    const node = path[path.length - 1];
    const below = path.slice(1);
    const base = info.path.replace(/\/$/, "");
    return {
      id,
      name: node.name,
      path: [base, ...below.map((n) => n.name)].join("/"),
      size: node.size,
      dir: node.dir,
      ancestors: [...info.ancestors.map((a) => a.id), info.id, ...below.slice(0, -1).map((n) => n.id)],
    };
  };

  const toggleBinById = (id: number) => {
    const item = describe(id);
    if (item) bin.toggle(item);
  };

  const showToast = (title: string, sub: string) => {
    window.clearTimeout(toastTimer.current);
    setToast({ title, sub });
    toastTimer.current = window.setTimeout(() => setToast(null), TOAST_MS);
  };

  const confirmSweep = async () => {
    if (sweeping || bin.items.length === 0) return;
    setSweeping(true);
    setSweepError(null);
    try {
      const result = await trashNodes(bin.items.map((i) => i.id));
      // If the current folder (or one of its ancestors) was trashed, step up to the nearest surviving parent.
      const here = [explorer.currentId, ...(explorer.info?.ancestors.map((a) => a.id) ?? [])];
      const trashedHere = bin.items.find((i) => here.includes(i.id));
      await Promise.all([explorer.refresh(trashedHere ? (trashedHere.ancestors.at(-1) ?? 0) : explorer.currentId), insights.load(), watchers.refresh()]);
      bin.clear();
      setReview(false);
      setHoverId(null);
      const n = result.count === 1 ? "1 item" : `${result.count} items`;
      showToast(`Moved ${humanBytes(result.freed)} to the Trash`, `${n}. Empty the Trash to reclaim the space on ${volume?.name ?? "this disk"}.`);
    } catch (e) {
      setSweepError(String(e));
    } finally {
      setSweeping(false);
    }
  };

  const bigCount = confirmBig ? bin.items.filter((i) => i.size >= BIG_ITEM).length : 0;
  const warning = bigCount > 0 ? `${bigCount === 1 ? "1 item is" : `${bigCount} items are`} over 1 GB. Double-check before you continue.` : undefined;

  const addWatcher = async () => {
    const dir = await open({ directory: true, multiple: false, defaultPath: home || scanRoot });
    if (typeof dir === "string") watchers.add(dir);
  };

  /** Jump to the watched folder in the Breakdown; for a file, open its parent and highlight it. */
  const cleanUp = async (status: WatcherStatus) => {
    if (status.id === null) return;
    setScreen("breakdown");
    if (status.dir) return navigate(status.id);
    const info = await nodeInfo(status.id);
    const parent = info.ancestors.at(-1);
    if (parent) await explorer.load(parent.id);
    setHoverId(status.id);
  };

  const canShow = (s: Screen) => s === "scan" || s === "settings" || (s === "watchers" ? watchersOn && scanned : scanned);
  const active = canShow(screen) ? screen : "scan";

  const binProps = {
    binItems: bin.items,
    binIds: bin.ids,
    onRemoveFromBin: bin.remove,
    onReview: () => setReview(true),
  };

  return (
    <div className={styles.app}>
      <Sidebar
        volume={volume}
        scanRoot={scanRoot}
        onPickRoot={pickRoot}
        scanned={scanned}
        screen={active}
        suggestBadge={scanned && insights.safeTotal > 0 ? humanBytes(insights.safeTotal) : undefined}
        watchers={watchersOn ? { badge: scanned && watchers.overCount > 0 ? `${watchers.overCount} over` : undefined } : null}
        onNavigate={(s) => {
          setHoverId(null);
          setScreen(s);
        }}
      />
      <main className={styles.main}>
        {active === "scan" && (
          <ScanScreen
            volume={volume}
            rootLabel={rootLabel}
            status={scan.status}
            progress={scan.progress}
            summary={scan.summary}
            error={scan.error}
            onStart={startScan}
            onStop={scan.cancel}
          />
        )}
        {active === "breakdown" && volume && explorer.rootView && explorer.view && explorer.info && (
          <BreakdownScreen
            volume={volume}
            rootView={explorer.rootView}
            view={explorer.view}
            info={explorer.info}
            theme={theme}
            hoverId={hoverId}
            onHover={setHoverId}
            onNavigate={navigate}
            onUp={() => {
              setHoverId(null);
              explorer.goUp().catch(console.error);
            }}
            viz={viz}
            onViz={setViz}
            listOpen={listOpen}
            onToggleList={() => setListOpen((o) => !o)}
            onRescan={startScan}
            onReveal={() => reveal(explorer.currentId).catch(console.error)}
            onToggleBin={toggleBinById}
            {...binProps}
          />
        )}
        {active === "largest" && (
          <LargestScreen
            entries={insights.largest}
            safeIds={insights.safeIds}
            onToggleBin={bin.toggle}
            onReveal={(id) => reveal(id).catch(console.error)}
            {...binProps}
          />
        )}
        {active === "suggest" && (
          <SuggestionsScreen
            suggestions={insights.suggestions}
            onToggleBin={bin.toggle}
            onAddAll={(items) => items.forEach(bin.add)}
            onRemoveMany={(ids) => ids.forEach(bin.remove)}
            {...binProps}
          />
        )}
        {active === "watchers" && (
          <WatchersScreen
            statuses={watchers.statuses}
            scanned={scanned}
            onAdd={() => addWatcher().catch(console.error)}
            onRemove={watchers.remove}
            onSetLimit={watchers.setLimit}
            onCleanUp={(s) => cleanUp(s).catch(console.error)}
            {...binProps}
          />
        )}
        {active === "settings" && <SettingsScreen />}
        {toast && <Toast title={toast.title} sub={toast.sub} />}
      </main>
      <ReviewSheet
        open={review}
        items={bin.items}
        volumeName={volume?.name ?? "this disk"}
        busy={sweeping}
        error={sweepError}
        warning={warning}
        onCancel={() => {
          setReview(false);
          setSweepError(null);
        }}
        onConfirm={confirmSweep}
      />
    </div>
  );
}
