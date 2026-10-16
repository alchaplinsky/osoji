import { useEffect } from "react";
import SuggestionsScreen from "../../screens/Suggestions";
import { useBin } from "../../hooks/useBin";
import { toBinItem } from "../../lib/bin";
import type { Safety, Suggestion } from "../../lib/types";
import styles from "./BreakdownPreview.module.css";

const GB = 2 ** 30;
const DAY = 86_400;
const NOW = Math.floor(Date.now() / 1000);

let nextId = 1;

function suggestion(path: string, gb: number, safety: Safety, reason: string, daysOld = 30): Suggestion {
  const name = path.slice(path.lastIndexOf("/") + 1);
  return { id: nextId++, name, path, size: Math.round(gb * GB), mtime: NOW - daysOld * DAY, dir: true, ancestors: [0], safety, reason };
}

const SUGGESTIONS: Suggestion[] = [
  suggestion("/Users/alex/Library/Caches", 42, "safe", "Apps rebuild their caches automatically. They may open a little slower the first time after."),
  suggestion("/Users/alex/Library/Developer/Xcode/DerivedData", 16.1, "safe", "Build leftovers. Xcode recreates them the next time you open a project."),
  suggestion("/Users/alex/Library/Developer/Xcode/iOS DeviceSupport", 11.9, "safe", "Debug files for devices you may no longer connect. Xcode re-downloads them if needed."),
  suggestion("/private/var/folders", 18.2, "safe", "Temporary files macOS clears on restart."),
  suggestion("/Users/alex/Code/web/node_modules", 2.4, "safe", "Installed packages. Reinstalled by npm, pnpm or bun the next time you run install."),
  suggestion("/Users/alex/Code/unshift/src-tauri/target", 3.1, "safe", "Rust build output. Rebuilt by cargo."),
  suggestion("/Users/alex/Movies/Trip to Lofoten.fcpbundle", 41.2, "review", "Large file not modified in 2 years.", 800),
  suggestion("/Users/alex/Documents/Windows 11.pvm", 14.6, "review", "A virtual machine. Remove only if you no longer need it."),
  suggestion("/Users/alex/Downloads/Install macOS Sequoia.app", 13.2, "review", "In Downloads for 7 months, and probably already used.", 210),
  suggestion("/Users/alex/Downloads/Xcode_16.xip", 11.8, "review", "In Downloads for 4 months, and probably already used.", 125),
  suggestion("/Users/alex/Pictures/Photos Library.photoslibrary", 44.4, "keep", "Your photos. Unshift never suggests removing these."),
  suggestion("/private/var/vm", 12, "keep", "Swap files. macOS manages these itself."),
].sort((a, b) => b.size - a.size);

/** `?bin=all` starts with every safe suggestion already in the bin. */
const PREFILL = new URLSearchParams(location.search).get("bin") === "all";

export default function SuggestionsPreview() {
  const bin = useBin();
  const { add } = bin;

  useEffect(() => {
    if (PREFILL) SUGGESTIONS.filter((s) => s.safety === "safe").forEach((s) => add(toBinItem(s)));
  }, [add]);

  return (
    <div className={styles.frame}>
      <div className={styles.sidebar} />
      <main className={styles.main}>
        <SuggestionsScreen
          suggestions={SUGGESTIONS}
          binItems={bin.items}
          binIds={bin.ids}
          onToggleBin={bin.toggle}
          onAddAll={(items) => items.forEach(bin.add)}
          onRemoveMany={(ids) => ids.forEach(bin.remove)}
          onRemoveFromBin={bin.remove}
          onReview={() => console.info("review", bin.items)}
        />
      </main>
    </div>
  );
}
