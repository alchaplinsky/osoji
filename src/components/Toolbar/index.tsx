import type { ReactNode } from "react";
import { cx } from "../ui";
import styles from "./Toolbar.module.css";

type ToolbarProps = { children: ReactNode; className?: string };

/** Screen header bar; doubles as the window drag region. */
export default function Toolbar({ children, className }: ToolbarProps) {
  return (
    <div className={cx(styles.bar, className)} data-tauri-drag-region>
      {children}
    </div>
  );
}

export function ToolbarTitle({ children }: { children: ReactNode }) {
  return (
    <div className={styles.title} data-tauri-drag-region>
      {children}
    </div>
  );
}
