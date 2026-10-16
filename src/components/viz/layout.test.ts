import { describe, expect, test } from "bun:test";
import { relatedKeys } from "../Sunburst/layout";
import type { ViewNode } from "../../lib/api";
import { visibleSlices } from "../../screens/Breakdown/slices";
import { squarify } from "./Treemap/squarify";

function node(id: number, size: number, children: ViewNode[] = [], other = 0, otherCount = 0): ViewNode {
  return { id, name: `node-${id}`, size, items: children.length, dir: children.length > 0, error: false, children, other, other_count: otherCount };
}

describe("visualization layout", () => {
  test("merges filtered children and the backend tail once", () => {
    const view = node(0, 100, [node(1, 50), node(2, 20), node(3, 1)], 29, 3);
    const slices = visibleSlices(view, "instrument", 2, 0.02);

    expect(slices.map((slice) => slice.size)).toEqual([50, 20, 30]);
    expect(slices.at(-1)?.name).toBe("4 smaller items");
  });

  test("keeps the hovered ancestry and subtree related", () => {
    const leaf = node(2, 30);
    const folder = node(1, 60, [leaf], 30, 2);
    const view = node(0, 100, [folder], 40, 4);
    const related = relatedKeys(view, leaf.id);

    expect(related).toEqual(new Set([0, 1, 2]));
  });

  test("fills the requested treemap rectangle without escaping it", () => {
    const cells = squarify(
      [
        { id: 1, value: 60 },
        { id: 2, value: 30 },
        { id: 3, value: 10 },
      ],
      0,
      0,
      200,
      100,
    );

    expect(cells.reduce((area, cell) => area + cell.w * cell.h, 0)).toBeCloseTo(20_000, 6);
    expect(cells.every((cell) => cell.x >= 0 && cell.y >= 0 && cell.x + cell.w <= 200 && cell.y + cell.h <= 100)).toBe(true);
  });
});
