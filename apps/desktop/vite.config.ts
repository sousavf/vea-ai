import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    host: "127.0.0.1",
    port: 1420,
    strictPort: true,
  },
  envPrefix: ["VITE_", "TAURI_ENV_*"],
  test: {
    environment: "jsdom",
    setupFiles: ["./src/test-setup.ts"],
  },
  build: {
    // Tauri v2 ships modern system webviews; an ES2022 target avoids lossy transforms
    // in dependencies while retaining support across the documented desktop baseline.
    target: "es2022",
    minify: process.env.TAURI_ENV_DEBUG ? false : "esbuild",
    sourcemap: Boolean(process.env.TAURI_ENV_DEBUG),
  },
});
