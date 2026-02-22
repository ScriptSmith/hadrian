/**
 * QuickJS Service
 *
 * Manages communication with the QuickJS Web Worker for executing JavaScript code.
 * Provides a simple async API for code execution with proper lifecycle management.
 *
 * ## Usage
 *
 * ```typescript
 * import { quickjsService } from "@/services/quickjs";
 *
 * // Execute JavaScript code
 * const result = await quickjsService.execute("console.log('Hello, World!')");
 *
 * // Execute with timeout
 * const result = await quickjsService.execute("while(true) {}", {
 *   timeout: 5000,
 * });
 * ```
 */

/** Result from JavaScript code execution */
export interface JSExecutionResult {
  /** Whether execution succeeded */
  success: boolean;
  /** Return value from the last expression (if any) */
  result?: unknown;
  /** Captured stdout output (console.log) */
  stdout: string;
  /** Captured stderr output (console.error, console.warn) */
  stderr: string;
  /** Error message if execution failed */
  error?: string;
}

/** Options for code execution */
export interface ExecuteOptions {
  /** Timeout in milliseconds (default: 30000) */
  timeout?: number;
  /** Abort signal for cancellation */
  signal?: AbortSignal;
}

/** QuickJS loading status */
export type QuickJSStatus = "idle" | "loading" | "ready" | "error";

/** Status update callback */
export type StatusCallback = (status: QuickJSStatus, message?: string) => void;

/** Internal message counter for correlation */
let messageId = 0;

/** Generate unique message ID */
function nextId(): string {
  return `qjs-${++messageId}-${Date.now()}`;
}

class QuickJSService {
  private worker: Worker | null = null;
  private pendingRequests = new Map<
    string,
    {
      resolve: (value: unknown) => void;
      reject: (error: Error) => void;
    }
  >();
  private status: QuickJSStatus = "idle";
  private statusListeners = new Set<StatusCallback>();
  private initPromise: Promise<void> | null = null;

  /**
   * Get the current QuickJS status
   */
  getStatus(): QuickJSStatus {
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
  private setStatus(status: QuickJSStatus, message?: string) {
    this.status = status;
    this.statusListeners.forEach((cb) => cb(status, message));
  }

  /**
   * Initialize the QuickJS worker
   */
  async init(): Promise<void> {
    if (this.worker) return;
    if (this.initPromise) return this.initPromise;

    this.initPromise = this.doInit();
    return this.initPromise;
  }

  private async doInit(): Promise<void> {
    this.setStatus("loading", "Initializing JavaScript runtime...");

    return new Promise((resolve, reject) => {
      try {
        // Create worker from the worker module
        this.worker = new Worker(new URL("./quickjsWorker.ts", import.meta.url), {
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
      throw new Error("QuickJS worker not initialized");
    }

    return new Promise<T>((resolve, reject) => {
      const id = message.id;

      // Set up timeout
      const timeout = options?.timeout ?? 30000;
      const timeoutId = setTimeout(() => {
        this.pendingRequests.delete(id);
        reject(new Error(`JavaScript execution timed out after ${timeout}ms`));
      }, timeout);

      // Set up abort handler
      if (options?.signal) {
        options.signal.addEventListener("abort", () => {
          clearTimeout(timeoutId);
          this.pendingRequests.delete(id);
          reject(new Error("JavaScript execution was cancelled"));
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
   * Execute JavaScript code
   */
  async execute(code: string, options?: ExecuteOptions): Promise<JSExecutionResult> {
    const id = nextId();

    const response = await this.sendMessage<{
      type: "executeResult";
      id: string;
      success: boolean;
      result?: unknown;
      stdout: string;
      stderr: string;
      error?: string;
    }>(
      {
        type: "execute",
        id,
        code,
        timeout: options?.timeout,
      },
      { timeout: options?.timeout, signal: options?.signal }
    );

    return {
      success: response.success,
      result: response.result,
      stdout: response.stdout,
      stderr: response.stderr,
      error: response.error,
    };
  }

  /**
   * Check if QuickJS is ready
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
    this.pendingRequests.clear();
    this.initPromise = null;
    this.setStatus("idle");
  }
}

// Export a singleton instance
export const quickjsService = new QuickJSService();

// Also export the class for testing
export { QuickJSService };
