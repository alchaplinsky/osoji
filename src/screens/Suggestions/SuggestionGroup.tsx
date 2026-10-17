import { Mono, cx } from "../../components/ui";
import { toBinItem } from "../../lib/bin";
import type { OpenItemMenu } from "../../hooks/useItemMenu";
import { humanBytes } from "../../lib/format";
import type { BinItem, Safety, Suggestion } from "../../lib/types";
import SuggestionCard from "./SuggestionCard";
import styles from "./Suggestions.module.css";

type SuggestionGroupProps = {
  safety: Safety;
  title: string;
  desc: string;
  items: Suggestion[];
  binIds: ReadonlySet<number>;
  onToggleBin: (item: BinItem) => void;
  onItemMenu?: OpenItemMenu;
};

export default function SuggestionGroup({ safety, title, desc, items, binIds, onToggleBin, onItemMenu }: SuggestionGroupProps) {
  const total = items.reduce((sum, s) => sum + s.size, 0);
  return (
    <div className={styles.group}>
      <div className={styles.groupHeader}>
        <span className={cx(styles.dot, styles[safety])} />
        <span className={styles.groupTitle}>{title}</span>
        <Mono className={styles.groupTotal}>{humanBytes(total)}</Mono>
        <span className={styles.groupDesc}>· {desc}</span>
      </div>
      <div className={styles.grid}>
        {items.map((s) => (
          <SuggestionCard
            key={s.id}
            suggestion={s}
            inBin={binIds.has(s.id)}
            onToggle={() => onToggleBin(toBinItem(s))}
            onContextMenu={
              onItemMenu &&
              ((e) =>
                onItemMenu(e, {
                  id: s.id,
                  path: s.path,
                  inBin: binIds.has(s.id),
                  // "Keep" suggestions have no bin button on the card, so the menu offers none either.
                  onToggleBin: s.safety === "keep" ? undefined : () => onToggleBin(toBinItem(s)),
                }))
            }
          />
        ))}
      </div>
    </div>
  );
}
