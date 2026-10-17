import type { ReactNode } from "react";
import { cx } from "../ui";
import styles from "./Toolbar.module.css";

type ToolbarProps = { children: ReactNode; className?: string };

/**
 * Top strip of a screen; doubles as the window's title bar. The `deep` drag region lets a press
 * anywhere in the bar move the window and a double-click zoom it, while Tauri skips buttons,
 * inputs and elements with an interactive role, so controls inside the bar keep working.
 */
export default function Toolbar({ children, className }: ToolbarProps) {
  return (
    <div className={cx(styles.bar, className)} data-tauri-drag-region="deep">
      {children}
    </div>
  );
}

export function ToolbarTitle({ children }: { children: ReactNode }) {
  return <div className={styles.title}>{children}</div>;
}
