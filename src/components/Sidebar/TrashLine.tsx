import { showTrash, type TrashInfo } from "../../lib/api";
import { humanBytes } from "../../lib/format";
import { fileManager } from "../../lib/platform";
import { Mono } from "../ui";
import styles from "./TrashLine.module.css";

/** What already sits in the Trash. Osoji never empties it; the file manager does. */
export default function TrashLine({ trash }: { trash: TrashInfo }) {
  return (
    <div className={styles.line}>
      <Mono size={11} className={styles.size}>
        Trash · {humanBytes(trash.size)}
      </Mono>
      <button type="button" className={styles.show} title={trash.path} onClick={() => showTrash().catch(console.error)}>
        Show in {fileManager()}
      </button>
    </div>
  );
}
