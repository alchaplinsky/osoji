import { HardDrive } from "lucide-react";
import type { Crumb } from "../../lib/api";
import { Icon, cx } from "../ui";
import styles from "./Breadcrumbs.module.css";

type BreadcrumbsProps = { ancestors: Crumb[]; current: Crumb; onNavigate: (id: number) => void };

/** A root-of-volume scan names its top node "/", which reads better as a drive glyph. */
const isVolumeRoot = (c: Crumb, i: number) => i === 0 && c.name === "/";

export default function Breadcrumbs({ ancestors, current, onNavigate }: BreadcrumbsProps) {
  const chain = [...ancestors, current];
  return (
    <nav className={styles.root}>
      {chain.map((c, i) => {
        const drive = isVolumeRoot(c, i);
        return (
          <div key={c.id} className={styles.crumb}>
            {i > 0 && <Icon name="chevronRight" size={12} className={styles.sep} />}
            <button
              type="button"
              className={cx(styles.btn, i === chain.length - 1 && styles.current)}
              aria-label={drive ? "Volume root" : undefined}
              title={drive ? c.name : undefined}
              onClick={() => onNavigate(c.id)}
            >
              {drive ? <HardDrive size={14} strokeWidth={1.75} /> : <span>{c.name}</span>}
            </button>
          </div>
        );
      })}
    </nav>
  );
}
