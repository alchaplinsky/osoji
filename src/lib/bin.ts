import type { BinItem, FileEntry } from "./types";

export const toBinItem = ({ id, name, path, size, dir, ancestors }: FileEntry): BinItem => ({ id, name, path, size, dir, ancestors });
