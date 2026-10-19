import type { DragEvent, MouseEvent } from "react";
import type { ThemeName } from "../../design/theme";
import type { ViewNode } from "../../lib/api";
import type { HoverId } from "../../lib/types";

/** The selected disk around a volume-root view: the chart's whole is `total`, of which `hidden` and `free` lie outside the scan. */
export type Disk = { name: string; total: number; hidden: number; free: number };

export type VizProps = {
  view: ViewNode;
  theme: ThemeName;
  hoverId: HoverId;
  onHover: (id: HoverId) => void;
  onDrill: (id: number) => void;
  onUp: () => void;
  /** Click on a file; `source` is the piece clicked, for animating it into the bin. */
  onSelectFile: (id: number, source?: Element) => void;
  onDragStart?: (e: DragEvent, id: number) => void;
  /** Right-click on a real node (not the "other" slice). */
  onContextMenu?: (e: MouseEvent, id: number) => void;
  /** Ids queued in the bin; their pieces are drawn as hatched slots. */
  binIds?: ReadonlySet<number>;
  /** Set when `view` is the root of a disk scan; the sunburst then spans the whole disk. */
  disk?: Disk;
  centerSize?: number;
  className?: string;
};
