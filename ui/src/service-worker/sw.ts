/**
 * Hadrian WASM Service Worker
 *
 * Intercepts API requests and routes them through the WASM-compiled
 * Hadrian gateway running entirely in the browser.
 *
 * Intercepted paths:
 * - /v1/*          — OpenAI-compatible API endpoints
 * - /admin/v1/*    — Admin API endpoints
 * - /health        — Health check
 * - /api/*         — Other API endpoints
 */

/// <reference lib="webworker" />
declare const self: ServiceWorkerGlobalScope;

// Initialize the sql.js bridge BEFORE loading the WASM module.
// This registers globalThis.__hadrian_sqlite so the Rust FFI can use it.
import "./sqlite-bridge";

// Static import — dynamic import() is disallowed in service workers.
// The WASM module is served from public/wasm/ at runtime.
import wasmInit, { HadrianGateway } from "/wasm/hadrian.js";

let gateway: HadrianGateway | null = null;
let initPromise: Promise<void> | null = null;

// Path prefixes handled by the WASM gateway
const GATEWAY_PATHS = ["/v1/", "/admin/v1/", "/health", "/auth/", "/api/"];

async function ensureGateway(): Promise<void> {
  await wasmInit("/wasm/hadrian_bg.wasm");
  gateway = await new HadrianGateway();
}

self.addEventListener("install", (event) => {
  // Activate immediately, don't wait for existing clients to close
  event.waitUntil(self.skipWaiting());
});

self.addEventListener("activate", (event) => {
  // Take control of all clients immediately
  event.waitUntil(self.clients.claim());
});

self.addEventListener("fetch", (event) => {
  const url = new URL(event.request.url);

  // Only intercept gateway API paths on the same origin
  if (url.origin !== self.location.origin) return;
  if (!GATEWAY_PATHS.some((p) => url.pathname.startsWith(p))) return;

  event.respondWith(handleRequest(event.request));
});

async function handleRequest(request: Request): Promise<Response> {
  // Lazy-init the WASM gateway on first intercepted request
  if (!gateway) {
    if (!initPromise) {
      initPromise = ensureGateway();
    }
    try {
      await initPromise;
    } catch (error) {
      initPromise = null; // Allow retry on next request
      console.error("Failed to initialize Hadrian WASM gateway:", error);
      return new Response(
        JSON.stringify({
          error: {
            message: `Gateway initialization failed: ${String(error)}`,
            type: "server_error",
            code: 503,
          },
        }),
        {
          status: 503,
          headers: { "Content-Type": "application/json" },
        }
      );
    }
  }

  try {
    return await gateway!.handle(request);
  } catch (error) {
    console.error("Hadrian WASM gateway error:", error);
    return new Response(
      JSON.stringify({
        error: {
          message: String(error),
          type: "server_error",
          code: 500,
        },
      }),
      {
        status: 500,
        headers: { "Content-Type": "application/json" },
      }
    );
  }
}
