import type { ViewNode } from "./api";

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
