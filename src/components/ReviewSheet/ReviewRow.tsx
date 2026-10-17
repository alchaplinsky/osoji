import type { MouseEvent } from "react";
import { Mono } from "../ui";
import { humanBytes } from "../../lib/format";
import type { BinItem } from "../../lib/types";
import styles from "./ReviewSheet.module.css";

export default function ReviewRow({ item, onContextMenu }: { item: BinItem; onContextMenu?: (e: MouseEvent) => void }) {
  return (
    <div className={styles.row} onContextMenu={onContextMenu}>
      <span className={styles.dot} />
      <div className={styles.text}>
        <div className={styles.name}>{item.name}</div>
        <Mono className={styles.path}>{item.path}</Mono>
      </div>
      <Mono className={styles.size}>{humanBytes(item.size)}</Mono>
    </div>
  );
}
