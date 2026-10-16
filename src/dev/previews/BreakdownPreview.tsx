import { useMemo, useState } from "react";
import BreakdownScreen from "../../screens/Breakdown";
import { useTheme } from "../../hooks/useTheme";
import type { NodeInfo, ViewNode } from "../../lib/api";
import type { BinItem, HoverId, Volume } from "../../lib/types";
import { findNode, pathTo } from "../../lib/tree";
import styles from "./BreakdownPreview.module.css";

const GB = 2 ** 30;

let nextId = 1;

function file(name: string, gb: number): ViewNode {
  return { id: nextId++, name, size: Math.round(gb * GB), items: 1, dir: false, error: false, children: [], other: 0, other_count: 0 };
}

function dir(name: string, children: ViewNode[], tail?: { count: number; gb: number }): ViewNode {
  const other = tail ? Math.round(tail.gb * GB) : 0;
  const sorted = [...children].sort((a, b) => b.size - a.size);
  return {
    id: nextId++,
    name,
    size: sorted.reduce((s, c) => s + c.size, other),
    items: sorted.reduce((s, c) => s + c.items, tail?.count ?? 0),
    dir: true,
    error: false,
    children: sorted,
    other,
    other_count: tail?.count ?? 0,
  };
}

const ROOT: ViewNode = {
  ...dir(
    "Macintosh HD",
    [
      dir("Users", [
        dir("alex", [
          dir("Library", [
            file("Caches", 42),
            dir("Application Support", [file("Adobe", 18.2), file("Steam", 12.6), file("Spotify", 7.2)]),
            dir("Developer", [file("Xcode DerivedData", 16.1), file("iOS DeviceSupport", 11.9)]),
            file("Containers", 6.4),
            dir("Mail", [file("Attachments", 2.6), file("Envelope Index", 0.8)]),
          ]),
          dir("Movies", [
            file("Trip to Lofoten.fcpbundle", 41.2),
            file("iMovie Library", 12.6),
            dir("Screen Recordings", [file("Screen Recording 2025-03-02.mov", 4.8), file("Screen Recording 2024-11-18.mov", 2.6), file("Screen Recording 2024-08-04.mov", 0.8)]),
          ]),
          dir("Pictures", [file("Photos Library.photoslibrary", 44.4), file("Lightroom", 7.6)]),
          dir("Downloads", [file("Install macOS Sequoia.app", 13.2), file("Xcode_16.xip", 11.8), file("Archives", 6.4)], { count: 42, gb: 4.6 }),
          dir("Documents", [file("Windows 11.pvm", 14.6), file("Work", 4.2), file("Personal", 2.2)]),
          dir("Music", [file("Music Library", 6.3), file("Logic Projects", 0.9)]),
        ]),
        dir("Shared", [file("Adobe", 2.8), file("Parallels", 1.2)]),
      ]),
      dir(
        "Applications",
        [file("Xcode.app", 31.4), file("Baldur's Gate 3.app", 28.1), file("Adobe Creative Cloud", 22.3), file("Final Cut Pro.app", 4.2)],
        { count: 38, gb: 6 },
      ),
      dir("private", [dir("var", [file("folders", 18.2), file("vm", 12), file("log", 11.8)])]),
      dir("Library", [file("Application Support", 20.4), file("Updates", 11.6), file("Caches", 6)]),
      file("Local snapshots", 30),
      dir("System", [dir("Library", [file("Frameworks", 14.2), file("PrivateFrameworks", 6.8), file("Extensions", 3)])]),
    ],
    { count: 11, gb: 18 },
  ),
  id: 0,
};

const VOLUME: Volume = { name: "Macintosh HD", mount: "/", total: 1024 * GB, free: 412 * GB };

function infoFor(node: ViewNode): NodeInfo {
  const ancestors = pathTo(ROOT, node.id)
    .slice(0, -1)
    .map(({ id, name }) => ({ id, name }));
  const path = "/" + ancestors.slice(1).map((c) => c.name).concat(node.id === 0 ? [] : [node.name]).join("/");
  return { id: node.id, name: node.name, path, size: node.size, items: node.items, dir: node.dir, error: false, trashed: false, mtime: 0, ancestors };
}

function binItemFor(id: number): BinItem | null {
  const path = pathTo(ROOT, id);
  const node = path.at(-1);
  if (!node || node.id === 0) return null;
  return { id, name: node.name, path: infoFor(node).path, size: node.size, dir: node.dir, ancestors: path.slice(0, -1).map((n) => n.id) };
}

function leaves(node: ViewNode): ViewNode[] {
  return node.dir ? node.children.flatMap(leaves) : [node];
}

/** `?bin=N` starts with the N largest files already in the bin. */
function initialBin(): BinItem[] {
  const n = Number(new URLSearchParams(location.search).get("bin")) || 0;
  return leaves(ROOT)
    .sort((a, b) => b.size - a.size)
    .slice(0, n)
    .flatMap((leaf) => binItemFor(leaf.id) ?? []);
}

export default function BreakdownPreview() {
  const { theme } = useTheme();
  const [view, setView] = useState<ViewNode>(ROOT);
  const [hoverId, setHoverId] = useState<HoverId>(null);
  const [listOpen, setListOpen] = useState(true);
  const [bin, setBin] = useState<BinItem[]>(initialBin);
  const info = useMemo(() => infoFor(view), [view]);

  const toggleBin = (id: number) => {
    if (bin.some((b) => b.id === id)) {
      setBin(bin.filter((b) => b.id !== id));
      return;
    }
    const item = binItemFor(id);
    if (item) setBin(bin.filter((b) => !b.ancestors.includes(id)).concat(item));
  };

  const navigate = (id: number) => {
    const node = findNode(ROOT, id);
    if (!node?.dir) return;
    setHoverId(null);
    setView(node);
  };

  return (
    <div className={styles.frame}>
      <div className={styles.sidebar} />
      <main className={styles.main}>
        <BreakdownScreen
          volume={VOLUME}
          rootView={ROOT}
          view={view}
          info={info}
          theme={theme}
          hoverId={hoverId}
          onHover={setHoverId}
          onNavigate={navigate}
          onUp={() => {
            const parent = info.ancestors.at(-1);
            if (parent) navigate(parent.id);
          }}
          listOpen={listOpen}
          onToggleList={() => setListOpen((o) => !o)}
          onRescan={() => console.info("rescan")}
          onReveal={() => console.info("reveal", info.path)}
          binItems={bin}
          onToggleBin={toggleBin}
          onRemoveFromBin={(id) => setBin(bin.filter((b) => b.id !== id))}
          onReview={() => console.info("review", bin)}
        />
      </main>
    </div>
  );
}
