/**
 * QuickJS Service
 *
 * Provides in-browser JavaScript execution using QuickJS (compiled to WebAssembly).
 *
 * ## Features
 *
 * - Lazy loading: QuickJS WASM is only downloaded when first needed
 * - Web Worker isolation: JavaScript runs in a separate thread, preventing UI blocking
 * - Sandboxed execution: Code runs in an isolated context with no access to host APIs
 * - Console capture: console.log/error/warn are captured and returned
 * - Memory limits: 16MB memory limit per execution
 * - Timeout support: Configurable execution timeout (default 30 seconds)
 *
 * ## Usage
 *
 * ```typescript
 * import { quickjsService } from "@/services/quickjs";
 *
 * // Execute JavaScript code
 * const result = await quickjsService.execute(`
 *   const numbers = [1, 2, 3, 4, 5];
 *   const sum = numbers.reduce((a, b) => a + b, 0);
 *   console.log("Sum:", sum);
 *   sum;
 * `);
 *
 * console.log(result.stdout); // "Sum: 15"
 * console.log(result.result); // 15
 * ```
 */

export {
  quickjsService,
  QuickJSService,
  type JSExecutionResult,
  type ExecuteOptions,
  type QuickJSStatus,
  type StatusCallback,
} from "./quickjsService";
