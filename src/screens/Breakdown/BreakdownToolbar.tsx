import { useState } from "react";
import type { NodeInfo } from "../../lib/api";
import type { Viz } from "../../lib/types";
import Toolbar from "../../components/Toolbar";
import Breadcrumbs from "../../components/Breadcrumbs";
import { Icon, IconButton, Menu, SegmentedControl, type SegmentOption } from "../../components/ui";
import styles from "./Breakdown.module.css";

const VIZ_TABS: SegmentOption<Viz>[] = [
  { value: "sunburst", label: "Sunburst" },
  { value: "treemap", label: "Treemap" },
  { value: "shelf", label: "Shelf" },
  { value: "orbit", label: "Orbit" },
];

type BreakdownToolbarProps = {
  info: NodeInfo;
  onNavigate: (id: number) => void;
  viz: Viz;
  onViz: (v: Viz) => void;
  listOpen: boolean;
  onToggleList: () => void;
  onRescan: () => void;
  onReveal: () => void;
};

export default function BreakdownToolbar({ info, onNavigate, viz, onViz, listOpen, onToggleList, onRescan, onReveal }: BreakdownToolbarProps) {
  const [menuOpen, setMenuOpen] = useState(false);
  return (
    <Toolbar>
      <Breadcrumbs ancestors={info.ancestors} current={{ id: info.id, name: info.name }} onNavigate={onNavigate} />
      <SegmentedControl options={VIZ_TABS} value={viz} onChange={onViz} />
      <IconButton active={listOpen} title="Toggle list" onClick={onToggleList}>
        <Icon name="list" />
      </IconButton>
      <div className={styles.menuAnchor}>
        <IconButton title="More" onClick={() => setMenuOpen((o) => !o)}>
          <Icon name="more" />
        </IconButton>
        <Menu
          open={menuOpen}
          onClose={() => setMenuOpen(false)}
          items={[
            { label: "Scan again", hint: "⌘R", onSelect: onRescan },
            { label: "Reveal in Finder", hint: "⌘⏎", onSelect: onReveal },
          ]}
        />
      </div>
    </Toolbar>
  );
}
