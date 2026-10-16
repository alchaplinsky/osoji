import { useCallback, useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import Sidebar from "./components/Sidebar";
import ScanScreen from "./screens/Scan";
import BreakdownScreen from "./screens/Breakdown";
import SettingsScreen from "./screens/Settings";
import { useScan } from "./hooks/useScan";
import { useExplorer } from "./hooks/useExplorer";
import { useTheme } from "./hooks/useTheme";
import { homeDir, reveal, volumeInfo } from "./lib/api";
import type { HoverId, Screen, Volume } from "./lib/types";
import styles from "./App.module.css";

export default function App() {
  const scan = useScan();
  const explorer = useExplorer();
  const { theme } = useTheme();
  const [screen, setScreen] = useState<Screen>("scan");
  const [home, setHome] = useState("");
  const [scanRoot, setScanRoot] = useState("/");
  const [volume, setVolume] = useState<Volume | null>(null);
  const [hoverId, setHoverId] = useState<HoverId>(null);
  const [listOpen, setListOpen] = useState(true);

  useEffect(() => {
    homeDir().then(setHome).catch(console.error);
  }, []);

  useEffect(() => {
    volumeInfo(scanRoot).then(setVolume).catch(console.error);
  }, [scanRoot]);

  const { load, reset } = explorer;
  useEffect(() => {
    if (scan.status === "scanning") reset();
    if (scan.status === "done") {
      load(0)
        .then(() => setScreen("breakdown"))
        .catch(console.error);
    }
  }, [scan.status, load, reset]);

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

  const canShow = (s: Screen) => s === "scan" || s === "settings" || (s === "breakdown" && scanned);
  const active = canShow(screen) ? screen : "scan";

  return (
    <div className={styles.app}>
      <Sidebar
        volume={volume}
        scanRoot={scanRoot}
        onPickRoot={pickRoot}
        scanned={scanned}
        screen={active}
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
            listOpen={listOpen}
            onToggleList={() => setListOpen((o) => !o)}
            onRescan={startScan}
            onReveal={() => reveal(explorer.currentId).catch(console.error)}
          />
        )}
        {active === "settings" && <SettingsScreen />}
      </main>
    </div>
  );
}
