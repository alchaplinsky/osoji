import { useMemo, useState } from "react";
import LargestScreen from "../../screens/Largest";
import type { BinItem, FileEntry } from "../../lib/types";
import { toBinItem } from "../../lib/bin";
import styles from "./LargestPreview.module.css";

const GB = 2 ** 30;
const MONTH = 30.44 * 86_400;
const NOW = Math.floor(Date.now() / 1000);

const HOME = 10;
const LIBRARY = 11;
const CORE_SIM_CACHES = 12;

function entry(id: number, path: string, gb: number, monthsAgo: number, dir: boolean, ancestors: number[]): FileEntry {
  return {
    id,
    name: path.slice(path.lastIndexOf("/") + 1),
    path,
    size: Math.round(gb * GB),
    mtime: Math.round(NOW - monthsAgo * MONTH - 86_400),
    dir,
    ancestors: [0, ...ancestors],
  };
}

const ENTRIES: FileEntry[] = [
  entry(101, "/Users/alex/Pictures/Photos Library.photoslibrary", 88.4, 0, true, [HOME]),
  entry(102, "/Users/alex/Library/Caches", 42, 0, true, [HOME, LIBRARY]),
  entry(103, "/Users/alex/Movies/Trip to Lofoten.fcpbundle", 41.2, 7, true, [HOME]),
  entry(104, "/Applications/Xcode.app", 31.4, 1, true, []),
  entry(105, "/.MobileBackups/Local snapshots", 30, 0, false, []),
  entry(106, "/Applications/Baldur's Gate 3.app", 28.1, 14, true, []),
  entry(107, "/Users/alex/Documents/Windows 11.pvm", 24.6, 26, true, [HOME]),
  entry(108, "/Users/alex/Library/Developer/Xcode/DerivedData", 22.1, 1, true, [HOME, LIBRARY]),
  entry(109, "/Users/alex/Downloads/Install macOS Sequoia.app", 13.2, 9, true, [HOME]),
  entry(110, "/Users/alex/Downloads/Xcode_16.xip", 11.8, 13, false, [HOME]),
  entry(111, "/Users/alex/Library/Developer/CoreSimulator/Caches/dyld/iOS 17.4.dmg", 7.9, 4, false, [HOME, LIBRARY, CORE_SIM_CACHES]),
  entry(112, "/Users/alex/Movies/Screen Recording 2025-03-02.mov", 4.8, 3, false, [HOME]),
  entry(113, "/Users/alex/Downloads/ubuntu-24.04-desktop-arm64.iso", 3.1, 20, false, [HOME]),
  entry(114, "/Users/alex/Downloads/Archives/photos-2019.zip", 2.6, 38, false, [HOME]),
  entry(115, "/Users/alex/Projects/datasets/events.parquet", 1.9, 5, false, [HOME]),
].sort((a, b) => b.size - a.size);

const SAFE_IDS: ReadonlySet<number> = new Set([102, 108, CORE_SIM_CACHES]);

/** `?bin=N` starts with the N largest entries already in the bin. */
function initialBin(): BinItem[] {
  const n = Number(new URLSearchParams(location.search).get("bin")) || 0;
  return ENTRIES.slice(0, n).map(toBinItem);
}

export default function LargestPreview() {
  const [bin, setBin] = useState<BinItem[]>(initialBin);
  const binIds = useMemo(() => new Set(bin.map((b) => b.id)), [bin]);

  const toggleBin = (item: BinItem) => {
    if (binIds.has(item.id)) setBin(bin.filter((b) => b.id !== item.id));
    else setBin(bin.filter((b) => !b.ancestors.includes(item.id)).concat(item));
  };

  return (
    <div className={styles.frame}>
      <div className={styles.sidebar} />
      <main className={styles.main}>
        <LargestScreen
          entries={ENTRIES}
          safeIds={SAFE_IDS}
          binItems={bin}
          binIds={binIds}
          onToggleBin={toggleBin}
          onRemoveFromBin={(id) => setBin(bin.filter((b) => b.id !== id))}
          onReview={() => console.info("review", bin)}
          onReveal={(id) => console.info("reveal", id)}
        />
      </main>
    </div>
  );
}
