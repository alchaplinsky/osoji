import { useEffect, useLayoutEffect, useRef, type ReactNode } from "react";
import { cx } from "../cx";
import styles from "./Menu.module.css";

/**
 * `icon` is any 16px glyph, typically a lucide-react icon rendered with `size={16}`.
 * `checked` turns the item into a radio choice and highlights it when true.
 */
export type MenuItemDef = { label: ReactNode; icon?: ReactNode; hint?: string; checked?: boolean; onSelect: () => void };

type Props = {
  open: boolean;
  onClose: () => void;
  items: MenuItemDef[];
  /** Viewport coordinates to open at (a context menu). Without it the menu hangs off its positioned parent. */
  at?: { x: number; y: number };
  className?: string;
};

/** Gap kept between a positioned menu and the window edges. */
const EDGE = 8;

const clamp = (v: number, min: number, max: number) => Math.max(min, Math.min(v, max));

export default function Menu({ open, onClose, items, at, className }: Props) {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      const inside = at ? ref.current : ref.current?.parentElement;
      if (inside && !inside.contains(e.target as Node)) onClose();
    };
    // Capture phase so an open menu takes Escape before anything underneath it (e.g. a sheet) closes too.
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.stopPropagation();
      onClose();
    };
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey, true);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey, true);
    };
  }, [open, onClose, at]);

  // Placed before paint; offset sizes ignore the pop-in scale so the clamp uses the settled size.
  useLayoutEffect(() => {
    const el = ref.current;
    if (!open || !at || !el) return;
    el.style.left = `${clamp(at.x, EDGE, window.innerWidth - el.offsetWidth - EDGE)}px`;
    el.style.top = `${clamp(at.y, EDGE, window.innerHeight - el.offsetHeight - EDGE)}px`;
  }, [open, at]);

  if (!open) return null;
  return (
    // Menus open inside toolbars that are window drag regions; a press on the menu's own padding
    // must never start moving the window.
    <div
      ref={ref}
      role="menu"
      className={cx(styles.menu, at && styles.at, className)}
      data-tauri-drag-region="false"
      onContextMenu={(e) => e.preventDefault()}
    >
      {items.map((it, i) => (
        <button
          key={i}
          type="button"
          role={it.checked === undefined ? "menuitem" : "menuitemradio"}
          aria-checked={it.checked}
          className={cx(styles.item, it.checked && styles.checked)}
          onClick={() => {
            onClose();
            it.onSelect();
          }}
        >
          <span className={styles.label}>
            {it.icon && <span className={styles.icon}>{it.icon}</span>}
            <span className={styles.text}>{it.label}</span>
          </span>
          {it.hint && <span className={styles.hint}>{it.hint}</span>}
        </button>
      ))}
    </div>
  );
}
