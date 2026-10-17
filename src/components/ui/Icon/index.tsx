import type { SVGAttributes } from "react";

const ICONS = {
  scan: { box: 16, d: "M8 2.2a5.8 5.8 0 1 1-5.8 5.8M8 5a3 3 0 1 0 3 3M8 8l3.6-3.6" },
  breakdown: { box: 16, d: "M8 2.2A5.8 5.8 0 1 0 13.8 8H8V2.2z" },
  largest: { box: 16, d: "M2.5 4h11M2.5 8h7M2.5 12h4" },
  suggest: { box: 16, d: "M8 2l1.5 4.5L14 8l-4.5 1.5L8 14l-1.5-4.5L2 8l4.5-1.5z" },
  watch: { box: 16, d: "M1.8 8s2.3-4.3 6.2-4.3S14.2 8 14.2 8s-2.3 4.3-6.2 4.3S1.8 8 1.8 8zM8 6a2 2 0 1 0 0 4a2 2 0 1 0 0-4z" },
  settings: {
    box: 16,
    d: "M8.15 1.33h-0.29a1.33 1.33 0 0 0-1.33 1.33v0.12a1.33 1.33 0 0 1-0.67 1.15l-0.29 0.17a1.33 1.33 0 0 1-1.33 0l-0.1-0.05a1.33 1.33 0 0 0-1.82 0.49l-0.15 0.25a1.33 1.33 0 0 0 0.49 1.82l0.1 0.07a1.33 1.33 0 0 1 0.67 1.15v0.34a1.33 1.33 0 0 1-0.67 1.16l-0.1 0.06a1.33 1.33 0 0 0-0.49 1.82l0.15 0.25a1.33 1.33 0 0 0 1.82 0.49l0.1-0.05a1.33 1.33 0 0 1 1.33 0l0.29 0.17a1.33 1.33 0 0 1 0.67 1.15V13.33a1.33 1.33 0 0 0 1.33 1.33h0.29a1.33 1.33 0 0 0 1.33-1.33v-0.12a1.33 1.33 0 0 1 0.67-1.15l0.29-0.17a1.33 1.33 0 0 1 1.33 0l0.1 0.05a1.33 1.33 0 0 0 1.82-0.49l0.15-0.26a1.33 1.33 0 0 0-0.49-1.82l-0.1-0.05a1.33 1.33 0 0 1-0.67-1.16v-0.33a1.33 1.33 0 0 1 0.67-1.16l0.1-0.06a1.33 1.33 0 0 0 0.49-1.82l-0.15-0.25a1.33 1.33 0 0 0-1.82-0.49l-0.1 0.05a1.33 1.33 0 0 1-1.33 0l-0.29-0.17a1.33 1.33 0 0 1-0.67-1.15V2.67a1.33 1.33 0 0 0-1.33-1.33zM8 6a2 2 0 1 0 0 4a2 2 0 1 0 0-4z",
  },
  list: { box: 16, d: "M2.5 3.5h11M2.5 8h11M2.5 12.5h11" },
  trash: { box: 18, d: "M3 4.5h12M7 4.5V3h4v1.5M4.5 4.5l.8 10.5h7.4l.8-10.5M7.5 7.5v5M10.5 7.5v5" },
  chevronDown: { box: 12, d: "M3 4.5l3 3 3-3" },
  chevronRight: { box: 12, d: "M4.5 2.5L8 6l-3.5 3.5" },
  chevronRightSm: { box: 10, d: "M3.5 2L7 5.5 3.5 9" },
  close: { box: 10, d: "M2 2l6 6M8 2l-6 6" },
  check: { box: 10, d: "M2 5.2l2.2 2.2L8.3 3" },
  checkLg: { box: 12, d: "M2.5 6.2l2.4 2.4 4.8-5.2" },
} as const;

export type IconName = keyof typeof ICONS | "more";

type Props = Omit<SVGAttributes<SVGSVGElement>, "name"> & {
  name: IconName;
  size?: number;
  strokeWidth?: number;
};

export default function Icon({ name, size, strokeWidth, style, ...rest }: Props) {
  if (name === "more") {
    const s = size ?? 16;
    return (
      <svg width={s} height={s} viewBox="0 0 16 16" fill="currentColor" style={{ flex: "none", ...style }} {...rest}>
        <circle cx="3.5" cy="8" r="1.3" />
        <circle cx="8" cy="8" r="1.3" />
        <circle cx="12.5" cy="8" r="1.3" />
      </svg>
    );
  }
  const { box, d } = ICONS[name];
  const s = size ?? box;
  return (
    <svg
      width={s}
      height={s}
      viewBox={`0 0 ${box} ${box}`}
      fill="none"
      stroke="currentColor"
      strokeWidth={strokeWidth ?? 1.5}
      strokeLinecap="round"
      strokeLinejoin="round"
      style={{ flex: "none", ...style }}
      {...rest}
    >
      <path d={d} />
    </svg>
  );
}
