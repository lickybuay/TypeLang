import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./theme.css";

// Keep the browser-style right-click menu (Inspect Element, Reload, …)
// available during development, but hide it in the shipped app — a native
// utility shouldn't expose a webview context menu.
if (!import.meta.env.DEV) {
  document.addEventListener("contextmenu", (e) => e.preventDefault());
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
