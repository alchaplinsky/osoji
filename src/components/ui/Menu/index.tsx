import { useEffect, useRef, type ReactNode } from "react";
import { cx } from "../cx";
import styles from "./Menu.module.css";

export type MenuItemDef = { label: ReactNode; hint?: string; onSelect: () => void };

type Props = {
  open: boolean;
  onClose: () => void;
  items: MenuItemDef[];
  className?: string;
};

export default function Menu({ open, onClose, items, className }: Props) {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      const anchor = ref.current?.parentElement;
      if (anchor && !anchor.contains(e.target as Node)) onClose();
    };
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [open, onClose]);

  if (!open) return null;
  return (
    // Menus open inside toolbars that are window drag regions; a press on the menu's own padding
    // must never start moving the window.
    <div ref={ref} role="menu" className={cx(styles.menu, className)} data-tauri-drag-region="false">
      {items.map((it, i) => (
        <button
          key={i}
          type="button"
          role="menuitem"
          className={styles.item}
          onClick={() => {
            onClose();
            it.onSelect();
          }}
        >
          <span>{it.label}</span>
          {it.hint && <span className={styles.hint}>{it.hint}</span>}
        </button>
      ))}
    </div>
  );
}
