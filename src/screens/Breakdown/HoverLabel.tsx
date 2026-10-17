import { humanBytes } from "../../lib/format";
import { Mono } from "../../components/ui";
import type { HoverTarget } from "./hover";
import styles from "./Breakdown.module.css";

type HoverLabelProps = { target: HoverTarget | null; hint: string };

export default function HoverLabel({ target, hint }: HoverLabelProps) {
  return (
    <div className={styles.hoverLabel}>
      {target ? (
        <>
          <span className={styles.hoverSwatch} style={{ background: target.fill }} />
          <span className={styles.hoverName}>{target.name}</span>
          <Mono className={styles.hoverMeta}>
            {humanBytes(target.size)} · {target.pct.toFixed(target.pct < 10 ? 1 : 0)}%{target.error && " · Can't read"}
          </Mono>
        </>
      ) : (
        <span className={styles.hint}>{hint}</span>
      )}
    </div>
  );
}
