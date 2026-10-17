import type { DragEvent } from "react";
import type { FileEntry } from "../../lib/types";
import { ageText } from "../../lib/age";
import { humanBytes } from "../../lib/format";
import { kindLabel } from "../../lib/kind";
import { isProtected } from "../../lib/bin";
import { Badge, Checkbox, Mono, cx } from "../../components/ui";
import GuardIcon from "../../components/GuardIcon";
import { startBinDrag } from "../Breakdown/BinBar";
import styles from "./Largest.module.css";

type LargestRowProps = {
  entry: FileEntry;
  safe: boolean;
  inBin: boolean;
  onToggleBin: () => void;
  onReveal: () => void;
};

function parentPath(path: string): string {
  const slash = path.lastIndexOf("/");
  return slash > 0 ? path.slice(0, slash) : "/";
}

export default function LargestRow({ entry, safe, inBin, onToggleBin, onReveal }: LargestRowProps) {
  const locked = isProtected(entry);
  return (
    <div
      className={cx(styles.row, locked && styles.locked)}
      draggable={!locked}
      onDragStart={locked ? undefined : (e: DragEvent) => startBinDrag(e, entry.id)}
      onDoubleClick={onReveal}
      title={entry.path}
    >
      {locked ? (
        <span />
      ) : (
        <Checkbox checked={inBin} onChange={onToggleBin} aria-label={inBin ? `Remove ${entry.name} from the bin` : `Add ${entry.name} to the bin`} />
      )}
      <div className={styles.kind}>{kindLabel(entry)}</div>
      <div className={styles.body}>
        <div className={styles.title}>
          <div className={styles.name}>{entry.name}</div>
          {!locked && <GuardIcon guard={entry.guard} />}
        </div>
        <div className={styles.path}>{parentPath(entry.path)}</div>
      </div>
      <div className={styles.age}>{ageText(entry.mtime)}</div>
      <div>
        {locked && (
          <Badge tone="keep" className={styles.badge} title={entry.guard?.reason}>
            Protected
          </Badge>
        )}
        {safe && !locked && (
          <Badge tone="safe" className={styles.badge}>
            Safe
          </Badge>
        )}
      </div>
      <Mono size={12.5} className={styles.size}>
        {humanBytes(entry.size)}
      </Mono>
    </div>
  );
}
