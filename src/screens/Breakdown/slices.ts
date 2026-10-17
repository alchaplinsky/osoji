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

/** A node's chart colour: sibling `i`'s ramp stop, or neutral for the Trash, which is not the user's to sort. */
export const nodeFill = (n: ViewNode, theme: ThemeName, i: number, depth = 1) => (isTrash(n) ? neutral(theme) : tone(theme, i, depth));

const sliceOf = (n: ViewNode, theme: ThemeName, i: number): Slice => ({
  node: n,
  key: n.id,
  hover: n.id,
  name: nodeName(n),
  size: n.size,
  fill: nodeFill(n, theme, i),
  guard: n.guard ?? null,
  error: n.error,
});

export function slicesOf(view: ViewNode, theme: ThemeName): Slice[] {
  const slices: Slice[] = view.children.map((n, i) => sliceOf(n, theme, i));
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
  const slices: Slice[] = [];
  let restSize = view.other;
  let restCount = view.other_count;
  view.children.forEach((n, i) => {
    if (i < max && view.size > 0 && n.size / view.size >= minShare) {
      slices.push(sliceOf(n, theme, i));
    } else {
      restSize += n.size;
      restCount += 1;
    }
  });
  if (restSize > 0) {
    slices.push({ node: null, key: otherKey(view.id), hover: "other", name: `${restCount} smaller items`, size: restSize, fill: neutral(theme), guard: null, error: false });
  }
  return slices;
}
