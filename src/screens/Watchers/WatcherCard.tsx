import type { WatcherStatus } from "../../lib/types";
import { humanBytes } from "../../lib/format";
import { Button, Card, Icon, Mono, ProgressBar, SegmentedControl, cx } from "../../components/ui";
import { LIMIT_OPTIONS, limitLabel } from "./limits";
import styles from "./Watchers.module.css";

type WatcherCardProps = {
  status: WatcherStatus;
  scanned: boolean;
  onRemove: () => void;
  onSetLimit: (limit: number) => void;
  onCleanUp: () => void;
};

function folderName(path: string): string {
  return path.slice(path.lastIndexOf("/") + 1) || path;
}

export default function WatcherCard({ status, scanned, onRemove, onSetLimit, onCleanUp }: WatcherCardProps) {
  const { path, limit, id, size } = status;
  const found = scanned && id !== null;
  const over = found && size > limit;
  const name = folderName(path);

  let text: string;
  if (!scanned) text = "Scan to check";
  else if (!found) text = "Not in the latest scan";
  else if (over) text = `${humanBytes(size)} · ${humanBytes(size - limit)} over the ${limitLabel(limit)} limit`;
  else text = `${humanBytes(size)} of ${limitLabel(limit)}`;

  return (
    <Card className={styles.card}>
      <div className={styles.head}>
        <span className={styles.name}>{name}</span>
        <Mono className={styles.path} title={path}>
          {path}
        </Mono>
      </div>
      <div className={styles.actions}>
        {over && (
          <Button variant="primary" size="md" className={styles.cleanUp} onClick={onCleanUp}>
            Clean up
          </Button>
        )}
        <SegmentedControl options={LIMIT_OPTIONS} value={String(limit)} onChange={(v) => onSetLimit(Number(v))} />
        <button type="button" className={styles.remove} aria-label={`Stop watching ${name}`} onClick={onRemove}>
          <Icon name="close" />
        </button>
      </div>
      <div className={styles.meter}>
        <ProgressBar
          className={styles.bar}
          value={found ? (size / limit) * 100 : 0}
          color={over ? "var(--review)" : "var(--ac)"}
        />
        <Mono className={cx(styles.status, over ? styles.over : found ? styles.under : styles.unknown)}>{text}</Mono>
      </div>
    </Card>
  );
}
