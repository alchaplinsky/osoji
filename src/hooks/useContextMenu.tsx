import { useCallback, useState, type MouseEvent } from "react";
import { Menu, type MenuItemDef } from "../components/ui";

type OpenMenu = { at: { x: number; y: number }; items: MenuItemDef[] };

/** One right-click menu: `open` it from an `onContextMenu` handler and render `element` once. */
export function useContextMenu() {
  const [menu, setMenu] = useState<OpenMenu | null>(null);
  const close = useCallback(() => setMenu(null), []);
  const open = useCallback((e: MouseEvent, items: MenuItemDef[]) => {
    e.preventDefault();
    setMenu({ at: { x: e.clientX, y: e.clientY }, items });
  }, []);

  const element = <Menu open={menu !== null} onClose={close} items={menu?.items ?? []} at={menu?.at} />;
  return { open, element };
}
