import { ShieldAlert } from "lucide-react";
import { Banner, Button } from "../ui";
import { humanCount } from "../../lib/format";
import styles from "./FdaBanner.module.css";

type FdaBannerProps = {
  unreadable: number;
  onOpenSettings: () => void;
  onDismiss: () => void;
};

/** Asks for Full Disk Access after a scan that had to skip protected folders. */
export default function FdaBanner({ unreadable, onOpenSettings, onDismiss }: FdaBannerProps) {
  const folders = unreadable === 1 ? "1 folder" : `${humanCount(unreadable)} folders`;
  return (
    <Banner
      className={styles.banner}
      icon={<ShieldAlert size={16} />}
      actions={
        <Button size="sm" onClick={onOpenSettings}>
          Open System Settings
        </Button>
      }
      onDismiss={onDismiss}
    >
      {folders} could not be read. Grant Unshift Full Disk Access to include Mail, Messages, Safari and other protected folders. You may
      need to relaunch Unshift afterwards.
    </Banner>
  );
}
