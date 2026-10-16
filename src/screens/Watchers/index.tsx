import type { BinItem, WatcherStatus } from "../../lib/types";
import Toolbar, { ToolbarTitle } from "../../components/Toolbar";
import { Button } from "../../components/ui";
import BinBar from "../Breakdown/BinBar";
import WatcherCard from "./WatcherCard";
import styles from "./Watchers.module.css";

type WatchersScreenProps = {
  statuses: WatcherStatus[];
  scanned: boolean;
  onAdd: () => void;
  onRemove: (path: string) => void;
  onSetLimit: (path: string, limit: number) => void;
  onCleanUp: (status: WatcherStatus) => void;
  binItems: BinItem[];
  onRemoveFromBin: (id: number) => void;
  onReview: () => void;
};

const ignoreDrop = () => {};

export default function WatchersScreen({
  statuses,
  scanned,
  onAdd,
  onRemove,
  onSetLimit,
  onCleanUp,
  binItems,
  onRemoveFromBin,
  onReview,
}: WatchersScreenProps) {
  return (
    <section className={styles.screen}>
      <Toolbar className={styles.toolbar}>
        <ToolbarTitle>Watchers</ToolbarTitle>
        <Button variant="outline" size="md" onClick={onAdd}>
          Watch a folder
        </Button>
      </Toolbar>
      <div className={styles.body}>
        <div className={styles.intro}>
          Unshift keeps an eye on these folders and checks them against your latest scan. A nudge when one grows past
          its limit comes once background watching lands.
        </div>
        {statuses.length === 0 && (
          <div className={styles.empty}>
            No folders watched yet. Pick one that tends to grow, like Downloads or a caches folder.
          </div>
        )}
        {statuses.map((status) => (
          <WatcherCard
            key={status.path}
            status={status}
            scanned={scanned}
            onRemove={() => onRemove(status.path)}
            onSetLimit={(limit) => onSetLimit(status.path, limit)}
            onCleanUp={() => onCleanUp(status)}
          />
        ))}
      </div>
      <BinBar items={binItems} onRemove={onRemoveFromBin} onDrop={ignoreDrop} onReview={onReview} wide={false} floating />
    </section>
  );
}
