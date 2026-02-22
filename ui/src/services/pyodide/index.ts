/**
 * Pyodide Service
 *
 * Provides in-browser Python execution using Pyodide (Python compiled to WebAssembly).
 *
 * ## Features
 *
 * - Lazy loading: Pyodide WASM is only downloaded when first needed
 * - Web Worker isolation: Python runs in a separate thread, preventing UI blocking
 * - Auto-import: Packages are automatically installed based on import statements
 * - Matplotlib support: Figures are captured as base64 PNG images
 * - Stdout/stderr capture: All output is captured and returned
 *
 * ## Supported Packages
 *
 * - numpy, pandas, scipy, matplotlib, scikit-learn (pre-built with C extensions)
 * - Pure Python packages via micropip
 *
 * ## Usage
 *
 * ```typescript
 * import { pyodideService } from "@/services/pyodide";
 *
 * // Execute Python code
 * const result = await pyodideService.execute(`
 *   import numpy as np
 *   x = np.array([1, 2, 3])
 *   print(x.sum())
 * `);
 *
 * console.log(result.stdout); // "6"
 * ```
 */

export {
  pyodideService,
  PyodideService,
  type PythonExecutionResult,
  type ExecuteOptions,
  type PyodideStatus,
  type StatusCallback,
} from "./pyodideService";
