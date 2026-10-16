import type { DragEvent } from "react";

const FORMAT = "text/plain";

export function startBinDrag(e: DragEvent, id: number) {
  e.dataTransfer.effectAllowed = "move";
  e.dataTransfer.setData(FORMAT, String(id));
}

export function readBinDrop(e: DragEvent): number | null {
  const raw = e.dataTransfer.getData(FORMAT);
  if (raw === "") return null;
  const id = Number(raw);
  return Number.isInteger(id) ? id : null;
}
