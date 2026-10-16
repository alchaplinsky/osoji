import type { ViewNode } from "./api";

/** Nodes from `view` down to the node with `id`, both inclusive; empty when `id` is not in the tree. */
export function pathTo(view: ViewNode, id: number): ViewNode[] {
  if (view.id === id) return [view];
  for (const child of view.children) {
    const path = pathTo(child, id);
    if (path.length) return [view, ...path];
  }
  return [];
}

export function findNode(view: ViewNode, id: number): ViewNode | null {
  return pathTo(view, id).at(-1) ?? null;
}
