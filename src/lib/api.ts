import { Channel, invoke } from "@tauri-apps/api/core";
import type { FileEntry, Guard, Suggestion } from "./types";

/** Bytes found so far under one of the scan root's direct children. */
export type TopProgress = { name: string; bytes: number };
export type ProgressMsg = { files: number; dirs: number; bytes: number; errors: number; elapsed_ms: number; tops: TopProgress[]; rest: number };
export type ScanSummary = { root: string; entries: number; bytes: number; errors: number; elapsed_ms: number; cancelled: boolean };
export type ViewNode = {
  id: number; name: string; size: number; items: number; dir: boolean; error: boolean;
  children: ViewNode[]; other: number; other_count: number; guard?: Guard | null;
};
export type Crumb = { id: number; name: string };
export type NodeInfo = {
  id: number; name: string; path: string; size: number; items: number; dir: boolean;
  error: boolean; trashed: boolean; mtime: number; ancestors: Crumb[]; guard?: Guard | null;
};
export type TrashResult = { bytes: number; count: number; trashed: number[]; failed: string[] };
/** `current` is the name of the item being moved; `bytes` counts what has been moved so far. */
export type TrashProgress = { done: number; total: number; bytes: number; current: string };
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
export function trashNodes(nodes: number[], onProgress: (p: TrashProgress) => void): Promise<TrashResult> {
  const channel = new Channel<TrashProgress>();
  channel.onmessage = onProgress;
  return invoke<TrashResult>("trash_nodes", { nodes, onProgress: channel });
}
export const homeDir = () => invoke<string>("home_dir");
export const volumeInfo = (path: string) => invoke<VolumeInfo>("volume_info", { path });
export const fullDiskAccess = () => invoke<boolean>("full_disk_access");
export const openPrivacySettings = () => invoke<void>("open_privacy_settings");
