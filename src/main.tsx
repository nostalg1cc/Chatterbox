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

// Without this, the browser treats the media cache's IndexedDB store
// (src/lib/media-cache.ts) as best-effort and can silently evict it under
// disk pressure - defeating its whole point of keeping chat media viewable
// for ~30 days after it expires remotely. Best-effort: browsers may still
// decline (e.g. no site-engagement history yet), but this at least asks.
void navigator.storage?.persist?.();

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>
);
