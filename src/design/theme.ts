export type ThemeName = "instrument" | "glass" | "light";
export type LayoutName = "cards" | "classic";

export type ThemeMeta = {
  label: string;
  desc: string;
  dark: boolean;
  swatch: { bg: string; card: string; ac: string };
  chart: { l: number; c: number };
};

export const THEMES: Record<ThemeName, ThemeMeta> = {
  instrument: {
    label: "Instrument",
    desc: "Amber on charcoal",
    dark: true,
    swatch: { bg: "#0C0C0E", card: "#1C1C1F", ac: "#E0A458" },
    chart: { l: 0.76, c: 0.15 },
  },
  glass: {
    label: "Glass",
    desc: "Blue-teal & copper",
    dark: true,
    swatch: { bg: "#0B1520", card: "rgba(255,255,255,.14)", ac: "#DB9163" },
    chart: { l: 0.76, c: 0.14 },
  },
  light: {
    label: "Light",
    desc: "Coral & sand",
    dark: false,
    swatch: { bg: "#F4F0E9", card: "#FFFFFF", ac: "#E8603C" },
    chart: { l: 0.72, c: 0.16 },
  },
};

export const LAYOUTS: Record<LayoutName, { label: string }> = {
  cards: { label: "Floating cards" },
  classic: { label: "Classic panes" },
};

export const THEME_NAMES = Object.keys(THEMES) as ThemeName[];
export const LAYOUT_NAMES = Object.keys(LAYOUTS) as LayoutName[];

export const isTheme = (v: unknown): v is ThemeName => typeof v === "string" && v in THEMES;
export const isLayout = (v: unknown): v is LayoutName => typeof v === "string" && v in LAYOUTS;

export function applyTheme(theme: ThemeName, layout: LayoutName) {
  const el = document.documentElement;
  el.dataset.theme = theme;
  el.dataset.layout = layout;
}
