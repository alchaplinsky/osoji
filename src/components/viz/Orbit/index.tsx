import { useMemo } from "react";
import { useElementSize } from "../../../hooks/useElementSize";
import { humanPercent } from "../../../lib/format";
import type { HoverId } from "../../../lib/types";
import { visibleSlices } from "../../../screens/Breakdown/slices";
import { neutral } from "../../../design/palette";
import { THEMES } from "../../../design/theme";
import { binnedKeys, relatedKeys, relatedOpacity } from "../../Sunburst/layout";
import { cx } from "../../ui";
import CenterLabel from "../CenterLabel";
import { GlassDefs, glass, tint, useLingering, usePaints } from "../glass";
import { pointerProps } from "../interact";
import type { VizProps } from "../types";
import MoonLabels from "./MoonLabels";
import { CORE, RINGS, orbitGeometry, placeMoons } from "./moons";
import styles from "./Orbit.module.css";

export default function Orbit({ view, theme, hoverId, onHover, onDrill, onUp, onSelectFile, onContextMenu, binIds, centerSize = 20, className }: VizProps) {
  const [rootRef, size] = useElementSize<HTMLDivElement>();
  const { id, paint } = usePaints();
  const slices = useMemo(() => visibleSlices(view, theme), [view, theme]);
  const related = useMemo(() => relatedKeys(view, hoverId), [view, hoverId]);
  const binned = useMemo(() => binnedKeys(view, binIds), [view, binIds]);
  const geometry = useMemo(() => orbitGeometry(size.w, size.h), [size]);
  const moons = useMemo(() => placeMoons(slices, view.size, geometry), [slices, view.size, geometry]);
  const { cx: x0, cy: y0, R } = geometry;
  const paused = hoverId !== null;
  const glowing = useLingering<HoverId>(hoverId);
  const hovered = paused ? slices.find((s) => s.hover === hoverId) : undefined;

  return (
    <div ref={rootRef} className={cx(styles.root, className)}>
      {size.w > 0 && size.h > 0 && (
        <>
          <svg className={styles.svg} width={size.w} height={size.h}>
            <GlassDefs id={id} neutral={neutral(theme)} />
            {RINGS.map((f) => (
              <circle key={f} className={styles.ring} cx={x0} cy={y0} r={Math.round(R * f)} />
            ))}
            <g className={cx(styles.spin, paused && styles.paused)} style={{ transformOrigin: `${x0}px ${y0}px` }}>
              {moons.map(({ slice, x, y, r }) => {
                const inBin = binned.has(slice.key);
                const hovered = hoverId === slice.hover;
                return (
                  <g
                    key={slice.key}
                    className={cx(styles.moon, !slice.node && styles.rest, hovered && cx(styles.lifted, glass.hovered), inBin && glass.binned)}
                    style={{ opacity: relatedOpacity(related, slice.key) }}
                    {...pointerProps(slice.node, slice.hover, { onHover, onDrill, onSelectFile, onContextMenu })}
                  >
                    {!inBin && glowing.includes(slice.hover) && <circle className={glass.halo} filter={paint("glow")} cx={x} cy={y} r={r} style={tint(slice.fill)} />}
                    <circle
                      className={glass.shape}
                      filter={!inBin && THEMES[theme].dark ? paint("halo") : undefined}
                      cx={x}
                      cy={y}
                      r={r}
                      style={tint(slice.fill, inBin ? { fill: paint("binned") } : undefined)}
                    />
                    {!inBin && <circle className={glass.gloss} cx={x} cy={y} r={r} style={{ fill: paint("gloss") }} />}
                  </g>
                );
              })}
            </g>
            <circle className={styles.core} cx={x0} cy={y0} r={Math.round(R * CORE)} onClick={onUp} />
          </svg>
          <MoonLabels moons={moons} geometry={geometry} hoverId={hoverId} related={related} />
          <CenterLabel
            name={hovered?.name ?? view.name}
            size={hovered?.size ?? view.size}
            note={hovered && view.size > 0 ? humanPercent((hovered.size / view.size) * 100) : undefined}
            fontSize={centerSize}
          />
        </>
      )}
    </div>
  );
}
