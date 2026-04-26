import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "./index.css";
import App from "./App";
import { handleMCPOAuthCallback } from "./services/mcp/oauth";

// Intercept MCP OAuth popup callbacks before rendering the full app.
// If this page is a popup returning from an OAuth authorization server,
// forward the code+state to the opener window and close.
if (handleMCPOAuthCallback()) {
  document.getElementById("root")!.textContent =
    "Authorization complete. You may close this window.";
} else {
  bootstrap();
}

async function bootstrap() {
  // In WASM mode, register the service worker and wait for it to control the
  // page before rendering. `serviceWorker.ready` resolves once a SW with a
  // scope covering this page is *active*, which closes the hard-refresh race
  // where API calls fired before the WASM gateway was reachable.
  if (import.meta.env.VITE_WASM_MODE === "true") {
    const { registerWasmServiceWorker } = await import("./service-worker/register");
    await registerWasmServiceWorker();
    if ("serviceWorker" in navigator) {
      await navigator.serviceWorker.ready;
    }
  } else if ("serviceWorker" in navigator) {
    // Only unregister service workers we recognise as ours. The previous
    // implementation called `unregister()` on every registration, which
    // tore down legitimate third-party service workers if the gateway was
    // installed on a shared origin. The Hadrian WASM SW always lives at
    // `/sw.js` (see `service-worker/register.ts`); leave anything else
    // alone.
    const registrations = await navigator.serviceWorker.getRegistrations();
    await Promise.all(
      registrations
        .filter((r) => {
          const sw = r.active ?? r.waiting ?? r.installing;
          if (!sw) return false;
          try {
            return new URL(sw.scriptURL).pathname === "/sw.js";
          } catch {
            return false;
          }
        })
        .map((r) => r.unregister())
    );
  }

  createRoot(document.getElementById("root")!).render(
    <StrictMode>
      <App />
    </StrictMode>
  );
}
