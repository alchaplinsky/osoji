import { cx } from "../../components/ui";
import type { ThemeMeta } from "../../design/theme";
import styles from "./ThemeCard.module.css";

type ThemeCardProps = {
  meta: ThemeMeta;
  active: boolean;
  onSelect: () => void;
};

export default function ThemeCard({ meta, active, onSelect }: ThemeCardProps) {
  const { swatch } = meta;
  return (
    <button type="button" role="radio" aria-checked={active} className={cx(styles.card, active && styles.active)} onClick={onSelect}>
      <div className={styles.swatch} style={{ background: swatch.bg }}>
        <div className={styles.swCard} style={{ background: swatch.card }} />
        <div className={styles.swAc} style={{ background: swatch.ac }} />
      </div>
      <div className={styles.name}>{meta.label}</div>
      <div className={styles.desc}>{meta.desc}</div>
    </button>
  );
}
