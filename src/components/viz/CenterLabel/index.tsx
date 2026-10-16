import type { ViewNode } from "../../../lib/api";
import { humanBytes } from "../../../lib/format";
import { Eyebrow, Mono } from "../../ui";
import styles from "./CenterLabel.module.css";

type CenterLabelProps = {
  view: ViewNode;
  size: number;
};

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
