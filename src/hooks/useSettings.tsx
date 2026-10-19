import { createContext, useCallback, useContext, useEffect, useMemo, useState, type ReactNode } from "react";

const STORAGE_KEY = "osoji.settings";

type Settings = { confirmBig: boolean; watchers: boolean };
type SettingsContextValue = Settings & { setConfirmBig: (v: boolean) => void; setWatchers: (v: boolean) => void };

const DEFAULTS: Settings = { confirmBig: true, watchers: false };

const SettingsContext = createContext<SettingsContextValue | null>(null);

function initial(): Settings {
  let stored: Partial<Settings> = {};
  try {
    stored = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "{}");
  } catch {}
  return {
    confirmBig: typeof stored.confirmBig === "boolean" ? stored.confirmBig : DEFAULTS.confirmBig,
    watchers: typeof stored.watchers === "boolean" ? stored.watchers : DEFAULTS.watchers,
  };
}

export function SettingsProvider({ children }: { children: ReactNode }) {
  const [settings, setSettings] = useState<Settings>(initial);

  useEffect(() => {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(settings));
  }, [settings]);

  const setConfirmBig = useCallback(
    (confirmBig: boolean) => setSettings((s) => (s.confirmBig === confirmBig ? s : { ...s, confirmBig })),
    [],
  );
  const setWatchers = useCallback(
    (watchers: boolean) => setSettings((s) => (s.watchers === watchers ? s : { ...s, watchers })),
    [],
  );

  const value = useMemo<SettingsContextValue>(
    () => ({ ...settings, setConfirmBig, setWatchers }),
    [settings, setConfirmBig, setWatchers],
  );

  return <SettingsContext.Provider value={value}>{children}</SettingsContext.Provider>;
}

export function useSettings(): SettingsContextValue {
  const ctx = useContext(SettingsContext);
  if (!ctx) throw new Error("useSettings must be used inside <SettingsProvider>");
  return ctx;
}
