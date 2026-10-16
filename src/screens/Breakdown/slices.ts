import type { ThemeName } from "../../design/theme";
import { neutral, tone } from "../../design/palette";
import type { ViewNode } from "../../lib/api";
import type { HoverId } from "../../lib/types";
import { otherKey, type SliceKey } from "../../components/Sunburst/layout";

export type Slice = {
  /** `null` for the merged "smaller items" slice. */
  node: ViewNode | null;
  key: SliceKey;
  hover: HoverId;
  name: string;
  size: number;
  fill: string;
};

/** One slice per child of `view`, plus its merged tail, coloured by position. */
export function slicesOf(view: ViewNode, theme: ThemeName): Slice[] {
  const slices: Slice[] = view.children.map((n, i) => ({ node: n, key: n.id, hover: n.id, name: n.name, size: n.size, fill: tone(theme, i) }));
  if (view.other > 0) {
    slices.push({ node: null, key: otherKey(view.id), hover: "other", name: `${view.other_count} smaller items`, size: view.other, fill: neutral(theme) });
  }
  return slices;
}
