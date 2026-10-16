import { createContext, useCallback, useContext, useEffect, useMemo, useState, type ReactNode } from "react";
import { applyTheme, isLayout, isTheme, type LayoutName, type ThemeName } from "../design/theme";

const STORAGE_KEY = "unshift.appearance";

type Appearance = { theme: ThemeName; layout: LayoutName };
type ThemeContextValue = Appearance & { setTheme: (t: ThemeName) => void; setLayout: (l: LayoutName) => void };

const ThemeContext = createContext<ThemeContextValue | null>(null);

function initial(): Appearance {
  let stored: Partial<Appearance> = {};
  try {
    stored = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "{}");
  } catch {}
  const systemLight = window.matchMedia("(prefers-color-scheme: light)").matches;
  return {
    theme: isTheme(stored.theme) ? stored.theme : systemLight ? "light" : "instrument",
    layout: isLayout(stored.layout) ? stored.layout : "cards",
  };
}

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [appearance, setAppearance] = useState<Appearance>(initial);

  useEffect(() => {
    applyTheme(appearance.theme, appearance.layout);
    localStorage.setItem(STORAGE_KEY, JSON.stringify(appearance));
  }, [appearance]);

  const setTheme = useCallback((theme: ThemeName) => setAppearance((a) => (a.theme === theme ? a : { ...a, theme })), []);
  const setLayout = useCallback((layout: LayoutName) => setAppearance((a) => (a.layout === layout ? a : { ...a, layout })), []);

  const value = useMemo<ThemeContextValue>(() => ({ ...appearance, setTheme, setLayout }), [appearance, setTheme, setLayout]);

  return <ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>;
}

export function useTheme(): ThemeContextValue {
  const ctx = useContext(ThemeContext);
  if (!ctx) throw new Error("useTheme must be used inside <ThemeProvider>");
  return ctx;
}
