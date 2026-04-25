/// <reference types="vitest/config" />
import { defineConfig, type Plugin } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import path from "path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { createRequire } from "node:module";
import { storybookTest } from "@storybook/addon-vitest/vitest-plugin";
import { playwright } from "@vitest/browser-playwright";
import { VitePWA } from "vite-plugin-pwa";
const dirname =
  typeof __dirname !== "undefined"
    ? __dirname
    : path.dirname(fileURLToPath(import.meta.url));

const isWasmMode = process.env.VITE_WASM_MODE === "true";

/**
 * Builds and serves the WASM service worker.
 *
 * Dev: intercepts /sw.js requests and transforms the TS source on the fly.
 * Build: compiles sw.ts with esbuild (separate from rollup) so the output
 *        is a standalone file without Vite's preload helpers.
 */
function wasmServiceWorkerPlugin(): Plugin {
  const swPath = path.resolve(__dirname, "src/service-worker/sw.ts");

  function getEsbuild() {
    const req = createRequire(pathToFileURL(__filename).href);
    return req("esbuild") as {
      transform: Function;
      build: Function;
    };
  }

  return {
    name: "hadrian-wasm-sw",
    configureServer(server) {
      // Must run before Vite's SPA fallback, which would serve index.html
      server.middlewares.use(async (req, res, next) => {
        if (req.url !== "/sw.js") return next();
        const { build } = getEsbuild();
        const os = await import("node:os");
        const outfile = path.join(os.tmpdir(), "hadrian-sw-dev.js");
        try {
          await build({
            entryPoints: [swPath],
            outfile,
            bundle: true,
            format: "esm",
            target: "es2022",
            write: true,
            // /wasm/hadrian.js is a runtime import served by the browser
            external: ["/wasm/hadrian.js"],
          });
          const fs = await import("node:fs/promises");
          const code = await fs.readFile(outfile, "utf-8");
          res.setHeader("Content-Type", "application/javascript; charset=utf-8");
          res.setHeader("Cache-Control", "no-store");
          res.end(code);
        } catch (err) {
          console.error("Failed to compile service worker:", err);
          next(err);
        }
      });
    },
    async writeBundle() {
      const { build } = getEsbuild();
      await build({
        entryPoints: [swPath],
        outfile: path.resolve(__dirname, "dist/sw.js"),
        bundle: true,
        format: "esm",
        target: "es2022",
        sourcemap: true,
        // /wasm/hadrian.js is a runtime import served by the browser
        external: ["/wasm/hadrian.js"],
      });
    },
  };
}

// More info at: https://storybook.js.org/docs/next/writing-tests/integrations/vitest-addon
export default defineConfig({
  plugins: [
    react(),
    tailwindcss(),
    // In WASM mode, compile and serve the service worker; otherwise use vite-plugin-pwa
    ...(isWasmMode ? [wasmServiceWorkerPlugin()] : []),
    ...(!isWasmMode
      ? [
          VitePWA({
            selfDestroying: true,
            includeAssets: ["favicon.ico", "icons/*.png"],
            manifest: {
              name: "Hadrian Gateway",
              short_name: "Hadrian",
              description: "AI Gateway - Chat & Admin Dashboard",
              theme_color: "#3b82f6",
              background_color: "#0f172a",
              display: "standalone",
              start_url: "/",
              icons: [
                {
                  src: "/icons/icon-72.png",
                  sizes: "72x72",
                  type: "image/png",
                },
                {
                  src: "/icons/icon-96.png",
                  sizes: "96x96",
                  type: "image/png",
                },
                {
                  src: "/icons/icon-128.png",
                  sizes: "128x128",
                  type: "image/png",
                },
                {
                  src: "/icons/icon-144.png",
                  sizes: "144x144",
                  type: "image/png",
                },
                {
                  src: "/icons/icon-152.png",
                  sizes: "152x152",
                  type: "image/png",
                },
                {
                  src: "/icons/icon-192.png",
                  sizes: "192x192",
                  type: "image/png",
                },
                {
                  src: "/icons/icon-384.png",
                  sizes: "384x384",
                  type: "image/png",
                },
                {
                  src: "/icons/icon-512.png",
                  sizes: "512x512",
                  type: "image/png",
                  purpose: "any maskable",
                },
              ],
            },
          }),
        ]
      : []),
  ],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  server: {
    port: 5173,
    // In WASM mode, the service worker handles API routing — no proxy needed
    proxy: isWasmMode
      ? undefined
      : {
          "/api/": {
            target: "http://localhost:8080",
            changeOrigin: true,
          },
          "/admin/v1": {
            target: "http://localhost:8080",
            changeOrigin: true,
          },
          "/auth": {
            target: "http://localhost:8080",
            changeOrigin: true,
          },
          // PKCE token-exchange endpoint. Only `/oauth/token` is proxied —
          // `/oauth/authorize` is the in-app consent page handled client-side
          // by the React router.
          "/oauth/token": {
            target: "http://localhost:8080",
            changeOrigin: true,
          },
          // RFC 8414 Authorization Server Metadata.
          "/.well-known/oauth-authorization-server": {
            target: "http://localhost:8080",
            changeOrigin: true,
          },
        },
  },
  worker: {
    format: "es",
  },
  build: {
    sourcemap: true,
    rollupOptions: {
      output: {
        manualChunks: {
          vendor: ["react", "react-dom", "react-router-dom"],
          query: ["@tanstack/react-query"],
          table: ["@tanstack/react-table"],
        },
      },
    },
  },
  test: {
    projects: [
      {
        extends: true,
        plugins: [
          storybookTest({
            configDir: path.join(dirname, ".storybook"),
          }),
        ],
        test: {
          name: "storybook",
          browser: {
            enabled: true,
            headless: true,
            provider: playwright({}),
            instances: [
              {
                browser: "chromium",
              },
            ],
          },
          setupFiles: [".storybook/vitest.setup.ts"],
        },
      },
      {
        extends: true,
        plugins: [
          storybookTest({
            configDir: path.join(dirname, ".storybook"),
          }),
        ],
        test: {
          name: "storybook-dark",
          browser: {
            enabled: true,
            headless: true,
            provider: playwright({}),
            instances: [
              {
                browser: "chromium",
              },
            ],
          },
          setupFiles: [".storybook/vitest.setup.dark.ts"],
        },
      },
      {
        extends: true,
        test: {
          name: "unit",
          environment: "node",
          include: ["src/**/__tests__/**/*.test.ts"],
        },
      },
    ],
  },
});
