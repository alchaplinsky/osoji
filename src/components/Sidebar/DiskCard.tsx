import { useState } from "react";
import type { Volume } from "../../lib/types";
import { humanBytes } from "../../lib/format";
import { Icon, Menu, Mono, ProgressBar } from "../ui";
import styles from "./DiskCard.module.css";

export type RootChoice = "disk" | "home" | "folder";

type DiskCardProps = {
  volume: Volume | null;
  scanRoot: string;
  disabled?: boolean;
  onPickRoot: (choice: RootChoice) => void;
};

export default function DiskCard({ volume, scanRoot, disabled, onPickRoot }: DiskCardProps) {
  const [open, setOpen] = useState(false);
  const usedPct = volume && volume.total > 0 ? ((volume.total - volume.free) / volume.total) * 100 : 0;
  const showRoot = volume !== null && scanRoot !== volume.mount;

  return (
    <div className={styles.wrap}>
      <button
        type="button"
        className={styles.card}
        title={scanRoot}
        aria-haspopup="menu"
        aria-expanded={open}
        disabled={disabled}
        onClick={() => setOpen((o) => !o)}
      >
        <span className={styles.head}>
          <span className={styles.name}>{volume?.name ?? "—"}</span>
          <Icon name="chevronDown" className={styles.chevron} />
        </span>
        {showRoot && <Mono className={styles.root}>{scanRoot}</Mono>}
        <ProgressBar value={usedPct} height={3} className={styles.bar} />
        <Mono size={11} className={styles.meta}>
          {volume ? `${humanBytes(volume.free)} free · ${humanBytes(volume.total)}` : "— free · —"}
        </Mono>
      </button>
      <Menu
        open={open}
        onClose={() => setOpen(false)}
        className={styles.menu}
        items={[
          { label: "Macintosh HD", onSelect: () => onPickRoot("disk") },
          { label: "Home folder", onSelect: () => onPickRoot("home") },
          { label: "Choose folder…", onSelect: () => onPickRoot("folder") },
        ]}
      />
    </div>
  );
}
