import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "./index.css";
import App from "./App";

// In WASM mode, register the service worker that runs the gateway in-browser
if (import.meta.env.VITE_WASM_MODE === "true") {
  import("./service-worker/register").then(({ registerWasmServiceWorker }) =>
    registerWasmServiceWorker()
  );
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>
);
