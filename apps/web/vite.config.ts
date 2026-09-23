import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  build: {
    target: "es2022",
    sourcemap: false,
    // The site CSP allows images from 'self' only, so never inline assets as data: URIs.
    assetsInlineLimit: 0,
  },
  test: {
    environment: "node",
  },
});
