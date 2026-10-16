import type { DragEvent } from "react";

const FORMAT = "text/plain";

/** Mark a drag as carrying node `id` towards the bin. */
export function startBinDrag(e: DragEvent, id: number) {
  e.dataTransfer.effectAllowed = "move";
  e.dataTransfer.setData(FORMAT, String(id));
}

/** The node id carried by a drop, or `null` if it is not one of ours. */
export function readBinDrop(e: DragEvent): number | null {
  const raw = e.dataTransfer.getData(FORMAT);
  if (raw === "") return null;
  const id = Number(raw);
  return Number.isInteger(id) ? id : null;
}
