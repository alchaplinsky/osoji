import type { Screen, Volume } from "../../lib/types";
import { Icon } from "../ui";
import DiskCard, { type RootChoice } from "./DiskCard";
import NavItem from "./NavItem";
import NavSection from "./NavSection";
import styles from "./Sidebar.module.css";

export type { RootChoice };

type SidebarProps = {
  volume: Volume | null;
  scanRoot: string;
  onPickRoot: (choice: RootChoice) => void;
  scanned: boolean;
  screen: Screen;
  onNavigate: (s: Screen) => void;
};

const COMING_SOON: ReadonlySet<Screen> = new Set(["largest", "suggest"]);

export default function Sidebar({ volume, scanRoot, onPickRoot, scanned, screen, onNavigate }: SidebarProps) {
  const item = (target: Screen) => {
    const soon = COMING_SOON.has(target);
    const locked = target !== "scan" && target !== "settings" && !scanned;
    return {
      active: screen === target,
      disabled: locked || soon,
      title: soon ? "Coming soon" : undefined,
      onClick: () => {
        if (locked) onNavigate("scan");
        else if (!soon) onNavigate(target);
      },
    };
  };

  return (
    <aside className={styles.sidebar}>
      <div className={styles.top} data-tauri-drag-region>
        <div className={styles.brand}>
          <span className={styles.mark}>
            <Icon name="logo" strokeWidth={1.6} />
          </span>
          <span className={styles.wordmark}>Unshift</span>
        </div>
      </div>

      <DiskCard volume={volume} scanRoot={scanRoot} onPickRoot={onPickRoot} />

      <NavSection>
        <NavItem icon="scan" label={scanned ? "Scan again" : "Scan"} {...item("scan")} />
      </NavSection>
      <NavSection title="Clean up">
        <NavItem icon="breakdown" label="Breakdown" {...item("breakdown")} />
        <NavItem icon="largest" label="Largest files" {...item("largest")} />
        <NavItem icon="suggest" label="Suggestions" {...item("suggest")} />
      </NavSection>

      <div className={styles.spacer} />
      <NavItem icon="settings" label="Settings" {...item("settings")} />
    </aside>
  );
}
