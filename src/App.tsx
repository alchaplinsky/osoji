import { useCallback, useEffect, useState } from "react";
import ScanControls from "./components/ScanControls";
import Breadcrumbs from "./components/Breadcrumbs";
import Sunburst from "./components/Sunburst";
import NodeDetails from "./components/NodeDetails";
import { useScan } from "./hooks/useScan";
import { getView, nodeInfo, type NodeInfo, type ViewNode } from "./lib/api";

export default function App() {
  const scan = useScan();
  const [currentId, setCurrentId] = useState<number>(0);
  const [view, setView] = useState<ViewNode | null>(null);
  const [info, setInfo] = useState<NodeInfo | null>(null);
  const [hovered, setHovered] = useState<ViewNode | null>(null);
  const [hoveredParent, setHoveredParent] = useState<ViewNode | null>(null);

  const load = useCallback(async (id: number) => {
    const [v, i] = await Promise.all([getView(id), nodeInfo(id)]);
    setView(v);
    setInfo(i);
    setCurrentId(id);
  }, []);

  useEffect(() => {
    if (scan.status === "done") load(0).catch(console.error);
    if (scan.status === "scanning") {
      setView(null);
      setInfo(null);
    }
  }, [scan.status, load]);

  const goUp = () => {
    const parent = info?.ancestors.at(-1);
    if (parent) load(parent.id).catch(console.error);
  };

  return (
    <div className="app">
      <aside className="sidebar" data-tauri-drag-region>
        <ScanControls
          status={scan.status}
          progress={scan.progress}
          summary={scan.summary}
          error={scan.error}
          onStart={(root) => scan.start(root)}
          onCancel={scan.cancel}
        />
        <hr style={{ border: 0, borderTop: "1px solid var(--border)", width: "100%" }} />
        <NodeDetails hovered={hovered} hoveredParent={hoveredParent} selected={info} onTrashed={() => load(currentId)} />
      </aside>
      <main className="main">
        <div className="titlebar" data-tauri-drag-region />
        <div className="content">
          {view && info ? (
            <>
              <Breadcrumbs ancestors={info.ancestors} current={{ id: info.id, name: info.name }} onNavigate={(id) => load(id)} />
              <Sunburst
                view={view}
                highlightId={hovered?.id ?? null}
                onHover={(n, p) => { setHovered(n); setHoveredParent(p); }}
                onDrill={(id) => load(id).catch(console.error)}
                onUp={goUp}
              />
            </>
          ) : (
            <div className="empty">{scan.status === "scanning" ? "Scanning…" : "Pick a folder and press Scan."}</div>
          )}
        </div>
      </main>
    </div>
  );
}
