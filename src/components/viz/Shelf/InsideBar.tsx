import type { ThemeName } from "../../../design/theme";
import { hueAt, shade } from "../../../design/palette";
import type { ViewNode } from "../../../lib/api";
import { humanBytes } from "../../../lib/format";
import { relatedOpacity, type SliceKey } from "../../Sunburst/layout";
import { Eyebrow, Mono } from "../../ui";
import { pointerProps, type PointerActions } from "../interact";
import styles from "./Shelf.module.css";

type InsideBarProps = {
  focus: ViewNode;
  theme: ThemeName;
  related: Set<SliceKey> | null;
  actions: PointerActions;
};

const KIDS = 12;
const LABEL_SHARE = 13;

/** A slimmer bar breaking down one folder of the shelf into its largest children. */
export default function InsideBar({ focus, theme, related, actions }: InsideBarProps) {
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
        {focus.children.slice(0, KIDS).map((child, i) => {
          const pct = focus.size > 0 ? (child.size / focus.size) * 100 : 0;
          return (
            <div
              key={child.id}
              className={styles.insideSegment}
              style={{ width: `${pct}%`, background: shade(theme, hueAt(i), 2), opacity: relatedOpacity(related, child.id) }}
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
