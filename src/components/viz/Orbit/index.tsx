import { useMemo } from "react";
import { useElementSize } from "../../../hooks/useElementSize";
import { visibleSlices } from "../../../screens/Breakdown/slices";
import { relatedKeys, relatedOpacity } from "../../Sunburst/layout";
import { cx } from "../../ui";
import CenterLabel from "../CenterLabel";
import { pointerProps } from "../interact";
import type { VizProps } from "../types";
import MoonLabels from "./MoonLabels";
import { CORE, RINGS, orbitGeometry, placeMoons } from "./moons";
import styles from "./Orbit.module.css";

export default function Orbit({ view, theme, hoverId, onHover, onDrill, onUp, onSelectFile, onContextMenu, centerSize = 20, className }: VizProps) {
  const [rootRef, size] = useElementSize<HTMLDivElement>();
  const slices = useMemo(() => visibleSlices(view, theme), [view, theme]);
  const related = useMemo(() => relatedKeys(view, hoverId), [view, hoverId]);
  const geometry = useMemo(() => orbitGeometry(size.w, size.h), [size]);
  const moons = useMemo(() => placeMoons(slices, view.size, geometry), [slices, view.size, geometry]);
  const { cx: x0, cy: y0, R } = geometry;
  const paused = hoverId !== null;

  return (
    <div ref={rootRef} className={cx(styles.root, className)}>
      {size.w > 0 && size.h > 0 && (
        <>
          <svg className={styles.svg} width={size.w} height={size.h}>
            {RINGS.map((f) => (
              <circle key={f} className={styles.ring} cx={x0} cy={y0} r={Math.round(R * f)} />
            ))}
            <g className={cx(styles.spin, paused && styles.paused)} style={{ transformOrigin: `${x0}px ${y0}px` }}>
              {moons.map(({ slice, x, y, r }) => (
                <circle
                  key={slice.key}
                  className={cx(styles.moon, !slice.node && styles.rest)}
                  cx={x}
                  cy={y}
                  r={r}
                  fill={slice.fill}
                  style={{ opacity: relatedOpacity(related, slice.key) }}
                  {...pointerProps(slice.node, slice.hover, { onHover, onDrill, onSelectFile, onContextMenu })}
                />
              ))}
            </g>
            <circle className={styles.core} cx={x0} cy={y0} r={Math.round(R * CORE)} onClick={onUp} />
          </svg>
          <MoonLabels moons={moons} geometry={geometry} hoverId={hoverId} related={related} />
          <CenterLabel name={view.name} size={view.size} fontSize={centerSize} />
        </>
      )}
    </div>
  );
}
