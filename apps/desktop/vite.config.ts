import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

// Tauri expects a fixed port and no clearing of its output.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: "localhost",
    watch: { ignored: ["**/src-tauri/**"] },
  },
  build: {
    target: "es2022",
    // No source maps in release builds: nothing to leak, nothing to load.
    sourcemap: false,
    // Inline assets would need `data:` in more CSP directives; keep them as files.
    assetsInlineLimit: 0,
  },
  test: {
    environment: "node",
  },
});
