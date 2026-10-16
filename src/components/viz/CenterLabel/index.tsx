import type { ViewNode } from "../../../lib/api";
import { humanBytes } from "../../../lib/format";
import { Eyebrow, Mono } from "../../ui";
import styles from "./CenterLabel.module.css";

type CenterLabelProps = {
  view: ViewNode;
  /** Font size of the size numeral in px. */
  size: number;
};

/** Size and name of the current folder, centred over a radial chart. */
export default function CenterLabel({ view, size }: CenterLabelProps) {
  return (
    <div className={styles.center}>
      <Mono numeral size={size}>
        {humanBytes(view.size)}
      </Mono>
      <Eyebrow className={styles.name}>{view.name}</Eyebrow>
    </div>
  );
}
