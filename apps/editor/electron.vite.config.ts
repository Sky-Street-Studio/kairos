import { resolve } from "node:path";

import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "electron-vite";

export default defineConfig({
  main: {
    resolve: { alias: { "@shared": resolve(import.meta.dirname, "src/shared") } },
  },
  preload: {
    resolve: { alias: { "@shared": resolve(import.meta.dirname, "src/shared") } },
    build: {
      rollupOptions: {
        // Sandboxed preloads cannot be ES modules.
        output: { format: "cjs", entryFileNames: "[name].cjs" },
      },
    },
  },
  renderer: {
    resolve: {
      alias: {
        "@": resolve(import.meta.dirname, "src/renderer/src"),
        "@shared": resolve(import.meta.dirname, "src/shared"),
      },
    },
    plugins: [react(), tailwindcss()],
    worker: { format: "es" },
  },
});
