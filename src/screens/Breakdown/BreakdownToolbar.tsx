import { useState } from "react";
import type { NodeInfo } from "../../lib/api";
import Toolbar from "../../components/Toolbar";
import Breadcrumbs from "../../components/Breadcrumbs";
import { Icon, IconButton, Menu } from "../../components/ui";
import { fileManager, modHint } from "../../lib/platform";
import styles from "./Breakdown.module.css";

type BreakdownToolbarProps = {
  info: NodeInfo;
  onNavigate: (id: number) => void;
  onRescan: () => void;
  onReveal: () => void;
};

export default function BreakdownToolbar({ info, onNavigate, onRescan, onReveal }: BreakdownToolbarProps) {
  const [menuOpen, setMenuOpen] = useState(false);
  return (
    <Toolbar>
      <Breadcrumbs ancestors={info.ancestors} current={{ id: info.id, name: info.name }} onNavigate={onNavigate} />
      <div className={styles.menuAnchor}>
        <IconButton title="More" onClick={() => setMenuOpen((o) => !o)}>
          <Icon name="more" />
        </IconButton>
        <Menu
          open={menuOpen}
          onClose={() => setMenuOpen(false)}
          items={[
            { label: "Scan again", hint: modHint("R"), onSelect: onRescan },
            { label: `Reveal in ${fileManager()}`, hint: modHint("Enter"), onSelect: onReveal },
          ]}
        />
      </div>
    </Toolbar>
  );
}
