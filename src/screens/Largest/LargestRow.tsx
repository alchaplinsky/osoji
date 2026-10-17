import type { DragEvent, MouseEvent } from "react";
import type { FileEntry } from "../../lib/types";
import { ageText } from "../../lib/age";
import { humanBytes } from "../../lib/format";
import { kindLabel } from "../../lib/kind";
import { Badge, Checkbox, Mono } from "../../components/ui";
import { startBinDrag } from "../Breakdown/BinBar";
import styles from "./Largest.module.css";

type LargestRowProps = {
  entry: FileEntry;
  safe: boolean;
  inBin: boolean;
  onToggleBin: () => void;
  onReveal: () => void;
  onContextMenu?: (e: MouseEvent) => void;
};

function parentPath(path: string): string {
  const slash = path.lastIndexOf("/");
  return slash > 0 ? path.slice(0, slash) : "/";
}

export default function LargestRow({ entry, safe, inBin, onToggleBin, onReveal, onContextMenu }: LargestRowProps) {
  return (
    <div
      className={styles.row}
      draggable
      onDragStart={(e: DragEvent) => startBinDrag(e, entry.id)}
      onDoubleClick={onReveal}
      onContextMenu={onContextMenu}
      title={entry.path}
    >
      <Checkbox checked={inBin} onChange={onToggleBin} aria-label={inBin ? `Remove ${entry.name} from the bin` : `Add ${entry.name} to the bin`} />
      <div className={styles.kind}>{kindLabel(entry)}</div>
      <div className={styles.body}>
        <div className={styles.name}>{entry.name}</div>
        <div className={styles.path}>{parentPath(entry.path)}</div>
      </div>
      <div className={styles.age}>{ageText(entry.mtime)}</div>
      <div>
        {safe && (
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
