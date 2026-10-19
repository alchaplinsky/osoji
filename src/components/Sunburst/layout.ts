import type { ViewNode } from "../../lib/api";
import type { DiskExtra, HoverId } from "../../lib/types";
import { pathTo } from "../../lib/tree";
import { nodeName } from "../../screens/Breakdown/slices";
import type { Disk } from "../viz/types";

export type SliceKey = number | `other:${number}` | DiskExtra;

export const otherKey = (parentId: number): SliceKey => `other:${parentId}`;

export type Arc = {
  key: SliceKey;
  hover: HoverId;
  node: ViewNode | null;
  name: string;
  size: number;
  depth: number;
  index: number;
  a0: number;
  a1: number;
};

export const MAX_DEPTH = 3;
const MIN_SPAN = 0.006;
export const TAU = Math.PI * 2;

export const EXTRA_NAMES: Record<DiskExtra, string> = { hidden: "Not scanned", free: "Free" };

/**
 * Arcs for up to MAX_DEPTH rings below `view`, angles in [0, TAU] clockwise from 12 o'clock.
 * The full circle is `view`, or the whole `disk` when given: the scanned children then cover the used
 * share and the not-scanned and free space follow them on the inner ring.
 */
export function layoutArcs(view: ViewNode, disk?: Disk): Arc[] {
  const arcs: Arc[] = [];

  function visit(node: ViewNode, depth: number, a0: number, a1: number) {
    if (depth > MAX_DEPTH || node.size <= 0) return;
    const scale = (a1 - a0) / node.size;
    let a = a0;
    node.children.forEach((child, index) => {
      const span = child.size * scale;
      const start = a;
      a += span;
      if (span < MIN_SPAN) return;
      arcs.push({ key: child.id, hover: child.id, node: child, name: nodeName(child), size: child.size, depth, index, a0: start, a1: a });
      visit(child, depth + 1, start, a);
    });
    const span = node.other * scale;
    if (span >= MIN_SPAN) {
      const hover: HoverId = depth === 1 ? "other" : node.id;
      const name = `${node.other_count} smaller items`;
      arcs.push({ key: otherKey(node.id), hover, node: null, name, size: node.other, depth, index: node.children.length, a0: a, a1: a + span });
    }
  }

  const whole = disk?.total ?? view.size;
  if (whole <= 0) return arcs;
  let end = (view.size / whole) * TAU;
  visit(view, 1, 0, end);
  if (disk) {
    (["hidden", "free"] as const).forEach((extra) => {
      const span = (disk[extra] / whole) * TAU;
      if (span >= MIN_SPAN) arcs.push({ key: extra, hover: extra, node: null, name: EXTRA_NAMES[extra], size: disk[extra], depth: 1, index: 0, a0: end, a1: end + span });
      end += span;
    });
  }
  return arcs;
}

export function relatedKeys(view: ViewNode, hoverId: HoverId): Set<SliceKey> | null {
  if (hoverId === null) return null;
  if (hoverId === "other") return new Set([otherKey(view.id)]);
  if (hoverId === "hidden" || hoverId === "free") return new Set([hoverId]);
  const path = pathTo(view, hoverId);
  const target = path.at(-1);
  if (!target) return null;
  const keys = new Set<SliceKey>(path.map((n) => n.id));
  const addSubtree = (n: ViewNode) => {
    keys.add(n.id);
    if (n.other > 0) keys.add(otherKey(n.id));
    n.children.forEach(addSubtree);
  };
  addSubtree(target);
  return keys;
}

export const isRelated = (related: Set<SliceKey> | null, key: SliceKey) => !related || related.has(key);

const DIMMED = 0.35;

export const relatedOpacity = (related: Set<SliceKey> | null, key: SliceKey) => (isRelated(related, key) ? 1 : DIMMED);
