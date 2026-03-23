import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "./index.css";
import App from "./App";

async function bootstrap() {
  // In WASM mode, register the service worker and wait for it to control the
  // page before rendering.  This prevents API calls from firing before the SW
  // is active (race condition on hard refresh).
  if (import.meta.env.VITE_WASM_MODE === "true") {
    const { registerWasmServiceWorker } = await import("./service-worker/register");
    await registerWasmServiceWorker();
  } else if ("serviceWorker" in navigator) {
    // Unregister any lingering WASM service workers so they don't intercept
    // requests when running the normal dev server.
    const registrations = await navigator.serviceWorker.getRegistrations();
    await Promise.all(registrations.map((r) => r.unregister()));
  }

  createRoot(document.getElementById("root")!).render(
    <StrictMode>
      <App />
    </StrictMode>
  );
}

bootstrap();
