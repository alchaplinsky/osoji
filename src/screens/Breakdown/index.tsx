import type { ThemeName } from "../../design/theme";
import type { NodeInfo, ViewNode } from "../../lib/api";
import type { HoverId, Volume } from "../../lib/types";
import Sunburst from "../../components/Sunburst";
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
}: BreakdownScreenProps) {
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
      <CapacityBar volume={volume} root={rootView} hoverId={capacityHover(rootView, info, hoverId)} onHover={onHover} onOpen={onNavigate} />
      <div className={styles.body}>
        <div className={styles.viz}>
          <HoverLabel target={hoverTarget(view, rootView, theme, hoverId)} hint={HINT} />
          {/* The bottom padding leaves room for the bin bar (PR 3). */}
          <div className={styles.stage}>
            <Sunburst
              view={view}
              theme={theme}
              hoverId={hoverId}
              onHover={onHover}
              onDrill={onNavigate}
              onUp={onUp}
              centerSize={listOpen ? 20 : 24}
            />
          </div>
        </div>
        {listOpen && <ListPanel view={view} theme={theme} hoverId={hoverId} onHover={onHover} onOpen={onNavigate} />}
      </div>
    </section>
  );
}
