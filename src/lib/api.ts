import { Channel, invoke } from "@tauri-apps/api/core";
import type { FileEntry, Suggestion } from "./types";

export type ProgressMsg = { files: number; dirs: number; bytes: number; errors: number; elapsed_ms: number };
export type ScanSummary = { root: string; entries: number; bytes: number; errors: number; elapsed_ms: number; cancelled: boolean };
export type ViewNode = {
  id: number; name: string; size: number; items: number; dir: boolean; error: boolean;
  children: ViewNode[]; other: number; other_count: number;
};
export type Crumb = { id: number; name: string };
export type NodeInfo = {
  id: number; name: string; path: string; size: number; items: number; dir: boolean;
  error: boolean; trashed: boolean; mtime: number; ancestors: Crumb[];
};
export type TrashResult = { freed: number; count: number };
export type VolumeInfo = { name: string; mount: string; total: number; free: number };
export type WatchHit = { path: string; id: number | null; size: number; dir: boolean };

export function startScan(root: string, onProgress: (p: ProgressMsg) => void): Promise<ScanSummary> {
  const channel = new Channel<ProgressMsg>();
  channel.onmessage = onProgress;
  return invoke<ScanSummary>("start_scan", { root, onProgress: channel });
}

export const cancelScan = () => invoke<void>("cancel_scan");

export const getView = (node: number, depth = 5, topK = 48, minFraction = 0.002) =>
  invoke<ViewNode>("get_view", { node, depth, topK, minFraction });

export const nodeInfo = (node: number) => invoke<NodeInfo>("node_info", { node });
export const largestFiles = (limit = 100) => invoke<FileEntry[]>("largest_files", { limit });
export const suggestions = () => invoke<Suggestion[]>("suggestions");
export const watchStatus = (paths: string[]) => invoke<WatchHit[]>("watch_status", { paths });
export const reveal = (node: number) => invoke<void>("reveal", { node });
export const trashNodes = (nodes: number[]) => invoke<TrashResult>("trash_nodes", { nodes });
export const homeDir = () => invoke<string>("home_dir");
export const volumeInfo = (path: string) => invoke<VolumeInfo>("volume_info", { path });
