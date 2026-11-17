import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

// Tauri expects a fixed port and watches src-tauri itself.
export default defineConfig({
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  envPrefix: ["VITE_", "TAURI_ENV_"],
  build: {
    // The oldest engines we ship on: WebKitGTK and WKWebView on macOS 14 (Safari 17).
    target: ["es2023", "safari17"],
    outDir: "dist",
    // Two windows, two pages: the main window and the Mini Window.
    rollupOptions: { input: { main: "index.html", mini: "mini.html" } },
  },
  test: {
    environment: "jsdom",
    setupFiles: ["./src/test/setup.ts"],
  },
});
