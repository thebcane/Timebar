import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import QuickPanel from "./windows/QuickPanel";
import MainWindow from "./windows/MainWindow";
import "./styles/tokens.css";

// One Vite build, two windows: pick the surface by window label (Section 10).
const label = getCurrentWindow().label;
const Root = label === "quick" ? QuickPanel : MainWindow;

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <Root />
  </React.StrictMode>,
);
