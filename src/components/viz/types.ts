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
  onSelectFile: (id: number) => void;
  onDragStart?: (e: DragEvent, id: number) => void;
  centerSize?: number;
  className?: string;
};
