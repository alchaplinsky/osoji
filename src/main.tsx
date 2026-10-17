import React from "react";
import ReactDOM from "react-dom/client";
import "@fontsource-variable/hanken-grotesk";
import "@fontsource-variable/geist-mono";
import "./design/tokens.css";
import "./design/themes.css";
import App from "./App";
import Gallery from "./dev/Gallery";
import { ThemeProvider } from "./hooks/useTheme";
import { SettingsProvider } from "./hooks/useSettings";
import { revealWindow } from "./lib/window";

const gallery = new URLSearchParams(location.search).get("gallery");

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <ThemeProvider>
      <SettingsProvider>{gallery === null ? <App /> : <Gallery section={gallery} />}</SettingsProvider>
    </ThemeProvider>
  </React.StrictMode>,
);

// The window starts hidden; show it once the first frame exists. Called
// unconditionally right after render so a failure elsewhere can't leave the
// app with a window that never appears.
revealWindow();
