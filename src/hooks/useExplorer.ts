import { useCallback, useState } from "react";
import { getView, nodeInfo, type NodeInfo, type ViewNode } from "../lib/api";

/** Rings drawn by the sunburst; the view is fetched to the same depth. */
const VIEW_DEPTH = 3;

/** Navigation state over a finished scan: the root view, the current view and its info. */
export function useExplorer() {
  const [currentId, setCurrentId] = useState(0);
  const [rootView, setRootView] = useState<ViewNode | null>(null);
  const [view, setView] = useState<ViewNode | null>(null);
  const [info, setInfo] = useState<NodeInfo | null>(null);

  const load = useCallback(async (id: number) => {
    const [v, i] = await Promise.all([getView(id, VIEW_DEPTH), nodeInfo(id)]);
    setView(v);
    setInfo(i);
    setCurrentId(id);
    if (id === 0) setRootView(v);
  }, []);

  /** Re-fetch the root view and move to `id`; used after the tree changed (e.g. items trashed). */
  const refresh = useCallback(
    async (id: number) => {
      const [root] = await Promise.all([getView(0, VIEW_DEPTH), load(id)]);
      setRootView(root);
    },
    [load],
  );

  const reset = useCallback(() => {
    setView(null);
    setInfo(null);
    setRootView(null);
    setCurrentId(0);
  }, []);

  const goUp = useCallback(() => {
    const parent = info?.ancestors.at(-1);
    if (parent) return load(parent.id);
    return Promise.resolve();
  }, [info, load]);

  return { currentId, rootView, view, info, load, refresh, goUp, reset };
}
