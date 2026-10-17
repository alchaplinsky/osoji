import type { ThemeName } from "../../design/theme";
import { neutral, tone } from "../../design/palette";
import type { ViewNode } from "../../lib/api";
import type { HoverId } from "../../lib/types";
import { otherKey, type SliceKey } from "../../components/Sunburst/layout";

export type Slice = {
  node: ViewNode | null;
  key: SliceKey;
  hover: HoverId;
  name: string;
  size: number;
  fill: string;
  /** The folder could not be read, so its size is incomplete. */
  error: boolean;
};

export function slicesOf(view: ViewNode, theme: ThemeName): Slice[] {
  const slices: Slice[] = view.children.map((n, i) => ({ node: n, key: n.id, hover: n.id, name: n.name, size: n.size, fill: tone(theme, i), error: n.error }));
  if (view.other > 0) {
    slices.push({ node: null, key: otherKey(view.id), hover: "other", name: `${view.other_count} smaller items`, size: view.other, fill: neutral(theme), error: false });
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
      slices.push({ node: n, key: n.id, hover: n.id, name: n.name, size: n.size, fill: tone(theme, i), error: n.error });
    } else {
      restSize += n.size;
      restCount += 1;
    }
  });
  if (restSize > 0) {
    slices.push({ node: null, key: otherKey(view.id), hover: "other", name: `${restCount} smaller items`, size: restSize, fill: neutral(theme), error: false });
  }
  return slices;
}
