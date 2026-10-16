import { useMemo } from "react";
import type { ThemeName } from "../../design/theme";
import type { NodeInfo, ViewNode } from "../../lib/api";
import type { BinItem, HoverId, Volume } from "../../lib/types";
import Sunburst from "../../components/Sunburst";
import BinBar from "./BinBar";
import BreakdownToolbar from "./BreakdownToolbar";
import CapacityBar from "./CapacityBar";
import HoverLabel from "./HoverLabel";
import ListPanel from "./ListPanel";
import { capacityHover, hoverTarget } from "./hover";
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
  onRescan: () => void;
  onReveal: () => void;
  binItems: BinItem[];
  /** Ids to show as ticked; defaults to the ids of `binItems`. Lets ticked children of a queued folder stay checked. */
  binIds?: ReadonlySet<number>;
  onToggleBin: (id: number) => void;
  onRemoveFromBin: (id: number) => void;
  onReview: () => void;
};

const HINT = "Click a ring to look inside it. Click the centre to go back up.";

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
          <HoverLabel target={hoverTarget(view, rootView, theme, hoverId)} hint={HINT} />
          <div className={styles.stage}>
            <Sunburst
              view={view}
              theme={theme}
              hoverId={hoverId}
              onHover={onHover}
              onDrill={onNavigate}
              onUp={onUp}
              onSelectFile={onToggleBin}
              centerSize={listOpen ? 20 : 24}
            />
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
