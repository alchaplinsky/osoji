import type { ThemeName } from "../../design/theme";
import { neutral, tone } from "../../design/palette";
import type { ViewNode } from "../../lib/api";
import { isTrash } from "../../lib/bin";
import type { Guard, HoverId } from "../../lib/types";
import { otherKey, type SliceKey } from "../../components/Sunburst/layout";

export type Slice = {
  node: ViewNode | null;
  key: SliceKey;
  hover: HoverId;
  name: string;
  size: number;
  fill: string;
  guard: Guard | null;
  /** The folder could not be read, so its size is incomplete. */
  error: boolean;
};

/** A node's chart label: the Trash folder reads as "Trash" rather than ".Trash". */
export const nodeName = (n: ViewNode) => (isTrash(n) ? "Trash" : n.name);

/** A node's chart colour: its size rank among `count` siblings on the ramp, or neutral for the Trash, which is not the user's to sort. */
export const nodeFill = (n: ViewNode, theme: ThemeName, rank: number, count: number, depth = 1) =>
  isTrash(n) ? neutral(theme) : tone(theme, rank, count, depth);

const sliceOf = (n: ViewNode, theme: ThemeName, rank: number, count: number): Slice => ({
  node: n,
  key: n.id,
  hover: n.id,
  name: nodeName(n),
  size: n.size,
  fill: nodeFill(n, theme, rank, count),
  guard: n.guard ?? null,
  error: n.error,
});

export function slicesOf(view: ViewNode, theme: ThemeName): Slice[] {
  const slices: Slice[] = view.children.map((n, i) => sliceOf(n, theme, i, view.children.length));
  if (view.other > 0) {
    slices.push({ node: null, key: otherKey(view.id), hover: "other", name: `${view.other_count} smaller items`, size: view.other, fill: neutral(theme), guard: null, error: false });
  }
  return slices;
}

/**
 * The slices a chart draws: the first `max` children holding at least `minShare` of `view`, then
 * everything else (smaller children and the merged tail) as one neutral "smaller items" slice.
 */
export function visibleSlices(view: ViewNode, theme: ThemeName, max = 14, minShare = 0.006): Slice[] {
  const shown = view.children.filter((n, i) => i < max && view.size > 0 && n.size / view.size >= minShare);
  const slices: Slice[] = shown.map((n, i) => sliceOf(n, theme, i, shown.length));
  let restSize = view.other;
  let restCount = view.other_count;
  view.children.forEach((n) => {
    if (shown.includes(n)) return;
    restSize += n.size;
    restCount += 1;
  });
  if (restSize > 0) {
    slices.push({ node: null, key: otherKey(view.id), hover: "other", name: `${restCount} smaller items`, size: restSize, fill: neutral(theme), guard: null, error: false });
  }
  return slices;
}
