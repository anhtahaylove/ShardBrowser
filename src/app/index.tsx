import React from "react";
import ReactDOM from "react-dom/client";
import { ThemeProvider } from "@proxyshard/shardx-ui-kit";
import "./styles/index.css";
import "./styles/app.css";
import "flag-icons/css/flag-icons.min.css";
import { App } from "./App";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ThemeProvider>
      <App />
    </ThemeProvider>
  </React.StrictMode>,
);
