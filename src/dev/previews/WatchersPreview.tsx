import { useState } from "react";
import WatchersScreen from "../../screens/Watchers";
import { GB } from "../../lib/format";
import type { WatcherStatus } from "../../lib/types";
import styles from "./LargestPreview.module.css";

const STATUSES: WatcherStatus[] = [
  { path: "/Users/alex/Downloads", limit: 20 * GB, id: 31, size: Math.round(26.4 * GB), dir: true },
  { path: "/Users/alex/Library/Caches", limit: 50 * GB, id: 42, size: 42 * GB, dir: true },
  { path: "/Users/alex/Movies/Screen Recordings", limit: 10 * GB, id: null, size: 0, dir: false },
];

const EXTRA: WatcherStatus = { path: "/Users/alex/Library/Developer/Xcode/DerivedData", limit: 10 * GB, id: 57, size: Math.round(16.1 * GB), dir: true };

const EMPTY = new URLSearchParams(location.search).get("empty") === "1";

export default function WatchersPreview() {
  const [statuses, setStatuses] = useState<WatcherStatus[]>(EMPTY ? [] : STATUSES);

  return (
    <div className={styles.frame}>
      <div className={styles.sidebar} />
      <main className={styles.main}>
        <WatchersScreen
          statuses={statuses}
          scanned
          onAdd={() => setStatuses((s) => (s.some((w) => w.path === EXTRA.path) ? s : [...s, EXTRA]))}
          onRemove={(path) => setStatuses((s) => s.filter((w) => w.path !== path))}
          onSetLimit={(path, limit) => setStatuses((s) => s.map((w) => (w.path === path ? { ...w, limit } : w)))}
          onCleanUp={(status) => console.info("clean up", status)}
          binItems={[]}
          onRemoveFromBin={(id) => console.info("remove from bin", id)}
          onReview={() => console.info("review")}
        />
      </main>
    </div>
  );
}
