import type { DragEvent } from "react";
import type { ViewNode } from "../../lib/api";
import type { HoverId } from "../../lib/types";
import { startBinDrag } from "../../screens/Breakdown/BinBar/drag";
import type { VizProps } from "./types";

export type PointerActions = Pick<VizProps, "onHover" | "onDrill" | "onSelectFile">;

export function pointerProps(node: ViewNode | null, hover: HoverId, actions: PointerActions) {
  return {
    onMouseEnter: () => actions.onHover(hover),
    onMouseLeave: () => actions.onHover(null),
    onClick: () => {
      if (node) (node.dir ? actions.onDrill : actions.onSelectFile)(node.id);
    },
  };
}

export function dragProps(node: ViewNode | null, onDragStart: VizProps["onDragStart"] = startBinDrag) {
  if (!node) return {};
  return { draggable: true, onDragStart: (e: DragEvent) => onDragStart(e, node.id) };
}
