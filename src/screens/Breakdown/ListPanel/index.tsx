import type { MouseEvent } from "react";
import type { ThemeName } from "../../../design/theme";
import type { ViewNode } from "../../../lib/api";
import type { HoverId } from "../../../lib/types";
import { humanBytes } from "../../../lib/format";
import { isProtected } from "../../../lib/bin";
import { Mono, Panel } from "../../../components/ui";
import { startBinDrag } from "../BinBar";
import { slicesOf } from "../slices";
import ListRow from "./ListRow";
import styles from "./ListPanel.module.css";

type ListPanelProps = {
  view: ViewNode;
  theme: ThemeName;
  hoverId: HoverId;
  onHover: (id: HoverId) => void;
  onOpen: (id: number) => void;
  binIds: ReadonlySet<number>;
  onToggleBin: (id: number) => void;
  onReveal?: (id: number) => void;
  onItemMenu?: (e: MouseEvent, id: number) => void;
};

export default function ListPanel({ view, theme, hoverId, onHover, onOpen, binIds, onToggleBin, onReveal, onItemMenu }: ListPanelProps) {
  const slices = slicesOf(view, theme);
  const largest = slices[0]?.size || 1;
  const count = view.children.length + view.other_count;

  return (
    <Panel className={styles.panel}>
      <div className={styles.header}>
        <div className={styles.heading}>{view.name}</div>
        <Mono className={styles.meta}>
          {count} items · {humanBytes(view.size)}
        </Mono>
      </div>
      <div className={styles.list}>
        {slices.map(({ node, key, hover, name, size, fill, guard, error }) => (
          <ListRow
            key={key}
            name={name}
            sizeLabel={humanBytes(size)}
            color={fill}
            pct={(size / largest) * 100}
            dir={!!node?.dir}
            locked={error}
            hovered={hoverId === hover}
            inBin={!!node && binIds.has(node.id)}
            guard={guard}
            onEnter={() => onHover(hover)}
            onLeave={() => onHover(null)}
            onOpen={node ? () => onOpen(node.id) : undefined}
            onToggleBin={node && !isProtected(node) ? () => onToggleBin(node.id) : undefined}
            onDragStart={node && !isProtected(node) ? (e) => startBinDrag(e, node.id) : undefined}
            onReveal={node && onReveal ? () => onReveal(node.id) : undefined}
            onContextMenu={node && onItemMenu ? (e) => onItemMenu(e, node.id) : undefined}
          />
        ))}
      </div>
    </Panel>
  );
}
