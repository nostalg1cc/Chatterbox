import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "./index.css";
import App from "./App";

// The desktop WebView does not need browser service-worker lifecycle handling.
if ("serviceWorker" in navigator && !("__TAURI_INTERNALS__" in window)) {
  window.addEventListener(
    "load",
    () => {
      void navigator.serviceWorker.register("/sw.js");
    },
    { once: true }
  );
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>
);
