import { showTrash, type TrashInfo } from "../../lib/api";
import { humanBytes } from "../../lib/format";
import { Mono } from "../ui";
import styles from "./TrashLine.module.css";

/** What already sits in the Trash. Unshift never empties it; Finder does. */
export default function TrashLine({ trash }: { trash: TrashInfo }) {
  return (
    <div className={styles.line}>
      <Mono size={11} className={styles.size}>
        Trash · {humanBytes(trash.size)}
      </Mono>
      <button type="button" className={styles.show} title={trash.path} onClick={() => showTrash().catch(console.error)}>
        Show in Finder
      </button>
    </div>
  );
}
