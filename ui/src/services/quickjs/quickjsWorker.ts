/**
 * QuickJS Web Worker
 *
 * This worker loads and manages a QuickJS instance for executing JavaScript code
 * in a sandboxed environment. Running QuickJS in a worker prevents blocking
 * the main thread during computation.
 *
 * Communication protocol:
 * - Main thread sends { type, id, ... } messages
 * - Worker responds with { type, id, ... } messages
 * - Errors are sent as { type: "error", id, error: string }
 */

import { newQuickJSWASMModuleFromVariant } from "quickjs-emscripten-core";
import variant from "@jitl/quickjs-singlefile-browser-release-sync";
import type { QuickJSWASMModule, QuickJSContext } from "quickjs-emscripten-core";

/** Message types from main thread to worker */
interface ExecuteMessage {
  type: "execute";
  id: string;
  code: string;
  timeout?: number;
}

interface StatusMessage {
  type: "status";
  id: string;
}

type WorkerMessage = ExecuteMessage | StatusMessage;

/** Message types from worker to main thread */
interface ReadyResponse {
  type: "ready";
}

interface LoadingResponse {
  type: "loading";
  message: string;
}

interface ExecuteResponse {
  type: "executeResult";
  id: string;
  success: boolean;
  result?: unknown;
  stdout: string;
  stderr: string;
  error?: string;
}

interface StatusResponse {
  type: "statusResult";
  id: string;
  ready: boolean;
}

interface ErrorResponse {
  type: "error";
  id?: string;
  error: string;
}

type WorkerResponse =
  | ReadyResponse
  | LoadingResponse
  | ExecuteResponse
  | StatusResponse
  | ErrorResponse;

// Worker state
let quickjs: QuickJSWASMModule | null = null;
let isLoading = false;

/**
 * Send a message to the main thread
 */
function sendMessage(message: WorkerResponse) {
  self.postMessage(message);
}

/**
 * Initialize QuickJS using the singlefile variant (WASM embedded as base64)
 */
async function initQuickJS(): Promise<QuickJSWASMModule> {
  if (quickjs) return quickjs;
  if (isLoading) {
    // Wait for existing load
    while (isLoading) {
      await new Promise((resolve) => setTimeout(resolve, 100));
    }
    if (quickjs) return quickjs;
    throw new Error("QuickJS initialization failed");
  }

  isLoading = true;
  sendMessage({ type: "loading", message: "Loading JavaScript runtime..." });

  try {
    // Use singlefile variant - WASM is embedded as base64, no network request needed
    quickjs = await newQuickJSWASMModuleFromVariant(variant);
    sendMessage({ type: "ready" });
    return quickjs;
  } catch (error) {
    const errorMsg = error instanceof Error ? error.message : String(error);
    sendMessage({ type: "error", error: `Failed to load QuickJS: ${errorMsg}` });
    throw error;
  } finally {
    isLoading = false;
  }
}

/**
 * Execute JavaScript code in a sandboxed context
 */
async function executeCode(
  code: string,
  timeout?: number
): Promise<{
  success: boolean;
  result?: unknown;
  stdout: string;
  stderr: string;
  error?: string;
}> {
  const qjs = await initQuickJS();

  // Create a new context for isolation
  const vm: QuickJSContext = qjs.newContext();

  // Capture console output
  let stdout = "";
  let stderr = "";

  try {
    // Set up console.log
    const consoleLog = vm.newFunction("log", (...args) => {
      const output = args
        .map((arg) => {
          const str = vm.getString(arg);
          return str;
        })
        .join(" ");
      stdout += output + "\n";
    });

    // Set up console.error
    const consoleError = vm.newFunction("error", (...args) => {
      const output = args
        .map((arg) => {
          const str = vm.getString(arg);
          return str;
        })
        .join(" ");
      stderr += output + "\n";
    });

    // Set up console.warn (routes to stderr)
    const consoleWarn = vm.newFunction("warn", (...args) => {
      const output = args
        .map((arg) => {
          const str = vm.getString(arg);
          return str;
        })
        .join(" ");
      stderr += "[warn] " + output + "\n";
    });

    // Create console object
    const consoleObj = vm.newObject();
    vm.setProp(consoleObj, "log", consoleLog);
    vm.setProp(consoleObj, "error", consoleError);
    vm.setProp(consoleObj, "warn", consoleWarn);
    vm.setProp(vm.global, "console", consoleObj);

    // Clean up function handles
    consoleLog.dispose();
    consoleError.dispose();
    consoleWarn.dispose();
    consoleObj.dispose();

    // Set up interrupt handler for timeout
    const startTime = Date.now();
    const maxTime = timeout ?? 30000; // Default 30 seconds
    vm.runtime.setInterruptHandler(() => {
      return Date.now() - startTime > maxTime;
    });

    // Set memory limit (16MB)
    vm.runtime.setMemoryLimit(16 * 1024 * 1024);

    // Execute the code
    const result = vm.evalCode(code);

    if (result.error) {
      // Execution error
      const errorValue = vm.dump(result.error);
      result.error.dispose();
      return {
        success: false,
        stdout: stdout.trim(),
        stderr: stderr.trim(),
        error: String(errorValue),
      };
    }

    // Success - extract result value
    const value = vm.dump(result.value);
    result.value.dispose();

    return {
      success: true,
      result: value,
      stdout: stdout.trim(),
      stderr: stderr.trim(),
    };
  } catch (error) {
    const errorMsg = error instanceof Error ? error.message : String(error);
    return {
      success: false,
      stdout: stdout.trim(),
      stderr: stderr.trim(),
      error: errorMsg,
    };
  } finally {
    vm.dispose();
  }
}

/**
 * Handle messages from the main thread
 */
self.onmessage = async (event: MessageEvent<WorkerMessage>) => {
  const message = event.data;

  switch (message.type) {
    case "execute": {
      try {
        const result = await executeCode(message.code, message.timeout);
        sendMessage({
          type: "executeResult",
          id: message.id,
          ...result,
        });
      } catch (error) {
        const errorMsg = error instanceof Error ? error.message : String(error);
        sendMessage({
          type: "error",
          id: message.id,
          error: errorMsg,
        });
      }
      break;
    }

    case "status": {
      sendMessage({
        type: "statusResult",
        id: message.id,
        ready: quickjs !== null,
      });
      break;
    }

    default:
      sendMessage({
        type: "error",
        error: `Unknown message type: ${(message as { type: string }).type}`,
      });
  }
};

// Start loading QuickJS immediately
initQuickJS().catch(console.error);
