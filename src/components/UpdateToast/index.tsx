import type { AutoUpdate } from "../../hooks/useAutoUpdate";
import UpdateDownloadToast from "./UpdateDownloadToast";
import UpdateReadyToast from "./UpdateReadyToast";
import UpdateStatusToast from "./UpdateStatusToast";

type Props = Pick<AutoUpdate, "ready" | "download" | "status" | "dismiss">;

/**
 * Bottom-right update toast. Prefers the sticky "update ready → restart" prompt
 * when one is staged; then the download progress of an update a manual
 * "Check for Updates…" found; otherwise that run's transient feedback
 * (checking / up to date / failed). Renders nothing when there's none of these.
 */
export default function UpdateToast({ ready, download, status, dismiss }: Props) {
  if (ready) return <UpdateReadyToast version={ready.version} notes={ready.notes} onDismiss={dismiss} />;
  if (download) return <UpdateDownloadToast progress={download} />;
  if (status) return <UpdateStatusToast status={status} />;
  return null;
}
