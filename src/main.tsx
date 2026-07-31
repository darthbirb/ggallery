import React from "react";
import ReactDOM from "react-dom/client";

import App from "./App";
import "./styles/index.css";

// A WebView's default context menu — Back, Reload, Save as… — is not part of
// this application. Right-click gets the app's own menus in M2; until then it
// does nothing at all, which is still the correct behaviour for a desktop
// window.
document.addEventListener("contextmenu", (event) => event.preventDefault());

const root = document.getElementById("root");
if (root) {
  ReactDOM.createRoot(root).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  );
}
