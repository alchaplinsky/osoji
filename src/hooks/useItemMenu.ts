import { useCallback, type MouseEvent } from "react";
import type { MenuItemDef } from "../components/ui";
import { reveal } from "../lib/api";
import { useContextMenu } from "./useContextMenu";

/** What the right-click menu needs to know about a scanned item. */
export type MenuTarget = { id: number; path: string; inBin: boolean; onToggleBin?: () => void };
export type OpenItemMenu = (e: MouseEvent, target: MenuTarget) => void;

export function itemMenuItems({ id, path, inBin, onToggleBin }: MenuTarget): MenuItemDef[] {
  return [
    { label: "Reveal in Finder", onSelect: () => reveal(id).catch(console.error) },
    ...(onToggleBin ? [{ label: inBin ? "Remove from bin" : "Add to bin", onSelect: onToggleBin }] : []),
    { label: "Copy path", onSelect: () => void Promise.resolve().then(() => navigator.clipboard.writeText(path)).catch(() => {}) },
  ];
}

/** The standard item context menu: reveal, bin toggle and copy path. */
export function useItemMenu() {
  const { open: openMenu, element } = useContextMenu();
  const open = useCallback<OpenItemMenu>((e, target) => openMenu(e, itemMenuItems(target)), [openMenu]);
  return { open, element };
}
