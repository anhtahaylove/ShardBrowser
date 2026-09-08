import React from "react";
import ReactDOM from "react-dom/client";
import { ThemeProvider } from "@proxyshard/shardx-ui-kit";
import "./styles/index.css";
import "./styles/app.css";
import "flag-icons/css/flag-icons.min.css";
import { App } from "./App";
import { SyncPanel } from "../widgets/SyncPanel";
import { HelperPanel } from "../widgets/HelperPanel";

// The always-on-top panels are second Tauri windows on this same bundle,
// addressed by hash — a 60px strip needs no vite entry of its own.
const panelParams = new URLSearchParams(
  window.location.hash.replace(/^#\/?/, ""),
);
const panelGroup = panelParams.get("syncPanel");
const helperProfile = panelParams.get("helperPanel");

// Upstream's Google Analytics beacon is deliberately absent from this custom
// build: a fleet anti-detect launcher should not phone a third party on start.

async function bootstrap() {
  // The e2e build stubs the Tauri command layer so the UI regression suite can
  // drive real components without a backend. Loading it must happen before the
  // first render, or components fire real invokes on mount.
  if (import.meta.env.MODE === "e2e") {
    await import("../e2e-mock");
  }

  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <ThemeProvider>
        {panelGroup ? <SyncPanel group={panelGroup} />
         : helperProfile ? <HelperPanel profile={helperProfile} />
         : <App />}
      </ThemeProvider>
    </React.StrictMode>,
  );
}

void bootstrap();
