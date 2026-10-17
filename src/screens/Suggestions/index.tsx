import { useMemo } from "react";
import Toolbar, { ToolbarTitle } from "../../components/Toolbar";
import { Button, Mono } from "../../components/ui";
import { toBinItem } from "../../lib/bin";
import type { OpenItemMenu } from "../../hooks/useItemMenu";
import { humanBytes } from "../../lib/format";
import type { BinItem, Safety, Suggestion } from "../../lib/types";
import BinBar from "../Breakdown/BinBar";
import SuggestionGroup from "./SuggestionGroup";
import styles from "./Suggestions.module.css";

type SuggestionsScreenProps = {
  suggestions: Suggestion[];
  binItems: BinItem[];
  binIds: ReadonlySet<number>;
  onToggleBin: (item: BinItem) => void;
  onAddAll: (items: BinItem[]) => void;
  onRemoveMany: (ids: number[]) => void;
  onRemoveFromBin: (id: number) => void;
  onReview: () => void;
  onItemMenu?: OpenItemMenu;
};

const GROUPS: { safety: Safety; title: string; desc: string }[] = [
  { safety: "safe", title: "Safe to remove", desc: "Rebuilt automatically or already installed elsewhere" },
  { safety: "review", title: "Worth a look", desc: "Probably fine to remove, but only you can say" },
  { safety: "keep", title: "We would keep these", desc: "Personal or managed by macOS" },
];

export default function SuggestionsScreen({
  suggestions,
  binItems,
  binIds,
  onToggleBin,
  onAddAll,
  onRemoveMany,
  onRemoveFromBin,
  onReview,
  onItemMenu,
}: SuggestionsScreenProps) {
  const bySafety = useMemo(() => {
    const groups: Record<Safety, Suggestion[]> = { safe: [], review: [], keep: [] };
    for (const s of suggestions) groups[s.safety].push(s);
    return groups;
  }, [suggestions]);

  const safe = bySafety.safe;
  const safeTotal = safe.reduce((sum, s) => sum + s.size, 0);
  const allSafeAdded = safe.length > 0 && safe.every((s) => binIds.has(s.id));

  const drop = (id: number) => {
    const s = suggestions.find((x) => x.id === id);
    if (s && !binIds.has(id)) onToggleBin(toBinItem(s));
  };

  return (
    <section className={styles.screen}>
      <Toolbar className={styles.toolbar}>
        <ToolbarTitle>Suggestions</ToolbarTitle>
        {allSafeAdded ? (
          <Button onClick={() => onRemoveMany(safe.map((s) => s.id))}>Remove safe items from bin</Button>
        ) : (
          <Button variant="primary" disabled={safe.length === 0} onClick={() => onAddAll(safe.map(toBinItem))}>
            Add all safe items
          </Button>
        )}
      </Toolbar>
      <div className={styles.body}>
        {suggestions.length === 0 ? (
          <div className={styles.empty}>Nothing to suggest. Everything looks like it belongs.</div>
        ) : (
          <>
            <div className={styles.headline}>
              You could free <Mono className={styles.amount}>{humanBytes(safeTotal)}</Mono> without losing anything you'd miss.
            </div>
            <div className={styles.sub}>
              Safe items are rebuilt automatically or already live somewhere else. Items worth a look are probably fine to remove, but
              only you can say for sure.
            </div>
            {GROUPS.map(
              (g) =>
                bySafety[g.safety].length > 0 && (
                  <SuggestionGroup key={g.safety} {...g} items={bySafety[g.safety]} binIds={binIds} onToggleBin={onToggleBin} onItemMenu={onItemMenu} />
                ),
            )}
          </>
        )}
      </div>
      <BinBar floating wide items={binItems} onRemove={onRemoveFromBin} onDrop={drop} onReview={onReview} />
    </section>
  );
}
