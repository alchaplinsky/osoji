import type { Crumb } from "../../lib/api";
import { Icon, cx } from "../ui";
import styles from "./Breadcrumbs.module.css";

type BreadcrumbsProps = { ancestors: Crumb[]; current: Crumb; onNavigate: (id: number) => void };

export default function Breadcrumbs({ ancestors, current, onNavigate }: BreadcrumbsProps) {
  const chain = [...ancestors, current];
  return (
    <nav className={styles.root}>
      {chain.map((c, i) => (
        <div key={c.id} className={styles.crumb}>
          {i > 0 && <Icon name="chevronRight" size={12} className={styles.sep} />}
          <button type="button" className={cx(styles.btn, i === chain.length - 1 && styles.current)} onClick={() => onNavigate(c.id)}>
            {c.name}
          </button>
        </div>
      ))}
    </nav>
  );
}
