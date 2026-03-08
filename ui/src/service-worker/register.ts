/**
 * Service worker registration for WASM mode.
 *
 * Only registers the service worker when VITE_WASM_MODE is enabled.
 * In server mode, the existing vite-plugin-pwa config handles SW lifecycle.
 */

export async function registerWasmServiceWorker(): Promise<void> {
  if (!("serviceWorker" in navigator)) {
    console.warn("Service workers not supported in this browser");
    return;
  }

  try {
    const registration = await navigator.serviceWorker.register("/sw.js", {
      type: "module",
      scope: "/",
    });

    console.log("Hadrian WASM service worker registered:", registration.scope);

    // Wait for the SW to be active (handles installing, waiting, or already active)
    const sw = registration.active || registration.waiting || registration.installing;
    if (sw && sw.state !== "activated") {
      await new Promise<void>((resolve) => {
        sw.addEventListener("statechange", function handler() {
          if (sw.state === "activated") {
            sw.removeEventListener("statechange", handler);
            resolve();
          }
        });
      });
    }

    // Ensure this page is controlled by the SW — even after activation,
    // the page may not be controlled until clients.claim() fires.
    // On hard refresh the activate event doesn't re-fire, so we ask the
    // SW to re-claim via postMessage and race against a timeout.
    if (!navigator.serviceWorker.controller) {
      const controllerReady = new Promise<void>((resolve) => {
        navigator.serviceWorker.addEventListener("controllerchange", () => resolve(), {
          once: true,
        });
      });

      // Ask the already-active SW to call clients.claim()
      registration.active?.postMessage({ type: "CLAIM" });

      await Promise.race([controllerReady, new Promise<void>((r) => setTimeout(r, 2000))]);
    }
  } catch (error) {
    console.error("Failed to register WASM service worker:", error);
  }
}

/**
 * Check if the WASM service worker is active and ready.
 */
export async function isWasmReady(): Promise<boolean> {
  if (!("serviceWorker" in navigator)) return false;

  const registration = await navigator.serviceWorker.getRegistration("/");
  return registration?.active != null;
}
