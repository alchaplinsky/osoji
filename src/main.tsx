import React from "react";
import ReactDOM from "react-dom/client";
import "@fontsource-variable/hanken-grotesk";
import "@fontsource-variable/geist-mono";
import "./design/tokens.css";
import "./design/themes.css";
import "./styles.css";
import App from "./App";
import Gallery from "./dev/Gallery";
import { ThemeProvider } from "./hooks/useTheme";

const gallery = new URLSearchParams(location.search).has("gallery");

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <ThemeProvider>{gallery ? <Gallery /> : <App />}</ThemeProvider>
  </React.StrictMode>,
);
