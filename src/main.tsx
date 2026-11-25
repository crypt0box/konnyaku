import { getCurrentWindow } from "@tauri-apps/api/window";
import React from "react";
import ReactDOM from "react-dom/client";
import { SettingsWindow } from "./windows/settings/SettingsWindow";
import { TranslationWindow } from "./windows/translation/TranslationWindow";
import "./styles.css";

const isSettings = getCurrentWindow().label === "settings";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>{isSettings ? <SettingsWindow /> : <TranslationWindow />}</React.StrictMode>,
);
