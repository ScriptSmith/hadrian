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

    // Wait for the service worker to be active before proceeding
    if (registration.installing) {
      await new Promise<void>((resolve) => {
        registration.installing!.addEventListener("statechange", (e) => {
          if ((e.target as ServiceWorker).state === "activated") {
            resolve();
          }
        });
      });
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
