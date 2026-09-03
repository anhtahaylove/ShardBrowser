import React from "react";
import ReactDOM from "react-dom/client";
import { ThemeProvider } from "@proxyshard/shardx-ui-kit";
import "./styles/index.css";
import "./styles/app.css";
import "flag-icons/css/flag-icons.min.css";
import { App } from "./App";
import { SyncPanel } from "../widgets/SyncPanel";
import { HelperPanel } from "../widgets/HelperPanel";
import { initAnalytics } from "../shared/lib/analytics";

// The always-on-top panels are second Tauri windows on this same bundle,
// addressed by hash — a 60px strip needs no vite entry of its own.
const panelParams = new URLSearchParams(
  window.location.hash.replace(/^#\/?/, ""),
);
const panelGroup = panelParams.get("syncPanel");
const helperProfile = panelParams.get("helperPanel");

// The launcher window only — a panel is a second window on the same bundle and
// would otherwise report a second user for one person.
if (!panelGroup && !helperProfile) void initAnalytics();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ThemeProvider>
      {panelGroup ? <SyncPanel group={panelGroup} />
       : helperProfile ? <HelperPanel profile={helperProfile} />
       : <App />}
    </ThemeProvider>
  </React.StrictMode>,
);
