import type { ThemeName } from "../../design/theme";
import { neutral } from "../../design/palette";
import type { NodeInfo, ViewNode } from "../../lib/api";
import type { HoverId } from "../../lib/types";
import { findNode, pathTo } from "../../lib/tree";
import type { Disk } from "../../components/viz";
import { EXTRA_NAMES } from "../../components/Sunburst/layout";
import { nodeFill, nodeName, type Slice } from "./slices";

export type HoverTarget = { name: string; size: number; pct: number; fill: string; error: boolean };

/**
 * Name, size, share and swatch colour of the hovered slice, looked up in `view` first, then in `root`.
 * `other` is the chart's merged "smaller items" slice when it covers more than `view.other`.
 * Shares of `view` are of the whole `disk` when the chart spans one.
 */
export function hoverTarget(view: ViewNode, root: ViewNode, theme: ThemeName, hoverId: HoverId, other?: Slice, disk?: Disk): HoverTarget | null {
  if (hoverId === null) return null;
  const whole = (tree: ViewNode) => (tree === view && disk ? disk.total : tree.size);
  if (hoverId === "hidden" || hoverId === "free") {
    if (!disk) return null;
    return { name: EXTRA_NAMES[hoverId], size: disk[hoverId], pct: share(disk[hoverId], disk.total), fill: neutral(theme), error: false };
  }
  if (hoverId === "other") {
    const name = other?.name ?? `${view.other_count} smaller items`;
    const size = other?.size ?? view.other;
    return { name, size, pct: share(size, whole(view)), fill: neutral(theme), error: false };
  }
  for (const tree of [view, root]) {
    const path = pathTo(tree, hoverId);
    const node = path.at(-1);
    if (!node || path.length < 2) continue;
    const top = tree.children.indexOf(path[1]);
    return { name: nodeName(node), size: node.size, pct: share(node.size, whole(tree)), fill: nodeFill(path[1], theme, top), error: node.error };
  }
  return null;
}

const share = (size: number, of: number) => (of > 0 ? (size / of) * 100 : 0);

/**
 * The capacity bar only knows the root's children. Map a hover it cannot resolve
 * (a deep node, or a drilled-in view's "other" slice) to the root child on the current path.
 */
export function capacityHover(root: ViewNode, info: NodeInfo, hoverId: HoverId): HoverId {
  if (hoverId === null || hoverId === "hidden" || hoverId === "free") return hoverId;
  const known = hoverId === "other" ? info.id === root.id : findNode(root, hoverId) !== null;
  if (known) return hoverId;
  return info.ancestors[1]?.id ?? info.id;
}
