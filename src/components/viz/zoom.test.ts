import { describe, expect, test } from "vitest";
import type { ViewNode } from "../../lib/api";
import { TAU, layoutArcs } from "../Sunburst/layout";
import { frame, zoomWorld } from "../Sunburst/zoom";
import { cameraBox, nestBox } from "./zoom";

function node(id: number, size: number, children: ViewNode[] = []): ViewNode {
  return { id, name: `node-${id}`, size, items: children.length, dir: children.length > 0, error: false, children, other: 0, other_count: 0 };
}

const round = (n: number) => Math.round(n * 1e6) / 1e6;
const spans = (shown: ReturnType<typeof frame>) => shown.map(({ arc }) => [arc.key, round(arc.depth), round((arc.a1 - arc.a0) / TAU)]);

describe("zoom", () => {
  const inner = node(2, 50, [node(3, 30), node(4, 20)]);
  const outer = node(1, 100, [inner, node(5, 50)]);
  const world = zoomWorld(layoutArcs(outer), layoutArcs(inner), inner.id);

  test("starts on the outer view and ends on the inner one", () => {
    if (!world) throw new Error("no world");
    const byKey = (a: unknown[], b: unknown[]) => Number(a[0]) - Number(b[0]);
    expect(spans(frame(world, 0)).sort(byKey)).toEqual(spans(layoutArcs(outer).map((arc) => ({ arc, fade: 1 }))).sort(byKey));
    expect(spans(frame(world, 1))).toEqual([
      [3, 1, 0.6],
      [4, 1, 0.4],
    ]);
  });

  test("halfway, the pivot's ring is fading into the hole", () => {
    if (!world) throw new Error("no world");
    const pivot = frame(world, 0.5).find(({ arc }) => arc.key === inner.id);
    expect(pivot?.fade).toBeCloseTo(0.5, 9);
  });

  test("boxes nest into a focus and the camera frames it at the end", () => {
    const whole = { x: 0, y: 0, w: 100, h: 50 };
    const focus = { x: 50, y: 0, w: 50, h: 25 };
    const nested = nestBox(whole, whole, focus);

    expect(nested).toEqual(focus);
    expect(cameraBox(nested, whole, focus, 1)).toEqual(whole);
    expect(cameraBox(focus, whole, focus, 0)).toEqual(focus);
  });
});
