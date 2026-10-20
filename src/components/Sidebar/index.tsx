import type { TrashInfo } from "../../lib/api";
import { isMac } from "../../lib/platform";
import type { Screen, Volume } from "../../lib/types";
import DiskCard, { type RootChoice } from "./DiskCard";
import NavItem from "./NavItem";
import NavSection from "./NavSection";
import TrashLine from "./TrashLine";
import styles from "./Sidebar.module.css";

export type { RootChoice };

type SidebarProps = {
  volume: Volume | null;
  volumes: Volume[];
  scanRoot: string;
  home?: string;
  rootPickerDisabled?: boolean;
  onPickRoot: (choice: RootChoice) => void;
  trash?: TrashInfo | null;
  scanned: boolean;
  screen: Screen;
  onNavigate: (s: Screen) => void;
  suggestBadge?: string;
  watchers?: { badge?: string } | null;
};

export default function Sidebar({ volume, volumes, scanRoot, home, rootPickerDisabled, onPickRoot, trash, scanned, screen, onNavigate, suggestBadge, watchers }: SidebarProps) {
  const item = (target: Screen) => {
    const locked = target !== "scan" && target !== "settings" && !scanned;
    return {
      active: screen === target,
      disabled: locked,
      onClick: () => onNavigate(locked ? "scan" : target),
    };
  };

  return (
    <aside className={styles.sidebar}>
      {/* Only macOS draws the traffic lights over the content; elsewhere the OS title bar sits above it. */}
      {isMac() && <div className={styles.top} data-tauri-drag-region="deep" />}

      <DiskCard volume={volume} volumes={volumes} scanRoot={scanRoot} home={home} disabled={rootPickerDisabled} onPickRoot={onPickRoot} />
      {trash && trash.size > 0 && <TrashLine trash={trash} />}

      <NavSection>
        <NavItem icon="scan" label={scanned ? "Scan again" : "Scan"} {...item("scan")} />
      </NavSection>
      <NavSection title="Clean up">
        <NavItem icon="breakdown" label="Breakdown" {...item("breakdown")} />
        <NavItem icon="largest" label="Largest files" {...item("largest")} />
        <NavItem icon="suggest" label="Suggestions" badge={scanned ? suggestBadge : undefined} {...item("suggest")} />
      </NavSection>
      {watchers && (
        <NavSection title="Keep an eye">
          <NavItem icon="watch" label="Watchers" badge={scanned ? watchers.badge : undefined} {...item("watchers")} />
        </NavSection>
      )}

      <div className={styles.spacer} />
      <NavItem icon="settings" label="Settings" {...item("settings")} />
    </aside>
  );
}
