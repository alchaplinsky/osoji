import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";

// Shown until Tauri answers, and kept when it can't (the browser-only ?gallery mode).
const FALLBACK_VERSION = "0.1.0";

export function useAppVersion(): string {
  const [version, setVersion] = useState(FALLBACK_VERSION);

  useEffect(() => {
    let active = true;
    getVersion()
      .then((v) => active && setVersion(v))
      .catch(() => {});
    return () => {
      active = false;
    };
  }, []);

  return version;
}
