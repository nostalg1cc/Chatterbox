import path from "node:path";
import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

// Tauri dev server settings: fixed port, no auto-open
export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  clearScreen: false,
  server: {
    port: 5174,
    strictPort: true,
  },
  envPrefix: ["VITE_", "TAURI_ENV_"],
  build: {
    rollupOptions: {
      // The floating voice HUD is a separate, minimal entry point loaded by
      // its own Tauri window (see src-tauri/tauri.conf.json "voice-hud"),
      // not part of the main app bundle.
      input: {
        main: path.resolve(__dirname, "index.html"),
        voiceHud: path.resolve(__dirname, "voice-hud.html"),
      },
    },
  },
});
