import type { ComponentProps } from "react";
import { humanBytes } from "../../../lib/format";
import type { Slice } from "../../../screens/Breakdown/slices";
import { Mono, cx } from "../../ui";
import { pointerProps, type PointerActions } from "../interact";
import styles from "./Shelf.module.css";

type LegendProps = {
  slices: Slice[];
  actions: PointerActions;
};

export default function Legend({ slices, actions }: LegendProps) {
  return (
    <div className={styles.legend}>
      {slices.map((slice) => (
        <LegendItem key={slice.key} slice={slice} {...pointerProps(slice.node, slice.hover, actions)} />
      ))}
    </div>
  );
}

function LegendItem({ slice, ...handlers }: { slice: Slice } & ComponentProps<"div">) {
  return (
    <div className={cx(styles.legendItem, !slice.node && styles.rest)} {...handlers}>
      <span className={styles.swatch} style={{ background: slice.fill }} />
      <span className={styles.legendName}>{slice.name}</span>
      <Mono className={styles.legendSize}>{humanBytes(slice.size)}</Mono>
    </div>
  );
}
