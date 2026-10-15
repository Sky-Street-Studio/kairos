import { resolve } from "node:path";

import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "electron-vite";

export default defineConfig(({ mode }) => ({
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
    plugins: [
      react(),
      tailwindcss(),
      ...(mode === "development"
        ? [
            {
              name: "kairos-development-csp",
              transformIndexHtml(html: string) {
                const scriptSrc = "script-src 'self'";
                const connectSrc = "connect-src 'self'";
                if (!html.includes(scriptSrc)) {
                  throw new Error(`kairos-development-csp: missing ${scriptSrc} in index.html`);
                }
                if (!html.includes(connectSrc)) {
                  throw new Error(`kairos-development-csp: missing ${connectSrc} in index.html`);
                }
                return html
                  .replace(scriptSrc, "script-src 'self' 'unsafe-inline'")
                  .replace(connectSrc, "connect-src 'self' ws:");
              },
            },
          ]
        : []),
    ],
    worker: { format: "es" },
  },
}));
