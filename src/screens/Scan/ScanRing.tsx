import type { ReactNode } from "react";
import { Eyebrow, Mono } from "../../components/ui";
import styles from "./ScanRing.module.css";

const SIZE = 240;
const C = SIZE / 2;
const R_OUTER = 104;
const R_INNER = 90;
const C_OUTER = 2 * Math.PI * R_OUTER;
const C_INNER = 2 * Math.PI * R_INNER;
const SPINNER_ARC = 0.25;

type ScanRingProps = {
  used: number;
  scanning: boolean;
  /** Fraction of the scan done (0–1). Null while scanning shows an indeterminate spinner instead. */
  progress?: number | null;
  value: ReactNode;
  valueSize?: number;
  label: ReactNode;
};

function dash(fraction: number, circumference: number) {
  return `${(Math.max(0, Math.min(1, fraction)) * circumference).toFixed(1)} ${circumference.toFixed(1)}`;
}

export default function ScanRing({ used, scanning, progress = null, value, valueSize = 42, label }: ScanRingProps) {
  const determinate = scanning && progress !== null;
  return (
    <div className={styles.ring}>
      <svg width={SIZE} height={SIZE} className={styles.svg}>
        <circle cx={C} cy={C} r={R_OUTER} className={styles.track} />
        <circle
          cx={C}
          cy={C}
          r={R_OUTER}
          className={styles.used}
          style={{ strokeDasharray: dash(used, C_OUTER), stroke: scanning ? "var(--line2)" : "var(--ac)" }}
        />
        {scanning && (
          <>
            <circle cx={C} cy={C} r={R_INNER} className={styles.track} />
            {determinate ? (
              <circle cx={C} cy={C} r={R_INNER} className={styles.progress} style={{ strokeDasharray: dash(progress, C_INNER) }} />
            ) : (
              <g className={styles.spinner}>
                <circle cx={C} cy={C} r={R_INNER} className={styles.spinnerArc} style={{ strokeDasharray: dash(SPINNER_ARC, C_INNER) }} />
              </g>
            )}
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
