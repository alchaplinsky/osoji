import { useCallback, useEffect, useRef, useState, type MouseEvent } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import Sidebar, { type RootChoice } from "./components/Sidebar";
import ReviewSheet from "./components/ReviewSheet";
import FdaBanner from "./components/FdaBanner";
import { Toast } from "./components/ui";
import ScanScreen, { SCAN_SETTLE_MS } from "./screens/Scan";
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
import { useFullDiskAccess } from "./hooks/useFullDiskAccess";
import { useShortcuts } from "./hooks/useShortcuts";
import { useItemMenu } from "./hooks/useItemMenu";
import { homeDir, nodeInfo, reveal, trashInfo, trashNodes, volumeInfo, volumes as listVolumes } from "./lib/api";
import type { TrashInfo, TrashProgress } from "./lib/api";
import { humanBytes } from "./lib/format";
import { isProtected, reviewWarning } from "./lib/bin";
import { expectedBytes } from "./lib/scanEstimate";
import { useScanTotals } from "./hooks/useScanTotals";
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
  const itemMenu = useItemMenu();
  const { theme } = useTheme();
  const fda = useFullDiskAccess();
  const { confirmBig, watchers: watchersOn } = useSettings();
  const [screen, setScreen] = useState<Screen>("scan");
  const [viz, setViz] = useState<Viz>("sunburst");
  const [home, setHome] = useState("");
  const [scanRoot, setScanRoot] = useState("/");
  const [volume, setVolume] = useState<Volume | null>(null);
  const [volumes, setVolumes] = useState<Volume[]>([]);
  const totals = useScanTotals();
  const [finishing, setFinishing] = useState(false);
  const [hoverId, setHoverId] = useState<HoverId>(null);
  const [listOpen, setListOpen] = useState(true);
  const [review, setReview] = useState(false);
  const [sweeping, setSweeping] = useState(false);
  const [sweepError, setSweepError] = useState<string | null>(null);
  const [sweepProgress, setSweepProgress] = useState<TrashProgress | null>(null);
  const [toast, setToast] = useState<{ title: string; sub: string } | null>(null);
  const toastTimer = useRef<number>();
  const [trash, setTrash] = useState<TrashInfo | null>(null);
  const loadTrash = useCallback(() => trashInfo().then(setTrash), []);

  useEffect(() => {
    homeDir().then(setHome).catch(console.error);
  }, []);

  // Drives come and go while the app is in the background; re-list them on focus.
  useEffect(() => {
    const refresh = () => listVolumes().then(setVolumes).catch(console.error);
    refresh();
    window.addEventListener("focus", refresh);
    return () => window.removeEventListener("focus", refresh);
  }, []);

  useEffect(() => {
    let active = true;
    setVolume(null);
    volumeInfo(scanRoot)
      .then((next) => active && setVolume(next))
      .catch(console.error);
    return () => {
      active = false;
    };
  }, [scanRoot]);

  const { load, reset } = explorer;
  const { reset: resetScan } = scan;
  const { load: loadInsights, reset: resetInsights } = insights;
  const { refresh: refreshWatchers, reset: resetWatchers } = watchers;
  const { clear: clearBin } = bin;
  const { remember } = totals;
  useEffect(() => {
    if (scan.status === "scanning") {
      reset();
      resetInsights();
      resetWatchers();
      clearBin();
    }
    if (scan.status === "done") {
      const done = scan.summary;
      const completed = done !== null && !done.cancelled;
      if (completed) remember(done.root, done.bytes);
      setFinishing(completed);
      const settle = new Promise((resolve) => setTimeout(resolve, completed ? SCAN_SETTLE_MS : 0));
      Promise.all([load(0), loadInsights(), refreshWatchers(), loadTrash(), settle])
        .then(() => setScreen("breakdown"))
        .catch(console.error)
        .finally(() => setFinishing(false));
    }
  }, [scan.status, scan.summary, load, reset, loadInsights, resetInsights, refreshWatchers, resetWatchers, clearBin, remember, loadTrash]);

  const changeRoot = useCallback(
    (next: string) => {
      if (next === scanRoot || scan.status === "scanning") return;
      setScanRoot(next);
      setScreen("scan");
      resetScan();
      reset();
      resetInsights();
      resetWatchers();
      clearBin();
    },
    [scanRoot, scan.status, resetScan, reset, resetInsights, resetWatchers, clearBin],
  );

  const pickRoot = useCallback(
    async (choice: RootChoice) => {
      if (choice.kind === "volume") return changeRoot(choice.mount);
      if (choice.kind === "home") return changeRoot(home || (await homeDir()));
      const dir = await open({ directory: true, multiple: false, defaultPath: scanRoot });
      if (typeof dir === "string") changeRoot(dir);
    },
    [changeRoot, home, scanRoot],
  );

  const rootLabel =
    scanRoot === volume?.mount
      ? volume.name
      : home && (scanRoot === home || scanRoot.startsWith(`${home}/`))
        ? "~" + scanRoot.slice(home.length)
        : scanRoot === "/"
          ? "Startup Disk"
          : scanRoot;
  const scanned = scan.status === "done" && explorer.view !== null;
  const unreadable = scan.summary?.errors ?? 0;
  const askForAccess = unreadable > 0 && fda.granted === false && !fda.dismissed;

  const startScan = () => {
    setScreen("scan");
    scan.start(scanRoot);
  };

  const navigate = (id: number) => {
    setHoverId(null);
    explorer.load(id).catch(console.error);
  };

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
      guard: node.guard ?? null,
    };
  };

  const toggleBinById = (id: number) => {
    const item = describe(id);
    if (item) bin.toggle(item);
  };

  const revealItem = (id: number) => reveal(id).catch(console.error);

  const openNodeMenu = (e: MouseEvent, id: number) => {
    const item = describe(id);
    if (item) itemMenu.open(e, { id, path: item.path, inBin: bin.ids.has(id), onToggleBin: isProtected(item) ? undefined : () => bin.toggle(item) });
  };

  const openReviewMenu = (e: MouseEvent, item: BinItem) =>
    itemMenu.open(e, { id: item.id, path: item.path, inBin: true, onToggleBin: sweeping ? undefined : () => bin.remove(item.id) });

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
      const result = await trashNodes(bin.items.map((i) => i.id), setSweepProgress);
      const here = [explorer.currentId, ...(explorer.info?.ancestors.map((a) => a.id) ?? [])];
      const moved = new Set(result.trashed);
      const trashedHere = bin.items.find((i) => moved.has(i.id) && here.includes(i.id));
      result.trashed.forEach(bin.remove);
      await Promise.all([explorer.refresh(trashedHere ? (trashedHere.ancestors.at(-1) ?? 0) : explorer.currentId), insights.load(), watchers.refresh(), loadTrash()]);
      setHoverId(null);
      if (result.failed.length > 0) {
        setSweepError(result.failed.join("\n"));
        return;
      }
      setReview(false);
      const n = result.count === 1 ? "1 item" : `${result.count} items`;
      showToast(`Moved ${humanBytes(result.bytes)} to the Trash`, `${n}. Empty the Trash to reclaim the space on ${volume?.name ?? "this disk"}.`);
    } catch (e) {
      setSweepError(String(e));
    } finally {
      setSweeping(false);
      setSweepProgress(null);
    }
  };

  const bigCount = confirmBig ? bin.items.filter((i) => i.size >= BIG_ITEM).length : 0;
  const warning = reviewWarning(bin.items, bigCount);

  const addWatcher = async () => {
    const dir = await open({ directory: true, multiple: false, defaultPath: home || scanRoot });
    if (typeof dir === "string") watchers.add(dir);
  };

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

  useShortcuts(
    {
      "Meta+r": () => {
        if (scan.status !== "scanning" && !sweeping) startScan();
      },
      "Meta+Enter": () => {
        if (active === "breakdown") reveal(explorer.currentId).catch(console.error);
      },
    },
    !review,
  );

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
        volumes={volumes}
        scanRoot={scanRoot}
        home={home}
        rootPickerDisabled={scan.status === "scanning" || sweeping}
        onPickRoot={pickRoot}
        trash={trash}
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
            expectedBytes={expectedBytes(volume, scanRoot, totals.get(scanRoot))}
            finishing={finishing}
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
            onReveal={() => revealItem(explorer.currentId)}
            onRevealItem={revealItem}
            onItemMenu={openNodeMenu}
            onToggleBin={toggleBinById}
            banner={askForAccess && <FdaBanner unreadable={unreadable} onOpenSettings={fda.openSettings} onDismiss={fda.dismiss} />}
            unreadable={unreadable}
            {...binProps}
          />
        )}
        {active === "largest" && (
          <LargestScreen
            entries={insights.largest}
            safeIds={insights.safeIds}
            onToggleBin={bin.toggle}
            onReveal={revealItem}
            onItemMenu={itemMenu.open}
            {...binProps}
          />
        )}
        {active === "suggest" && (
          <SuggestionsScreen
            suggestions={insights.suggestions}
            onToggleBin={bin.toggle}
            onAddAll={(items) => items.forEach(bin.add)}
            onRemoveMany={(ids) => ids.forEach(bin.remove)}
            onItemMenu={itemMenu.open}
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
        progress={sweepProgress}
        error={sweepError}
        warning={warning}
        onCancel={() => {
          setReview(false);
          setSweepError(null);
        }}
        onConfirm={confirmSweep}
        onItemMenu={openReviewMenu}
      />
      {itemMenu.element}
    </div>
  );
}
