import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

/**
 * The UI is served from `ui/` and built to `ui/dist`, which is the
 * `frontendDist` referenced by `src-tauri/tauri.conf.json`.
 *
 * `host: "0.0.0.0"` and `allowedHosts: true` let the app run inside a
 * container preview as well as on a normal Windows machine. This affects the
 * development server only; the packaged app loads local assets.
 */
export default defineConfig({
  root: "ui",
  base: "./",
  plugins: [react()],
  clearScreen: false,
  server: {
    host: "0.0.0.0",
    port: 1420,
    strictPort: true,
    allowedHosts: true,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
  build: {
    outDir: "dist",
    emptyOutDir: true,
    target: "chrome105",
    sourcemap: false,
  },
});
