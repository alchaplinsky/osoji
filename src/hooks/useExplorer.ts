import { useCallback, useRef, useState } from "react";
import { getView, nodeInfo, type NodeInfo, type ViewNode } from "../lib/api";

const VIEW_DEPTH = 3;

export function useExplorer() {
  const [currentId, setCurrentId] = useState(0);
  const [rootView, setRootView] = useState<ViewNode | null>(null);
  const [view, setView] = useState<ViewNode | null>(null);
  const [info, setInfo] = useState<NodeInfo | null>(null);
  const generation = useRef(0);

  const load = useCallback(async (id: number) => {
    const gen = ++generation.current;
    const [v, i] = await Promise.all([getView(id, VIEW_DEPTH), nodeInfo(id)]);
    if (gen !== generation.current) return;
    setView(v);
    setInfo(i);
    setCurrentId(id);
    if (id === 0) setRootView(v);
  }, []);

  const refresh = useCallback(
    async (id: number) => {
      const gen = ++generation.current;
      const [root, v, i] = await Promise.all([getView(0, VIEW_DEPTH), getView(id, VIEW_DEPTH), nodeInfo(id)]);
      if (gen !== generation.current) return;
      setRootView(root);
      setView(v);
      setInfo(i);
      setCurrentId(id);
    },
    [],
  );

  const reset = useCallback(() => {
    generation.current++;
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
