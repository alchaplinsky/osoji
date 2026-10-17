import { useMemo, useState } from "react";
import type { BinItem, FileEntry } from "../../lib/types";
import { isOlderThan } from "../../lib/age";
import { toBinItem } from "../../lib/bin";
import type { OpenItemMenu } from "../../hooks/useItemMenu";
import Toolbar, { ToolbarTitle } from "../../components/Toolbar";
import { Panel, SegmentedControl, type SegmentOption } from "../../components/ui";
import BinBar from "../Breakdown/BinBar";
import LargestRow from "./LargestRow";
import styles from "./Largest.module.css";

type LargestScreenProps = {
  entries: FileEntry[];
  safeIds: ReadonlySet<number>;
  binItems: BinItem[];
  binIds: ReadonlySet<number>;
  onToggleBin: (item: BinItem) => void;
  onRemoveFromBin: (id: number) => void;
  onReview: () => void;
  onReveal: (id: number) => void;
  onItemMenu?: OpenItemMenu;
};

type Filter = "all" | "safe" | "old";

const FILTERS: readonly SegmentOption<Filter>[] = [
  { value: "all", label: "All" },
  { value: "safe", label: "Safe to remove" },
  { value: "old", label: "Over a year old" },
];

export default function LargestScreen({ entries, safeIds, binItems, binIds, onToggleBin, onRemoveFromBin, onReview, onReveal, onItemMenu }: LargestScreenProps) {
  const [filter, setFilter] = useState<Filter>("all");

  const safe = useMemo(
    () => new Set(entries.filter((e) => safeIds.has(e.id) || e.ancestors.some((id) => safeIds.has(id))).map((e) => e.id)),
    [entries, safeIds],
  );

  const rows = useMemo(() => {
    if (filter === "safe") return entries.filter((e) => safe.has(e.id));
    if (filter === "old") return entries.filter((e) => isOlderThan(e.mtime, 365));
    return entries;
  }, [entries, safe, filter]);

  const drop = (id: number) => {
    const entry = entries.find((e) => e.id === id);
    if (entry && !binIds.has(id)) onToggleBin(toBinItem(entry));
  };

  return (
    <section className={styles.screen}>
      <Toolbar className={styles.toolbar}>
        <ToolbarTitle>Largest files</ToolbarTitle>
        <SegmentedControl options={FILTERS} value={filter} onChange={setFilter} />
      </Toolbar>
      <Panel className={styles.panel}>
        <div className={styles.list}>
          {rows.length === 0 && <div className={styles.empty}>Nothing matches this filter.</div>}
          {rows.map((entry) => (
            <LargestRow
              key={entry.id}
              entry={entry}
              safe={safe.has(entry.id)}
              inBin={binIds.has(entry.id)}
              onToggleBin={() => onToggleBin(toBinItem(entry))}
              onReveal={() => onReveal(entry.id)}
              onContextMenu={
                onItemMenu &&
                ((e) => onItemMenu(e, { id: entry.id, path: entry.path, inBin: binIds.has(entry.id), onToggleBin: () => onToggleBin(toBinItem(entry)) }))
              }
            />
          ))}
        </div>
      </Panel>
      <BinBar items={binItems} onRemove={onRemoveFromBin} onDrop={drop} onReview={onReview} wide={false} floating />
    </section>
  );
}
