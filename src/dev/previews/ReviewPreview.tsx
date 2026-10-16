import { useState } from "react";
import ReviewSheet from "../../components/ReviewSheet";
import { Button, Toggle } from "../../components/ui";
import type { BinItem } from "../../lib/types";
import styles from "./ReviewPreview.module.css";

const GB = 1024 ** 3;
const MB = 1024 ** 2;

const ITEMS: BinItem[] = [
  { id: 12, name: "Caches", path: "/Users/alex/Library/Caches", size: 18.4 * GB, dir: true, ancestors: [0, 1, 2, 3] },
  { id: 27, name: "DerivedData", path: "/Users/alex/Library/Developer/Xcode/DerivedData", size: 14.2 * GB, dir: true, ancestors: [0, 1, 2, 3, 20, 21] },
  { id: 41, name: "node_modules", path: "/Users/alex/Projects/sweep-prototype/node_modules", size: 2.7 * GB, dir: true, ancestors: [0, 1, 2, 30, 31] },
  { id: 58, name: "macOS-Sequoia-Installer.dmg", path: "/Users/alex/Downloads/macOS-Sequoia-Installer.dmg", size: 7.3 * GB + 412 * MB, dir: false, ancestors: [0, 1, 2, 50] },
];

const ERROR = "Could not move 1 item to the Trash: permission denied for /Users/alex/Library/Caches.";

const flag = (key: string) => new URLSearchParams(location.search).get(key) === "1";

export default function ReviewPreview() {
  const [open, setOpen] = useState(() => flag("open"));
  const [busy, setBusy] = useState(() => flag("busy"));
  const [error, setError] = useState(() => flag("error"));

  return (
    <div className={styles.frame}>
      <Button variant="primary" size="lg" onClick={() => setOpen(true)}>Review 4 items</Button>
      <div className={styles.devbar}>
        busy <Toggle checked={busy} onChange={setBusy} aria-label="Busy" />
        error <Toggle checked={error} onChange={setError} aria-label="Error" />
      </div>
      <ReviewSheet
        open={open}
        items={ITEMS}
        volumeName="Macintosh HD"
        busy={busy}
        error={error ? ERROR : null}
        onCancel={() => setOpen(false)}
        onConfirm={() => setBusy(true)}
      />
    </div>
  );
}
