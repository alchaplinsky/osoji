import type { ViewNode } from "../../lib/api";
import type { HoverId } from "../../lib/types";
import { pathTo } from "../../lib/tree";

export type SliceKey = number | `other:${number}`;

export const otherKey = (parentId: number): SliceKey => `other:${parentId}`;

export type Arc = {
  key: SliceKey;
  hover: HoverId;
  node: ViewNode | null;
  parent: ViewNode;
  depth: number;
  index: number;
  a0: number;
  a1: number;
};

export const MAX_DEPTH = 3;
const MIN_SPAN = 0.006;
const TAU = Math.PI * 2;

export function layoutArcs(view: ViewNode): Arc[] {
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
      arcs.push({ key: child.id, hover: child.id, node: child, parent: node, depth, index, a0: start, a1: a });
      visit(child, depth + 1, start, a);
    });
    const span = node.other * scale;
    if (span >= MIN_SPAN) {
      const hover: HoverId = depth === 1 ? "other" : node.id;
      arcs.push({ key: otherKey(node.id), hover, node: null, parent: node, depth, index: node.children.length, a0: a, a1: a + span });
    }
  }

  visit(view, 1, 0, TAU);
  return arcs;
}

// Keep the hovered node's ancestry and subtree bright so nested views share one hover model.
export function relatedKeys(view: ViewNode, hoverId: HoverId): Set<SliceKey> | null {
  if (hoverId === null) return null;
  if (hoverId === "other") return new Set([otherKey(view.id)]);
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
