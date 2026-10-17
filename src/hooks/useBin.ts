import { useCallback, useMemo, useState } from "react";
import { withItem } from "../lib/bin";
import type { BinItem } from "../lib/types";

export type Bin = {
  items: BinItem[];
  ids: ReadonlySet<number>;
  total: number;
  has: (id: number) => boolean;
  add: (item: BinItem) => void;
  remove: (id: number) => void;
  toggle: (item: BinItem) => void;
  clear: () => void;
};

// A queued ancestor covers its descendants, so reviews and totals never count the same bytes twice.
// Protected items never enter the bin.
export function useBin(): Bin {
  const [queued, setItems] = useState<BinItem[]>([]);

  const ids = useMemo<ReadonlySet<number>>(() => new Set(queued.map((b) => b.id)), [queued]);
  const items = useMemo(() => queued.filter((b) => !b.ancestors.some((a) => ids.has(a))), [queued, ids]);
  const total = useMemo(() => items.reduce((sum, b) => sum + b.size, 0), [items]);
  const has = useCallback((id: number) => ids.has(id), [ids]);

  const add = useCallback((item: BinItem) => setItems((prev) => withItem(prev, item)), []);
  const remove = useCallback((id: number) => setItems((prev) => prev.filter((b) => b.id !== id)), []);
  const toggle = useCallback(
    (item: BinItem) =>
      setItems((prev) => (prev.some((b) => b.id === item.id) ? prev.filter((b) => b.id !== item.id) : withItem(prev, item))),
    [],
  );
  const clear = useCallback(() => setItems([]), []);

  return { items, ids, total, has, add, remove, toggle, clear };
}
