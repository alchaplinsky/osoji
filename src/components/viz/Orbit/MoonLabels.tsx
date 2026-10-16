import type { HoverId } from "../../../lib/types";
import { humanBytes } from "../../../lib/format";
import { relatedOpacity, type SliceKey } from "../../Sunburst/layout";
import { Mono, cx } from "../../ui";
import type { Moon, OrbitGeometry } from "./moons";
import styles from "./Orbit.module.css";

type MoonLabelsProps = {
  moons: Moon[];
  geometry: OrbitGeometry;
  hoverId: HoverId;
  related: Set<SliceKey> | null;
};

const LABEL_W = 120;
const GAP = 6;
const MIN_R = 15;

/** Names under the moons; the layer spins with them while each label counter-spins to stay upright. */
export default function MoonLabels({ moons, geometry, hoverId, related }: MoonLabelsProps) {
  const paused = hoverId !== null;
  return (
    <div className={cx(styles.labels, paused && styles.paused)} style={{ transformOrigin: `${geometry.cx}px ${geometry.cy}px` }}>
      {moons.map(({ slice, x, y, r }) =>
        r >= MIN_R || hoverId === slice.hover ? (
          <div
            key={slice.key}
            className={cx(styles.label, paused && styles.paused)}
            style={{
              left: x - LABEL_W / 2,
              top: y + r + GAP,
              transformOrigin: `${LABEL_W / 2}px ${-(r + GAP)}px`,
              opacity: relatedOpacity(related, slice.key),
            }}
          >
            <div className={styles.labelName}>{slice.name}</div>
            <Mono className={styles.labelSize}>{humanBytes(slice.size)}</Mono>
          </div>
        ) : null,
      )}
    </div>
  );
}
