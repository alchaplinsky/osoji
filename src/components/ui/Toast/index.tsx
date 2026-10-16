import type { ReactNode } from "react";
import Icon from "../Icon";
import styles from "./Toast.module.css";

type Props = { title: ReactNode; sub?: ReactNode };

export default function Toast({ title, sub }: Props) {
  return (
    <div role="status" className={styles.toast}>
      <div className={styles.icon}>
        <Icon name="checkLg" strokeWidth={1.8} />
      </div>
      <div>
        <div className={styles.title}>{title}</div>
        {sub && <div className={styles.sub}>{sub}</div>}
      </div>
    </div>
  );
}
