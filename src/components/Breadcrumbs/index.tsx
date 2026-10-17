import { useLayoutEffect, useRef, useState, type CSSProperties } from "react";
import { HardDrive } from "lucide-react";
import type { Crumb } from "../../lib/api";
import { useElementSize } from "../../hooks/useElementSize";
import { Icon, Menu, cx } from "../ui";
import styles from "./Breadcrumbs.module.css";

type BreadcrumbsProps = { ancestors: Crumb[]; current: Crumb; onNavigate: (id: number) => void };

/** A root-of-volume scan names its top node "/", which reads better as a drive glyph. */
const isVolumeRoot = (c: Crumb, i: number) => i === 0 && c.name === "/";

/** The root crumb and the current one always stay; everything between collapses into "…" as space runs out. */
const KEEP_HEAD = 1;

export default function Breadcrumbs({ ancestors, current, onNavigate }: BreadcrumbsProps) {
  const chain = [...ancestors, current];
  const [rootRef, size] = useElementSize<HTMLElement>();
  const chainRef = useRef<HTMLDivElement>(null);
  const [hidden, setHidden] = useState(0);
  const [menuX, setMenuX] = useState<number | null>(null);
  const maxHidden = Math.max(0, chain.length - KEEP_HEAD - 1);

  // Start from the full chain whenever the available width or the path changes…
  useLayoutEffect(() => {
    setHidden(0);
    setMenuX(null);
  }, [size.w, chain.length, current.id]);
  // …then hide one more middle crumb per pass until it fits. Layout effects run before paint, so no flicker.
  // eslint-disable-next-line react-hooks/exhaustive-deps -- measures after every render on purpose; `hidden` caps the chain
  useLayoutEffect(() => {
    const el = chainRef.current;
    if (el && el.scrollWidth > el.clientWidth && hidden < maxHidden) setHidden(hidden + 1);
  });

  const collapsed = chain.slice(KEEP_HEAD, KEEP_HEAD + hidden);
  const visible = [...chain.slice(0, KEEP_HEAD), ...chain.slice(KEEP_HEAD + hidden)];
  const navigate = (id: number) => {
    setMenuX(null);
    onNavigate(id);
  };

  return (
    <nav ref={rootRef} className={styles.root} style={{ "--menu-x": `${menuX ?? 0}px` } as CSSProperties}>
      <div ref={chainRef} className={cx(styles.chain, hidden === maxHidden && styles.exhausted)}>
        {visible.map((c, i) => {
          const drive = isVolumeRoot(c, i);
          return (
            <div key={c.id} className={styles.crumb}>
              {i === KEEP_HEAD && collapsed.length > 0 && (
                <>
                  <Icon name="chevronRight" size={12} className={styles.sep} />
                  <button
                    type="button"
                    className={styles.btn}
                    aria-label="Show hidden folders"
                    title={collapsed.map((h) => h.name).join(" / ")}
                    onClick={(e) => setMenuX(menuX === null ? e.currentTarget.offsetLeft : null)}
                  >
                    …
                  </button>
                </>
              )}
              {i > 0 && <Icon name="chevronRight" size={12} className={styles.sep} />}
              <button
                type="button"
                className={cx(styles.btn, c.id === current.id && styles.current)}
                aria-label={drive ? "Volume root" : undefined}
                title={drive ? c.name : undefined}
                onClick={() => navigate(c.id)}
              >
                {drive ? <HardDrive size={14} strokeWidth={1.75} /> : <span>{c.name}</span>}
              </button>
            </div>
          );
        })}
      </div>
      <Menu
        open={menuX !== null}
        onClose={() => setMenuX(null)}
        className={styles.menu}
        items={collapsed.map((c) => ({ label: c.name, onSelect: () => navigate(c.id) }))}
      />
    </nav>
  );
}
