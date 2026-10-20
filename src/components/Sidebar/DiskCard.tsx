import { useState } from "react";
import { Folder, HardDrive, House } from "lucide-react";
import type { Volume } from "../../lib/types";
import { humanBytes } from "../../lib/format";
import { Icon, Menu, Mono, ProgressBar, type MenuItemDef } from "../ui";
import styles from "./DiskCard.module.css";

export type RootChoice = { kind: "volume"; mount: string } | { kind: "home" } | { kind: "folder" };

type DiskCardProps = {
  /** The volume holding the scan root. */
  volume: Volume | null;
  /** Every mounted volume the picker offers. */
  volumes: Volume[];
  scanRoot: string;
  home?: string;
  disabled?: boolean;
  onPickRoot: (choice: RootChoice) => void;
};

export default function DiskCard({ volume, volumes, scanRoot, home, disabled, onPickRoot }: DiskCardProps) {
  const [open, setOpen] = useState(false);
  const usedPct = volume && volume.total > 0 ? ((volume.total - volume.free) / volume.total) * 100 : 0;
  const showRoot = volume !== null && scanRoot !== volume.mount;
  const onVolume = volumes.some((v) => v.mount === scanRoot);
  const onHome = !onVolume && scanRoot === home;
  const items: MenuItemDef[] = [
    ...volumes.map((v) => ({
      label: v.name,
      icon: <HardDrive size={16} strokeWidth={1.5} />,
      hint: `${humanBytes(v.free)} free`,
      checked: v.mount === scanRoot,
      onSelect: () => onPickRoot({ kind: "volume", mount: v.mount }),
    })),
    { label: "Home folder", icon: <House size={16} strokeWidth={1.5} />, checked: onHome, onSelect: () => onPickRoot({ kind: "home" }) },
    {
      label: "Choose folder…",
      icon: <Folder size={16} strokeWidth={1.5} />,
      checked: !onVolume && !onHome,
      onSelect: () => onPickRoot({ kind: "folder" }),
    },
  ];

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
          {volume ? `${humanBytes(volume.total)} · ${humanBytes(volume.free)} free` : "— · — free"}
        </Mono>
      </button>
      <Menu
        open={open}
        onClose={() => setOpen(false)}
        className={styles.menu}
        items={items}
      />
    </div>
  );
}
