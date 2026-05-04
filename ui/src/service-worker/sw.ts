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

import { formatApiError } from "../utils/formatApiError";
import {
  augmentModelsResponse,
  handleChatCompletionsRequest,
  handleResponsesRequest,
  invalidateAvailabilityCache,
  isBrowserAiModel,
  type ChatCompletionsPayload,
  type ResponsesPayload,
} from "./browser-ai";

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

// Allow clients to request re-claim (e.g. after hard refresh where
// the activate event doesn't fire again).
self.addEventListener("message", (event) => {
  if (event.data?.type === "CLAIM") {
    self.clients.claim();
  }
  if (event.data?.type === "BROWSER_AI_AVAILABILITY_CHANGED") {
    invalidateAvailabilityCache();
  }
});

self.addEventListener("fetch", (event) => {
  const url = new URL(event.request.url);

  // Only intercept gateway API paths on the same origin
  if (url.origin !== self.location.origin) return;
  if (!GATEWAY_PATHS.some((p) => url.pathname.startsWith(p))) return;

  event.respondWith(handleRequest(event.request, url, event.clientId));
});

async function handleRequest(request: Request, url: URL, clientId: string): Promise<Response> {
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
            message: `Gateway initialization failed: ${formatApiError(error)}`,
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
    const intercepted = await maybeHandleBrowserAi(request, url, clientId);
    if (intercepted) return intercepted;
    return await gateway!.handle(request);
  } catch (error) {
    console.error("Hadrian WASM gateway error:", error);
    return new Response(
      JSON.stringify({
        error: {
          message: formatApiError(error),
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

function isResponsesPath(pathname: string): boolean {
  return pathname.endsWith("/v1/responses");
}

function isChatCompletionsPath(pathname: string): boolean {
  return pathname.endsWith("/v1/chat/completions");
}

function isModelsPath(pathname: string): boolean {
  return pathname.endsWith("/v1/models");
}

async function maybeHandleBrowserAi(
  request: Request,
  url: URL,
  clientId: string
): Promise<Response | null> {
  if (request.method === "GET" && isModelsPath(url.pathname)) {
    const upstream = await gateway!.handle(request);
    return augmentModelsResponse(upstream, clientId);
  }

  if (request.method !== "POST") return null;
  if (!isResponsesPath(url.pathname) && !isChatCompletionsPath(url.pathname)) return null;

  let body: unknown;
  try {
    body = await request.clone().json();
  } catch {
    return null;
  }
  if (!body || typeof body !== "object") return null;
  const model = (body as { model?: unknown }).model;
  if (!isBrowserAiModel(model)) return null;

  if (isResponsesPath(url.pathname)) {
    return handleResponsesRequest(request, body as ResponsesPayload, clientId);
  }
  return handleChatCompletionsRequest(request, body as ChatCompletionsPayload, clientId);
}
