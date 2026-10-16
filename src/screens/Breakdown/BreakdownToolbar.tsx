import { useState } from "react";
import type { NodeInfo } from "../../lib/api";
import Toolbar from "../../components/Toolbar";
import Breadcrumbs from "../../components/Breadcrumbs";
import { Icon, IconButton, Menu, SegmentedControl, type SegmentOption } from "../../components/ui";
import styles from "./Breakdown.module.css";

type Viz = "sunburst" | "treemap" | "shelf" | "orbit";

const VIZ_TABS: SegmentOption<Viz>[] = [
  { value: "sunburst", label: "Sunburst" },
  { value: "treemap", label: "Treemap", disabled: true },
  { value: "shelf", label: "Shelf", disabled: true },
  { value: "orbit", label: "Orbit", disabled: true },
];

type BreakdownToolbarProps = {
  info: NodeInfo;
  onNavigate: (id: number) => void;
  listOpen: boolean;
  onToggleList: () => void;
  onRescan: () => void;
  onReveal: () => void;
};

export default function BreakdownToolbar({ info, onNavigate, listOpen, onToggleList, onRescan, onReveal }: BreakdownToolbarProps) {
  const [menuOpen, setMenuOpen] = useState(false);
  return (
    <Toolbar>
      <Breadcrumbs ancestors={info.ancestors} current={{ id: info.id, name: info.name }} onNavigate={onNavigate} />
      <SegmentedControl options={VIZ_TABS} value="sunburst" onChange={() => {}} />
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
