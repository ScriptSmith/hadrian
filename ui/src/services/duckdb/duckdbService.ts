/**
 * DuckDB Service
 *
 * Manages communication with the DuckDB Web Worker for executing SQL queries.
 * Provides a simple async API for database operations with proper lifecycle management.
 *
 * Note: SQLite support is NOT available in DuckDB-WASM due to extension limitations.
 *
 * ## Usage
 *
 * ```typescript
 * import { duckdbService } from "@/services/duckdb";
 *
 * // Execute a SQL query
 * const result = await duckdbService.execute("SELECT 1 + 1 AS answer");
 * console.log(result.rows); // [{ answer: 2 }]
 *
 * // Register a CSV file
 * const csvData = new TextEncoder().encode("a,b\n1,2\n3,4");
 * await duckdbService.registerFile("data.csv", csvData.buffer, "csv");
 *
 * // Query the CSV
 * const csvResult = await duckdbService.execute("SELECT * FROM 'data.csv'");
 * ```
 */

/** Column information from query results */
export interface ColumnInfo {
  name: string;
  type: string;
}

/** Result from SQL query execution */
export interface QueryResult {
  /** Whether execution succeeded */
  success: boolean;
  /** Column definitions */
  columns: ColumnInfo[];
  /** Query result rows */
  rows: Array<Record<string, unknown>>;
  /** Number of rows returned */
  rowCount: number;
  /** Error message if execution failed */
  error?: string;
}

/** Table information from schema */
export interface TableInfo {
  schema: string;
  name: string;
  type: string;
}

/** Column information from table schema */
export interface TableColumnInfo {
  name: string;
  type: string;
  nullable: boolean;
}

/** Options for query execution */
export interface ExecuteOptions {
  /** Timeout in milliseconds (default: 30000) */
  timeout?: number;
  /** Abort signal for cancellation */
  signal?: AbortSignal;
}

/** DuckDB loading status */
export type DuckDBStatus = "idle" | "loading" | "ready" | "error";

/** Status update callback */
export type StatusCallback = (status: DuckDBStatus, message?: string) => void;

/** File types supported for registration (SQLite NOT supported in WASM) */
export type FileType = "csv" | "parquet" | "json";

/** Internal message counter for correlation */
let messageId = 0;

/** Generate unique message ID */
function nextId(): string {
  return `ddb-${++messageId}-${Date.now()}`;
}

class DuckDBService {
  private worker: Worker | null = null;
  private pendingRequests = new Map<
    string,
    {
      resolve: (value: unknown) => void;
      reject: (error: Error) => void;
    }
  >();
  private status: DuckDBStatus = "idle";
  private statusListeners = new Set<StatusCallback>();
  private initPromise: Promise<void> | null = null;
  private registeredFiles = new Set<string>();

  /**
   * Get the current DuckDB status
   */
  getStatus(): DuckDBStatus {
    return this.status;
  }

  /**
   * Get list of registered files
   */
  getRegisteredFiles(): string[] {
    return Array.from(this.registeredFiles);
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
  private setStatus(status: DuckDBStatus, message?: string) {
    this.status = status;
    this.statusListeners.forEach((cb) => cb(status, message));
  }

  /**
   * Initialize the DuckDB worker
   */
  async init(): Promise<void> {
    if (this.worker && this.status === "ready") return;
    if (this.initPromise) return this.initPromise;

    this.initPromise = this.doInit();
    return this.initPromise;
  }

  private async doInit(): Promise<void> {
    this.setStatus("loading", "Initializing SQL engine...");

    return new Promise((resolve, reject) => {
      try {
        // Create worker from the worker module
        this.worker = new Worker(new URL("./duckdbWorker.ts", import.meta.url), {
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
            case "registerFileResult":
            case "unregisterFileResult":
            case "listTablesResult":
            case "describeTableResult":
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
    options?: { timeout?: number; signal?: AbortSignal },
    transferables?: Transferable[]
  ): Promise<T> {
    await this.init();

    if (!this.worker) {
      throw new Error("DuckDB worker not initialized");
    }

    return new Promise<T>((resolve, reject) => {
      const id = message.id;

      // Set up timeout
      const timeout = options?.timeout ?? 30000;
      const timeoutId = setTimeout(() => {
        this.pendingRequests.delete(id);
        reject(new Error(`SQL execution timed out after ${timeout}ms`));
      }, timeout);

      // Set up abort handler
      if (options?.signal) {
        options.signal.addEventListener("abort", () => {
          clearTimeout(timeoutId);
          this.pendingRequests.delete(id);
          reject(new Error("SQL execution was cancelled"));
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

      // Send the message (with transferables if provided)
      if (transferables && transferables.length > 0) {
        this.worker!.postMessage(message, transferables);
      } else {
        this.worker!.postMessage(message);
      }
    });
  }

  /**
   * Execute a SQL query
   */
  async execute(sql: string, options?: ExecuteOptions): Promise<QueryResult> {
    const id = nextId();

    const response = await this.sendMessage<{
      type: "executeResult";
      id: string;
      success: boolean;
      columns: ColumnInfo[];
      rows: Array<Record<string, unknown>>;
      rowCount: number;
      error?: string;
    }>(
      {
        type: "execute",
        id,
        sql,
      },
      { timeout: options?.timeout, signal: options?.signal }
    );

    return {
      success: response.success,
      columns: response.columns,
      rows: response.rows,
      rowCount: response.rowCount,
      error: response.error,
    };
  }

  /**
   * Register a file in DuckDB's virtual filesystem
   *
   * @param name - Filename to register (e.g., "data.csv", "data.parquet")
   * @param data - File data as ArrayBuffer
   * @param fileType - Type of file for proper handling
   */
  async registerFile(
    name: string,
    data: ArrayBuffer,
    fileType: FileType,
    options?: ExecuteOptions
  ): Promise<{ success: boolean; error?: string }> {
    const id = nextId();

    const response = await this.sendMessage<{
      type: "registerFileResult";
      id: string;
      success: boolean;
      error?: string;
    }>(
      {
        type: "registerFile",
        id,
        name,
        data,
        fileType,
      },
      { timeout: options?.timeout ?? 60000, signal: options?.signal },
      [data] // Transfer the ArrayBuffer for efficiency
    );

    if (response.success) {
      this.registeredFiles.add(name);
    }

    return {
      success: response.success,
      error: response.error,
    };
  }

  /**
   * Unregister a file from DuckDB's virtual filesystem
   */
  async unregisterFile(
    name: string,
    options?: ExecuteOptions
  ): Promise<{ success: boolean; error?: string }> {
    const id = nextId();

    const response = await this.sendMessage<{
      type: "unregisterFileResult";
      id: string;
      success: boolean;
      error?: string;
    }>(
      {
        type: "unregisterFile",
        id,
        name,
      },
      { timeout: options?.timeout, signal: options?.signal }
    );

    if (response.success) {
      this.registeredFiles.delete(name);
    }

    return {
      success: response.success,
      error: response.error,
    };
  }

  /**
   * List all tables in the database
   */
  async listTables(options?: ExecuteOptions): Promise<{
    success: boolean;
    tables: TableInfo[];
    error?: string;
  }> {
    const id = nextId();

    const response = await this.sendMessage<{
      type: "listTablesResult";
      id: string;
      success: boolean;
      tables: TableInfo[];
      error?: string;
    }>(
      {
        type: "listTables",
        id,
      },
      { timeout: options?.timeout, signal: options?.signal }
    );

    return {
      success: response.success,
      tables: response.tables,
      error: response.error,
    };
  }

  /**
   * Describe a table's schema (columns and types)
   *
   * @param tableName - Table name or file path (e.g., "'data.csv'")
   */
  async describeTable(
    tableName: string,
    options?: ExecuteOptions
  ): Promise<{
    success: boolean;
    columns: TableColumnInfo[];
    error?: string;
  }> {
    const id = nextId();

    const response = await this.sendMessage<{
      type: "describeTableResult";
      id: string;
      success: boolean;
      columns: TableColumnInfo[];
      error?: string;
    }>(
      {
        type: "describeTable",
        id,
        tableName,
      },
      { timeout: options?.timeout, signal: options?.signal }
    );

    return {
      success: response.success,
      columns: response.columns,
      error: response.error,
    };
  }

  /**
   * Check if DuckDB is ready
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
    this.registeredFiles.clear();
    this.initPromise = null;
    this.setStatus("idle");
  }
}

// Export a singleton instance
export const duckdbService = new DuckDBService();

// Also export the class for testing
export { DuckDBService };
