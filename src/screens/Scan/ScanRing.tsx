import type { ReactNode } from "react";
import { Eyebrow, Mono } from "../../components/ui";
import styles from "./ScanRing.module.css";

const SIZE = 240;
const C = SIZE / 2;
const R_OUTER = 104;
const R_INNER = 90;
const C_OUTER = 2 * Math.PI * R_OUTER;
const C_INNER = 2 * Math.PI * R_INNER;

type ScanRingProps = {
  /** 0 to 1; the share of the volume in use. */
  used: number;
  /** Shows the indeterminate inner arc and dims the used arc. */
  scanning: boolean;
  value: ReactNode;
  /** Font size of the centre value in px. */
  valueSize?: number;
  label: ReactNode;
};

export default function ScanRing({ used, scanning, value, valueSize = 42, label }: ScanRingProps) {
  const usedLen = Math.max(0, Math.min(1, used)) * C_OUTER;
  return (
    <div className={styles.ring}>
      <svg width={SIZE} height={SIZE} className={styles.svg}>
        <circle cx={C} cy={C} r={R_OUTER} className={styles.track} />
        <circle
          cx={C}
          cy={C}
          r={R_OUTER}
          className={styles.used}
          style={{ strokeDasharray: `${usedLen.toFixed(1)} ${C_OUTER.toFixed(1)}`, stroke: scanning ? "var(--line2)" : "var(--ac)" }}
        />
        {scanning && (
          <>
            <circle cx={C} cy={C} r={R_INNER} className={styles.track} />
            <g className={styles.spinner}>
              <circle
                cx={C}
                cy={C}
                r={R_INNER}
                className={styles.progress}
                style={{ strokeDasharray: `${(C_INNER * 0.25).toFixed(1)} ${C_INNER.toFixed(1)}` }}
              />
            </g>
          </>
        )}
      </svg>
      <div className={styles.centre}>
        <Mono numeral size={valueSize}>{value}</Mono>
        <Eyebrow className={styles.label}>{label}</Eyebrow>
      </div>
    </div>
  );
}
