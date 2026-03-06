/**
 * Type declarations for the Hadrian WASM module.
 * Generated types live in the wasm-pack output, but the service worker
 * needs declarations at build time.
 */

declare module "/wasm/hadrian.js" {
  export default function init(wasmUrl?: string | URL): Promise<WebAssembly.Instance>;

  export class HadrianGateway {
    /** The constructor is async (returns a Promise) via wasm-bindgen. */
    constructor();
    handle(request: Request): Promise<Response>;
    free(): void;
  }
}
