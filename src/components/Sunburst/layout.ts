import type { ViewNode } from "../../lib/api";

export type Arc = {
  node: ViewNode | null; // null for a merged "other" slice
  depth: number;         // 1 = first ring
  a0: number;
  a1: number;
  hue: number;           // -1 for "other"
  parent: ViewNode;
};

const TAU = Math.PI * 2;
const GOLDEN = 137.508;

/** Flatten a view tree into arcs. Angles are proportional to size within each parent. */
export function layoutArcs(root: ViewNode): { arcs: Arc[]; maxDepth: number } {
  const arcs: Arc[] = [];
  let maxDepth = 0;

  function visit(node: ViewNode, depth: number, a0: number, a1: number, hue: number) {
    if (node.size <= 0) return;
    const span = a1 - a0;
    let a = a0;
    node.children.forEach((child, i) => {
      const w = (child.size / node.size) * span;
      if (w <= 0) return;
      const h = depth === 0 ? (i * GOLDEN) % 360 : hue;
      const d = depth + 1;
      maxDepth = Math.max(maxDepth, d);
      arcs.push({ node: child, depth: d, a0: a, a1: a + w, hue: h, parent: node });
      visit(child, d, a, a + w, h);
      a += w;
    });
    if (node.other > 0) {
      const w = (node.other / node.size) * span;
      arcs.push({ node: null, depth: depth + 1, a0: a, a1: a + w, hue: -1, parent: node });
      maxDepth = Math.max(maxDepth, depth + 1);
    }
  }

  visit(root, 0, 0, TAU, 0);
  return { arcs, maxDepth };
}
