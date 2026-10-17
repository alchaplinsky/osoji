import { useMemo, type ComponentType } from "react";
import type { ThemeName } from "../../design/theme";
import type { NodeInfo, ViewNode } from "../../lib/api";
import type { BinItem, HoverId, Viz, Volume } from "../../lib/types";
import Sunburst from "../../components/Sunburst";
import { Orbit, Shelf, Treemap, type VizProps } from "../../components/viz";
import BinBar from "./BinBar";
import BreakdownToolbar from "./BreakdownToolbar";
import CapacityBar from "./CapacityBar";
import HoverLabel from "./HoverLabel";
import ListPanel from "./ListPanel";
import VizSwitcher from "./VizSwitcher";
import { capacityHover, hoverTarget } from "./hover";
import { visibleSlices } from "./slices";
import styles from "./Breakdown.module.css";

type BreakdownScreenProps = {
  volume: Volume;
  rootView: ViewNode;
  view: ViewNode;
  info: NodeInfo;
  theme: ThemeName;
  hoverId: HoverId;
  onHover: (id: HoverId) => void;
  onNavigate: (id: number) => void;
  onUp: () => void;
  listOpen: boolean;
  onToggleList: () => void;
  viz: Viz;
  onViz: (v: Viz) => void;
  onRescan: () => void;
  onReveal: () => void;
  binItems: BinItem[];
  binIds?: ReadonlySet<number>;
  onToggleBin: (id: number) => void;
  onRemoveFromBin: (id: number) => void;
  onReview: () => void;
};

const HINTS: Record<Viz, string> = {
  sunburst: "Click a ring to look inside it. Click the centre to go back up.",
  treemap: "Click a tile to look inside, or drag it to the bin.",
  shelf: "Hover a segment to see what is inside it. Click to go in.",
  orbit: "Bigger moons take more space. Click one to orbit it instead.",
};

const VIEWS: Record<Viz, ComponentType<VizProps>> = { sunburst: Sunburst, treemap: Treemap, shelf: Shelf, orbit: Orbit };
/** Per-chart sizing inside the stage; see the matching rules in Breakdown.module.css. */
const STAGE_CLASS: Record<Viz, string> = { sunburst: styles.radial, orbit: styles.radial, treemap: styles.treemap, shelf: styles.shelf };

export default function BreakdownScreen({
  volume,
  rootView,
  view,
  info,
  theme,
  hoverId,
  onHover,
  onNavigate,
  onUp,
  listOpen,
  onToggleList,
  viz,
  onViz,
  onRescan,
  onReveal,
  binItems,
  binIds: tickedIds,
  onToggleBin,
  onRemoveFromBin,
  onReview,
}: BreakdownScreenProps) {
  const ownIds = useMemo(() => new Set(binItems.map((item) => item.id)), [binItems]);
  const binIds = tickedIds ?? ownIds;
  const pending = binItems.reduce((sum, item) => sum + item.size, 0);
  const View = VIEWS[viz];
  const rest = viz === "sunburst" ? undefined : visibleSlices(view, theme).find((s) => !s.node);
  const vizProps: VizProps = {
    view,
    theme,
    hoverId,
    onHover,
    onDrill: onNavigate,
    onUp,
    onSelectFile: onToggleBin,
    centerSize: listOpen ? 20 : 24,
    className: STAGE_CLASS[viz],
  };

  return (
    <section className={styles.screen}>
      <BreakdownToolbar
        info={info}
        onNavigate={onNavigate}
        listOpen={listOpen}
        onToggleList={onToggleList}
        onRescan={onRescan}
        onReveal={onReveal}
      />
      <CapacityBar
        volume={volume}
        root={rootView}
        hoverId={capacityHover(rootView, info, hoverId)}
        onHover={onHover}
        onOpen={onNavigate}
        pending={pending}
      />
      <div className={styles.body}>
        <div className={styles.viz}>
          <HoverLabel target={hoverTarget(view, rootView, theme, hoverId, rest)} hint={HINTS[viz]} />
          <VizSwitcher viz={viz} onViz={onViz} className={styles.vizSwitcher} />
          <div className={styles.stage}>
            <View {...vizProps} />
          </div>
          <BinBar
            items={binItems}
            onRemove={onRemoveFromBin}
            onDrop={(id) => {
              if (!binIds.has(id)) onToggleBin(id);
            }}
            onReview={onReview}
            wide={!listOpen}
          />
        </div>
        {listOpen && (
          <ListPanel view={view} theme={theme} hoverId={hoverId} onHover={onHover} onOpen={onNavigate} binIds={binIds} onToggleBin={onToggleBin} />
        )}
      </div>
    </section>
  );
}
