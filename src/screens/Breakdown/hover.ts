import type { ThemeName } from "../../design/theme";
import { neutral, tone } from "../../design/palette";
import type { NodeInfo, ViewNode } from "../../lib/api";
import type { HoverId } from "../../lib/types";
import { findNode, pathTo } from "../../lib/tree";

export type HoverTarget = { name: string; size: number; pct: number; fill: string };

/** Name, size, share and swatch colour of the hovered slice, looked up in `view` first, then in `root`. */
export function hoverTarget(view: ViewNode, root: ViewNode, theme: ThemeName, hoverId: HoverId): HoverTarget | null {
  if (hoverId === null) return null;
  if (hoverId === "other") {
    return { name: `${view.other_count} smaller items`, size: view.other, pct: share(view.other, view), fill: neutral(theme) };
  }
  for (const tree of [view, root]) {
    const path = pathTo(tree, hoverId);
    const node = path.at(-1);
    if (!node || path.length < 2) continue;
    const top = tree.children.indexOf(path[1]);
    return { name: node.name, size: node.size, pct: share(node.size, tree), fill: tone(theme, top) };
  }
  return null;
}

const share = (size: number, of: ViewNode) => (of.size > 0 ? (size / of.size) * 100 : 0);

/**
 * The capacity bar only knows the root's children. Map a hover it cannot resolve
 * (a deep node, or a drilled-in view's "other" slice) to the root child on the current path.
 */
export function capacityHover(root: ViewNode, info: NodeInfo, hoverId: HoverId): HoverId {
  if (hoverId === null) return null;
  const known = hoverId === "other" ? info.id === root.id : findNode(root, hoverId) !== null;
  if (known) return hoverId;
  return info.ancestors[1]?.id ?? info.id;
}
