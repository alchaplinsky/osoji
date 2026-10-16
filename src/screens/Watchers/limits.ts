import { humanBytes } from "../../lib/format";

const GB = 2 ** 30;

export const LIMIT_OPTIONS = [5, 10, 20, 50, 100].map((gb, i, all) => ({
  value: String(gb * GB),
  label: i === all.length - 1 ? `${gb} GB` : String(gb),
}));

export function limitLabel(bytes: number): string {
  return Number.isInteger(bytes / GB) ? `${bytes / GB} GB` : humanBytes(bytes);
}
