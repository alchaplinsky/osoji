import { describe, expect, test } from "vitest";
import { TAU, layoutArcs, relatedKeys } from "../Sunburst/layout";
import { diskOf } from "../../screens/Breakdown/CapacityBar/capacity";
import type { ViewNode } from "../../lib/api";
import { neutral, tone } from "../../design/palette";
import { slicesOf, visibleSlices } from "../../screens/Breakdown/slices";
import { squarify } from "./Treemap/squarify";

function node(id: number, size: number, children: ViewNode[] = [], other = 0, otherCount = 0): ViewNode {
  return { id, name: `node-${id}`, size, items: children.length, dir: children.length > 0, error: false, children, other, other_count: otherCount };
}

/** An angle as a share of the full circle, rounded off float noise. */
const share = (angle: number) => Math.round((angle / TAU) * 1e6) / 1e6;

describe("visualization layout", () => {
  test("merges filtered children and the backend tail once", () => {
    const view = node(0, 100, [node(1, 50), node(2, 20), node(3, 1)], 29, 3);
    const slices = visibleSlices(view, "instrument", 2, 0.02);

    expect(slices.map((slice) => slice.size)).toEqual([50, 20, 30]);
    expect(slices.at(-1)?.name).toBe("4 smaller items");
  });

  test("names the Trash folder and draws it neutral", () => {
    const trash = { ...node(1, 40), name: ".Trash", guard: { kind: "trash" as const, reason: "Already in the Trash." } };
    const view = node(0, 100, [trash, node(2, 60)]);

    for (const slices of [visibleSlices(view, "instrument"), slicesOf(view, "instrument")]) {
      expect(slices.map((slice) => slice.name)).toEqual(["Trash", "node-2"]);
      expect(slices[0].fill).toBe(neutral("instrument"));
      expect(slices[1].fill).toBe(tone("instrument", 1));
    }
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

  test("a folder view fills the whole circle", () => {
    const view = node(0, 100, [node(1, 60), node(2, 40)]);
    const arcs = layoutArcs(view);

    expect(arcs.map((a) => [share(a.a0), share(a.a1)])).toEqual([
      [0, 0.6],
      [0.6, 1],
    ]);
  });

  test("a disk view spans the disk, with not-scanned and free space after the scanned items", () => {
    const root = { ...node(0, 300, [node(1, 200), node(2, 100)]), name: "/" };
    const disk = diskOf({ name: "Macintosh HD", mount: "/", total: 1000, free: 600 }, root, root);
    const arcs = layoutArcs(root, disk);

    expect(disk).toEqual({ name: "Macintosh HD", total: 1000, hidden: 100, free: 600 });
    expect(arcs.map((a) => [a.key, share(a.a1 - a.a0)])).toEqual([
      [1, 0.2],
      [2, 0.1],
      ["hidden", 0.1],
      ["free", 0.6],
    ]);
    expect(share(arcs.at(-1)?.a1 ?? 0)).toBe(1);
  });

  test("only the volume root of a disk scan spans the disk", () => {
    const child = node(1, 200);
    const root = { ...node(0, 300, [child]), name: "/" };
    const volume = { name: "Macintosh HD", mount: "/", total: 1000, free: 600 };

    expect(diskOf(volume, root, child)).toBeUndefined();
    expect(diskOf(volume, { ...root, name: "/Users/me" }, root)).toBeUndefined();
  });

  test("hovering free space relates only to itself", () => {
    expect(relatedKeys(node(0, 100, [node(1, 100)]), "free")).toEqual(new Set(["free"]));
  });
});
