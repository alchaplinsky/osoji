import { useEffect, useRef, useState } from "react";

const SETTLED = 1e-4;

/**
 * Eases a list of numbers toward their targets every frame, so values that arrive in steps
 * (scan progress at 10 Hz, whole directories at a time) move continuously. The gap to each
 * target shrinks by 1/e every `tau` ms. With `snap` the values jump to their targets instead,
 * which is how a fresh scan starts from zero rather than easing down from the last one.
 * Targets that appear later start from zero.
 */
export function useSmoothed(targets: number[], tau: number, snap = false): number[] {
  const [values, setValues] = useState(targets);
  const latest = useRef(targets);
  const current = useRef(values);
  latest.current = targets;
  const key = targets.join(",");

  useEffect(() => {
    if (snap) {
      current.current = latest.current;
      setValues(latest.current);
      return;
    }
    let frame = 0;
    let last = performance.now();
    const step = (now: number) => {
      const k = 1 - Math.exp(-(now - last) / tau);
      last = now;
      const goal = latest.current;
      let settled = true;
      const next = goal.map((target, i) => {
        const from = current.current[i] ?? 0;
        const v = from + (target - from) * k;
        if (Math.abs(target - v) > SETTLED) settled = false;
        return v;
      });
      current.current = settled ? goal : next;
      setValues(current.current);
      if (!settled) frame = requestAnimationFrame(step);
    };
    frame = requestAnimationFrame(step);
    return () => cancelAnimationFrame(frame);
  }, [key, tau, snap]);

  return values;
}
