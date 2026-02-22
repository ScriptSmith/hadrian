/**
 * Pyodide Service
 *
 * Manages communication with the Pyodide Web Worker for executing Python code.
 * Provides a simple async API for code execution with proper lifecycle management.
 *
 * ## Usage
 *
 * ```typescript
 * import { pyodideService } from "@/services/pyodide";
 *
 * // Execute Python code
 * const result = await pyodideService.execute("print('Hello, World!')");
 *
 * // Execute with timeout
 * const result = await pyodideService.execute("import time; time.sleep(1)", {
 *   timeout: 5000,
 * });
 * ```
 */

/** Result from Python code execution */
export interface PythonExecutionResult {
  /** Whether execution succeeded */
  success: boolean;
  /** Return value from the last expression (if any) */
  result?: unknown;
  /** Captured stdout output */
  stdout: string;
  /** Captured stderr output */
  stderr: string;
  /** Base64-encoded PNG images from matplotlib figures */
  figures: string[];
  /** Error message if execution failed */
  error?: string;
}

/** Options for code execution */
export interface ExecuteOptions {
  /** Timeout in milliseconds (default: 30000) */
  timeout?: number;
  /** Packages to pre-load before execution */
  packages?: string[];
  /** Abort signal for cancellation */
  signal?: AbortSignal;
}

/** Pyodide loading status */
export type PyodideStatus = "idle" | "loading" | "ready" | "error";

/** Status update callback */
export type StatusCallback = (status: PyodideStatus, message?: string) => void;

/** Internal message counter for correlation */
let messageId = 0;

/** Generate unique message ID */
function nextId(): string {
  return `msg-${++messageId}-${Date.now()}`;
}

/**
 * Execution queue item for serializing Python execution.
 *
 * Pyodide uses a single interpreter instance, and matplotlib figures are global.
 * When multiple execute() calls run concurrently, their async handlers interleave
 * at await points, causing figure capture to grab figures from other executions.
 * This queue ensures only one execution runs at a time.
 */
interface QueuedExecution {
  code: string;
  options?: ExecuteOptions;
  resolve: (result: PythonExecutionResult) => void;
  reject: (error: Error) => void;
}

class PyodideService {
  private worker: Worker | null = null;
  private pendingRequests = new Map<
    string,
    {
      resolve: (value: unknown) => void;
      reject: (error: Error) => void;
    }
  >();
  private status: PyodideStatus = "idle";
  private statusListeners = new Set<StatusCallback>();
  private initPromise: Promise<void> | null = null;

  /** Queue for serializing code executions */
  private executionQueue: QueuedExecution[] = [];
  /** Whether an execution is currently in progress */
  private isExecuting = false;

  /**
   * Get the current Pyodide status
   */
  getStatus(): PyodideStatus {
    return this.status;
  }

  /**
   * Subscribe to status changes
   */
  onStatusChange(callback: StatusCallback): () => void {
    this.statusListeners.add(callback);
    return () => this.statusListeners.delete(callback);
  }

  /**
   * Notify all status listeners
   */
  private setStatus(status: PyodideStatus, message?: string) {
    this.status = status;
    this.statusListeners.forEach((cb) => cb(status, message));
  }

  /**
   * Initialize the Pyodide worker
   */
  async init(): Promise<void> {
    if (this.worker) return;
    if (this.initPromise) return this.initPromise;

    this.initPromise = this.doInit();
    return this.initPromise;
  }

  private async doInit(): Promise<void> {
    this.setStatus("loading", "Initializing Python runtime...");

    return new Promise((resolve, reject) => {
      try {
        // Create worker from the worker module
        this.worker = new Worker(new URL("./pyodideWorker.ts", import.meta.url), {
          type: "module",
        });

        // Handle messages from worker
        this.worker.onmessage = (event) => {
          const message = event.data;

          switch (message.type) {
            case "ready":
              this.setStatus("ready");
              resolve();
              break;

            case "loading":
              this.setStatus("loading", message.message);
              break;

            case "executeResult":
            case "packagesLoaded":
            case "statusResult": {
              const pending = this.pendingRequests.get(message.id);
              if (pending) {
                this.pendingRequests.delete(message.id);
                pending.resolve(message);
              }
              break;
            }

            case "error": {
              if (message.id) {
                const pending = this.pendingRequests.get(message.id);
                if (pending) {
                  this.pendingRequests.delete(message.id);
                  pending.reject(new Error(message.error));
                }
              } else {
                // Global error during initialization
                this.setStatus("error", message.error);
                reject(new Error(message.error));
              }
              break;
            }
          }
        };

        this.worker.onerror = (error) => {
          this.setStatus("error", error.message);
          reject(new Error(error.message));
        };
      } catch (error) {
        const errorMsg = error instanceof Error ? error.message : String(error);
        this.setStatus("error", errorMsg);
        reject(error);
      }
    });
  }

  /**
   * Send a message to the worker and wait for response
   */
  private async sendMessage<T>(
    message: { type: string; id: string; [key: string]: unknown },
    options?: { timeout?: number; signal?: AbortSignal }
  ): Promise<T> {
    await this.init();

    if (!this.worker) {
      throw new Error("Pyodide worker not initialized");
    }

    return new Promise<T>((resolve, reject) => {
      const id = message.id;

      // Set up timeout
      const timeout = options?.timeout ?? 30000;
      const timeoutId = setTimeout(() => {
        this.pendingRequests.delete(id);
        reject(new Error(`Python execution timed out after ${timeout}ms`));
      }, timeout);

      // Set up abort handler
      if (options?.signal) {
        options.signal.addEventListener("abort", () => {
          clearTimeout(timeoutId);
          this.pendingRequests.delete(id);
          reject(new Error("Python execution was cancelled"));
        });
      }

      // Store the pending request
      this.pendingRequests.set(id, {
        resolve: (value) => {
          clearTimeout(timeoutId);
          resolve(value as T);
        },
        reject: (error) => {
          clearTimeout(timeoutId);
          reject(error);
        },
      });

      // Send the message
      this.worker!.postMessage(message);
    });
  }

  /**
   * Execute Python code.
   *
   * Executions are serialized to prevent race conditions with matplotlib figures.
   * Parallel calls will queue and execute sequentially.
   */
  async execute(code: string, options?: ExecuteOptions): Promise<PythonExecutionResult> {
    return new Promise((resolve, reject) => {
      // Handle abort signal
      if (options?.signal?.aborted) {
        reject(new Error("Python execution was cancelled"));
        return;
      }

      const queueItem: QueuedExecution = { code, options, resolve, reject };

      // If signal is provided, remove from queue if aborted
      if (options?.signal) {
        options.signal.addEventListener("abort", () => {
          const index = this.executionQueue.indexOf(queueItem);
          if (index !== -1) {
            this.executionQueue.splice(index, 1);
            reject(new Error("Python execution was cancelled"));
          }
        });
      }

      this.executionQueue.push(queueItem);
      this.processQueue();
    });
  }

  /**
   * Process the execution queue, running one item at a time.
   */
  private async processQueue(): Promise<void> {
    if (this.isExecuting || this.executionQueue.length === 0) {
      return;
    }

    this.isExecuting = true;

    while (this.executionQueue.length > 0) {
      const item = this.executionQueue.shift()!;

      try {
        const result = await this.executeInternal(item.code, item.options);
        item.resolve(result);
      } catch (error) {
        item.reject(error instanceof Error ? error : new Error(String(error)));
      }
    }

    this.isExecuting = false;
  }

  /**
   * Internal execution method - sends to worker and waits for result.
   */
  private async executeInternal(
    code: string,
    options?: ExecuteOptions
  ): Promise<PythonExecutionResult> {
    const id = nextId();

    const response = await this.sendMessage<{
      type: "executeResult";
      id: string;
      success: boolean;
      result?: unknown;
      stdout: string;
      stderr: string;
      figures: string[];
      error?: string;
    }>(
      {
        type: "execute",
        id,
        code,
        packages: options?.packages,
      },
      { timeout: options?.timeout, signal: options?.signal }
    );

    return {
      success: response.success,
      result: response.result,
      stdout: response.stdout,
      stderr: response.stderr,
      figures: response.figures,
      error: response.error,
    };
  }

  /**
   * Pre-load Python packages
   */
  async loadPackages(packages: string[]): Promise<void> {
    const id = nextId();

    await this.sendMessage<{ type: "packagesLoaded"; id: string; packages: string[] }>({
      type: "loadPackages",
      id,
      packages,
    });
  }

  /**
   * Check if Pyodide is ready
   */
  isReady(): boolean {
    return this.status === "ready";
  }

  /**
   * Terminate the worker and clean up
   */
  terminate(): void {
    if (this.worker) {
      this.worker.terminate();
      this.worker = null;
    }

    // Reject all queued executions
    for (const item of this.executionQueue) {
      item.reject(new Error("Pyodide worker was terminated"));
    }
    this.executionQueue = [];
    this.isExecuting = false;

    this.pendingRequests.clear();
    this.initPromise = null;
    this.setStatus("idle");
  }
}

// Export a singleton instance
export const pyodideService = new PyodideService();

// Also export the class for testing
export { PyodideService };
