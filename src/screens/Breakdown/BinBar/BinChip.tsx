import type { BinItem } from "../../../lib/types";
import { humanBytes } from "../../../lib/format";
import { Icon, Mono } from "../../../components/ui";
import styles from "./BinBar.module.css";

type BinChipProps = {
  item: BinItem;
  onRemove: () => void;
};

export default function BinChip({ item, onRemove }: BinChipProps) {
  return (
    <div className={styles.chip} title={item.path}>
      <span className={styles.chipName}>{item.name}</span>
      <Mono className={styles.chipSize}>{humanBytes(item.size)}</Mono>
      <button type="button" className={styles.remove} aria-label={`Remove ${item.name}`} onClick={onRemove}>
        <Icon name="close" />
      </button>
    </div>
  );
}
