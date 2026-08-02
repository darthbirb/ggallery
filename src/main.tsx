import React from "react";
import ReactDOM from "react-dom/client";

import App from "./App";
import "./styles/index.css";

// A WebView's default context menu — Back, Reload, Save as… — is not part of
// this application. The app's own menus are opened by the surfaces that have
// one (see `features/menus`); this is the backstop for everywhere else, so the
// browser's never appears in a desktop window.
document.addEventListener("contextmenu", (event) => event.preventDefault());

const root = document.getElementById("root");
if (root) {
  ReactDOM.createRoot(root).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  );
}
