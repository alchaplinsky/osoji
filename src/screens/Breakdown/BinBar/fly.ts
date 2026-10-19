const LIFT = 220;
const FLY = 520;
const LANDED = LIFT + FLY;

/**
 * A copy of `source` lifts off the chart, then flies into `target` (the bin's icon), shrinking and fading,
 * and the target pops as it lands. The copy lives on top of the page, so the chart can redraw underneath.
 */
export function flyToBin(source: Element, target: Element | null) {
  if (!target || matchMedia("(prefers-reduced-motion: reduce)").matches) return;
  const ghost = ghostOf(source);
  if (!ghost) return;
  document.body.append(ghost.el);
  const to = target.getBoundingClientRect();
  const dx = to.left + to.width / 2 - (ghost.rect.left + ghost.rect.width / 2);
  const dy = to.top + to.height / 2 - (ghost.rect.top + ghost.rect.height / 2);
  const flight = ghost.el.animate(
    [
      { transform: "none", opacity: 1, easing: "cubic-bezier(.2, .9, .2, 1)" },
      { transform: "translateY(-8px) scale(1.04)", opacity: 1, offset: LIFT / LANDED, easing: "cubic-bezier(.6, 0, .2, 1)" },
      { opacity: 1, offset: (LIFT + FLY * 0.4) / LANDED },
      { transform: `translate(${dx}px, ${dy}px) scale(.08)`, opacity: 0 },
    ],
    { duration: LANDED },
  );
  const pop = () => target.animate([{ transform: "scale(1)" }, { transform: "scale(1.3)" }, { transform: "scale(1)" }], { duration: 400, easing: "cubic-bezier(.2, .9, .2, 1)" });
  flight.finished.then(pop, () => {}).finally(() => ghost.el.remove());
}

/** A fixed-position copy of `source` over its current place on screen; SVG shapes are wrapped in an svg of their own. */
function ghostOf(source: Element): { el: HTMLElement | SVGSVGElement; rect: DOMRect } | null {
  let rect = source.getBoundingClientRect();
  if (rect.width === 0 || rect.height === 0) return null;
  const copy = source.cloneNode(true) as Element;
  let el: HTMLElement | SVGSVGElement;
  if (source instanceof SVGGraphicsElement) {
    // The copy keeps its own coordinates; the wrapper's viewBox frames them over the same spot on screen.
    const box = source.getBBox();
    el = document.createElementNS("http://www.w3.org/2000/svg", "svg");
    el.setAttribute("viewBox", `${box.x} ${box.y} ${box.width} ${box.height}`);
    el.style.overflow = "visible";
    el.append(copy);
    const cx = rect.left + rect.width / 2;
    const cy = rect.top + rect.height / 2;
    rect = new DOMRect(cx - box.width / 2, cy - box.height / 2, box.width, box.height);
  } else if (copy instanceof HTMLElement) {
    el = copy;
    el.style.margin = "0";
  } else {
    return null;
  }
  Object.assign(el.style, {
    position: "fixed",
    left: `${rect.left}px`,
    top: `${rect.top}px`,
    width: `${rect.width}px`,
    height: `${rect.height}px`,
    zIndex: "100",
    pointerEvents: "none",
    opacity: "1",
  });
  return { el, rect };
}
