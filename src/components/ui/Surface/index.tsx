import type { HTMLAttributes, ReactNode } from "react";
import { Eyebrow, Mono } from "../Text";
import { cx } from "../cx";
import styles from "./Surface.module.css";

type DivProps = HTMLAttributes<HTMLDivElement>;

export function Card({ className, ...rest }: DivProps) {
  return <div className={cx(styles.card, className)} {...rest} />;
}

export function Panel({ className, ...rest }: DivProps) {
  return <div className={cx(styles.panel, className)} {...rest} />;
}

export function StatTile({ label, value, className, ...rest }: DivProps & { label: ReactNode; value: ReactNode }) {
  return (
    <Card className={cx(styles.statTile, className)} {...rest}>
      <Eyebrow>{label}</Eyebrow>
      <Mono size={20} className={styles.statValue} style={{ fontWeight: 500 }}>
        {value}
      </Mono>
    </Card>
  );
}
