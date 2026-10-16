import type { DragEvent } from "react";
import type { ThemeName } from "../../design/theme";
import type { ViewNode } from "../../lib/api";
import type { HoverId } from "../../lib/types";

export type VizProps = {
  view: ViewNode;
  theme: ThemeName;
  hoverId: HoverId;
  onHover: (id: HoverId) => void;
  onDrill: (id: number) => void;
  onUp: () => void;
  /** Clicking a file; directories drill instead. */
  onSelectFile: (id: number) => void;
  /** Defaults to `startBinDrag`. */
  onDragStart?: (e: DragEvent, id: number) => void;
  /** Font size of the centre size label in px, where the view has one. */
  centerSize?: number;
  className?: string;
};
