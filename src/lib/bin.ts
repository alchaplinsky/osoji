import type { BinItem, FileEntry, Guard } from "./types";

export const toBinItem = ({ id, name, path, size, dir, ancestors, guard }: FileEntry): BinItem => ({ id, name, path, size, dir, ancestors, guard });

export const isTrash = (item: { guard?: Guard | null }) => item.guard?.kind === "trash";

export const isProtected = (item: { guard?: Guard | null }) => item.guard?.kind === "protected" || isTrash(item);

export const isRisky = (item: { guard?: Guard | null }) => item.guard?.kind === "risky";

const itemsAre = (n: number) => (n === 1 ? "1 item is" : `${n} items are`);

/** The review sheet's caution line, or undefined when nothing needs a second click. */
export function reviewWarning(items: BinItem[], bigCount: number): string | undefined {
  const risky = items.filter(isRisky);
  const notes: string[] = [];
  if (risky.length > 0) notes.push(`${itemsAre(risky.length)} marked risky: ${risky[0].guard?.reason.replace(/\.$/, "")}.`);
  if (bigCount > 0) notes.push(`${itemsAre(bigCount)} over 1 GB.`);
  return notes.length > 0 ? `${notes.join(" ")} Double-check before you continue.` : undefined;
}

/** Queue `item` unless it is protected or already queued; a queued ancestor replaces its queued descendants. */
export function withItem(items: BinItem[], item: BinItem): BinItem[] {
  if (isProtected(item) || items.some((b) => b.id === item.id)) return items;
  return items.filter((b) => !b.ancestors.includes(item.id)).concat(item);
}
