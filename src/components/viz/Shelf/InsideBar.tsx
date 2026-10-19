import type { ThemeName } from "../../../design/theme";
import { tone } from "../../../design/palette";
import type { ViewNode } from "../../../lib/api";
import type { HoverId } from "../../../lib/types";
import { humanBytes } from "../../../lib/format";
import { relatedOpacity, type SliceKey } from "../../Sunburst/layout";
import { Eyebrow, Mono, cx } from "../../ui";
import { glass, tint } from "../glass";
import { pointerProps, type PointerActions } from "../interact";
import styles from "./Shelf.module.css";

type InsideBarProps = {
  focus: ViewNode;
  theme: ThemeName;
  hoverId: HoverId;
  related: Set<SliceKey> | null;
  binned: Set<SliceKey>;
  actions: PointerActions;
};

const KIDS = 12;
const LABEL_SHARE = 13;

export default function InsideBar({ focus, theme, hoverId, related, binned, actions }: InsideBarProps) {
  const kids = focus.children.slice(0, KIDS);
  return (
    <div>
      <div className={styles.insideHead}>
        <Eyebrow>Inside</Eyebrow>
        <span className={styles.insideName}>{focus.name}</span>
        <Mono size={12} className={styles.insideSize}>
          {humanBytes(focus.size)}
        </Mono>
      </div>
      <div className={styles.insideBar}>
        {kids.map((child, i) => {
          const pct = focus.size > 0 ? (child.size / focus.size) * 100 : 0;
          return (
            <div
              key={child.id}
              className={cx(styles.insideSegment, glass.piece, hoverId === child.id && cx(styles.lifted, glass.hovered), binned.has(child.id) && glass.binned)}
              style={tint(tone(theme, i, kids.length, 2), { width: `${pct}%`, opacity: relatedOpacity(related, child.id) })}
              {...pointerProps(child, child.id, actions)}
            >
              {pct >= LABEL_SHARE && <span className={styles.insideLabel}>{child.name}</span>}
            </div>
          );
        })}
      </div>
    </div>
  );
}
