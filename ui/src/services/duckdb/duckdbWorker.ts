/**
 * DuckDB Web Worker
 *
 * This worker loads and manages a DuckDB WASM instance for executing SQL queries
 * in-browser. Supports CSV, Parquet, and JSON files via the virtual filesystem.
 *
 * Note: SQLite support is NOT available in DuckDB-WASM due to extension limitations.
 *
 * Communication protocol:
 * - Main thread sends { type, id, ... } messages
 * - Worker responds with { type, id, ... } messages
 * - Errors are sent as { type: "error", id, error: string }
 */

import * as duckdb from "@duckdb/duckdb-wasm";

/** Message types from main thread to worker */
interface ExecuteMessage {
  type: "execute";
  id: string;
  sql: string;
}

interface RegisterFileMessage {
  type: "registerFile";
  id: string;
  name: string;
  data: ArrayBuffer;
  fileType: "csv" | "parquet" | "json";
}

interface UnregisterFileMessage {
  type: "unregisterFile";
  id: string;
  name: string;
}

interface ListTablesMessage {
  type: "listTables";
  id: string;
}

interface DescribeTableMessage {
  type: "describeTable";
  id: string;
  tableName: string;
}

interface StatusMessage {
  type: "status";
  id: string;
}

type WorkerMessage =
  | ExecuteMessage
  | RegisterFileMessage
  | UnregisterFileMessage
  | ListTablesMessage
  | DescribeTableMessage
  | StatusMessage;

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
  columns: Array<{ name: string; type: string }>;
  rows: Array<Record<string, unknown>>;
  rowCount: number;
  error?: string;
}

interface RegisterFileResponse {
  type: "registerFileResult";
  id: string;
  success: boolean;
  error?: string;
}

interface UnregisterFileResponse {
  type: "unregisterFileResult";
  id: string;
  success: boolean;
  error?: string;
}

interface ListTablesResponse {
  type: "listTablesResult";
  id: string;
  success: boolean;
  tables: Array<{ schema: string; name: string; type: string }>;
  error?: string;
}

interface DescribeTableResponse {
  type: "describeTableResult";
  id: string;
  success: boolean;
  columns: Array<{ name: string; type: string; nullable: boolean }>;
  error?: string;
}

interface StatusResponse {
  type: "statusResult";
  id: string;
  ready: boolean;
  registeredFiles: string[];
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
  | RegisterFileResponse
  | UnregisterFileResponse
  | ListTablesResponse
  | DescribeTableResponse
  | StatusResponse
  | ErrorResponse;

// Worker state
let db: duckdb.AsyncDuckDB | null = null;
let conn: duckdb.AsyncDuckDBConnection | null = null;
let isLoading = false;
const registeredFiles = new Set<string>();

/**
 * Send a message to the main thread
 */
function sendMessage(message: WorkerResponse) {
  self.postMessage(message);
}

/**
 * Initialize DuckDB WASM
 */
async function initDuckDB(): Promise<duckdb.AsyncDuckDB> {
  if (db) return db;
  if (isLoading) {
    // Wait for existing load
    while (isLoading) {
      await new Promise((resolve) => setTimeout(resolve, 100));
    }
    if (db) return db;
    throw new Error("DuckDB initialization failed");
  }

  isLoading = true;
  sendMessage({ type: "loading", message: "Loading DuckDB WASM..." });

  try {
    // Select the best bundle for the current browser
    const JSDELIVR_BUNDLES = duckdb.getJsDelivrBundles();
    const bundle = await duckdb.selectBundle(JSDELIVR_BUNDLES);

    sendMessage({ type: "loading", message: "Initializing database..." });

    // Create worker URL for DuckDB's internal worker
    const workerUrl = URL.createObjectURL(
      new Blob([`importScripts("${bundle.mainWorker!}");`], { type: "text/javascript" })
    );

    // Initialize DuckDB
    const worker = new Worker(workerUrl);
    const logger = new duckdb.ConsoleLogger(duckdb.LogLevel.WARNING);
    db = new duckdb.AsyncDuckDB(logger, worker);
    await db.instantiate(bundle.mainModule, bundle.pthreadWorker);

    // Clean up the blob URL
    URL.revokeObjectURL(workerUrl);

    // Create a connection
    conn = await db.connect();

    sendMessage({ type: "ready" });
    return db;
  } catch (error) {
    const errorMsg = error instanceof Error ? error.message : String(error);
    sendMessage({ type: "error", error: `Failed to load DuckDB: ${errorMsg}` });
    throw error;
  } finally {
    isLoading = false;
  }
}

/**
 * Execute a SQL query and return results
 */
async function executeQuery(sql: string): Promise<{
  success: boolean;
  columns: Array<{ name: string; type: string }>;
  rows: Array<Record<string, unknown>>;
  rowCount: number;
  error?: string;
}> {
  await initDuckDB();

  if (!conn) {
    throw new Error("No database connection");
  }

  try {
    const result = await conn.query(sql);

    // Extract column info from Arrow schema
    const columns = result.schema.fields.map((field) => ({
      name: field.name,
      type: String(field.type),
    }));

    // Convert Arrow table to plain JS objects
    const rows: Array<Record<string, unknown>> = [];
    for (const row of result) {
      const rowObj: Record<string, unknown> = {};
      for (const field of result.schema.fields) {
        const value = row[field.name];
        // Convert BigInt to number for JSON compatibility
        if (typeof value === "bigint") {
          rowObj[field.name] = Number(value);
        } else {
          rowObj[field.name] = value;
        }
      }
      rows.push(rowObj);
    }

    return {
      success: true,
      columns,
      rows,
      rowCount: rows.length,
    };
  } catch (error) {
    const errorMsg = error instanceof Error ? error.message : String(error);
    return {
      success: false,
      columns: [],
      rows: [],
      rowCount: 0,
      error: errorMsg,
    };
  }
}

/**
 * Register a file in DuckDB's virtual filesystem
 */
async function registerFile(
  name: string,
  data: ArrayBuffer,
  _fileType: "csv" | "parquet" | "json"
): Promise<{ success: boolean; error?: string }> {
  await initDuckDB();

  if (!db) {
    return { success: false, error: "Database not initialized" };
  }

  try {
    const dataSize = data.byteLength;

    if (dataSize === 0) {
      return { success: false, error: "File data is empty" };
    }

    // Register the file buffer
    await db.registerFileBuffer(name, new Uint8Array(data));
    registeredFiles.add(name);

    return { success: true };
  } catch (error) {
    const errorMsg = error instanceof Error ? error.message : String(error);
    return { success: false, error: errorMsg };
  }
}

/**
 * Unregister a file from DuckDB's virtual filesystem
 */
async function unregisterFile(name: string): Promise<{ success: boolean; error?: string }> {
  if (!db) {
    return { success: false, error: "Database not initialized" };
  }

  try {
    await db.dropFile(name);
    registeredFiles.delete(name);
    return { success: true };
  } catch (error) {
    const errorMsg = error instanceof Error ? error.message : String(error);
    return { success: false, error: errorMsg };
  }
}

/**
 * List all tables in the database
 */
async function listTables(): Promise<{
  success: boolean;
  tables: Array<{ schema: string; name: string; type: string }>;
  error?: string;
}> {
  await initDuckDB();

  if (!conn) {
    return { success: false, tables: [], error: "No database connection" };
  }

  try {
    const result = await conn.query(`
      SELECT table_schema, table_name, table_type
      FROM information_schema.tables
      WHERE table_schema NOT IN ('information_schema', 'pg_catalog')
      ORDER BY table_schema, table_name
    `);

    const tables: Array<{ schema: string; name: string; type: string }> = [];
    for (const row of result) {
      tables.push({
        schema: String(row.table_schema),
        name: String(row.table_name),
        type: String(row.table_type),
      });
    }

    return { success: true, tables };
  } catch (error) {
    const errorMsg = error instanceof Error ? error.message : String(error);
    return { success: false, tables: [], error: errorMsg };
  }
}

/**
 * Describe a table's schema (columns and types)
 *
 * Uses DESCRIBE SELECT for files (e.g., 'data.csv') or information_schema for tables.
 */
async function describeTable(tableName: string): Promise<{
  success: boolean;
  columns: Array<{ name: string; type: string; nullable: boolean }>;
  error?: string;
}> {
  await initDuckDB();

  if (!conn) {
    return { success: false, columns: [], error: "No database connection" };
  }

  try {
    // For files (quoted names like 'data.csv'), use DESCRIBE SELECT
    // For tables, we can also use DESCRIBE which works for both
    const result = await conn.query(`DESCRIBE SELECT * FROM ${tableName}`);

    const columns: Array<{ name: string; type: string; nullable: boolean }> = [];
    for (const row of result) {
      columns.push({
        name: String(row.column_name),
        type: String(row.column_type),
        // DESCRIBE doesn't provide nullable info directly, default to true
        nullable: true,
      });
    }

    return { success: true, columns };
  } catch (error) {
    const errorMsg = error instanceof Error ? error.message : String(error);
    return { success: false, columns: [], error: errorMsg };
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
        const result = await executeQuery(message.sql);
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

    case "registerFile": {
      try {
        const result = await registerFile(message.name, message.data, message.fileType);
        sendMessage({
          type: "registerFileResult",
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

    case "unregisterFile": {
      try {
        const result = await unregisterFile(message.name);
        sendMessage({
          type: "unregisterFileResult",
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

    case "listTables": {
      try {
        const result = await listTables();
        sendMessage({
          type: "listTablesResult",
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

    case "describeTable": {
      try {
        const result = await describeTable(message.tableName);
        sendMessage({
          type: "describeTableResult",
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
        ready: db !== null && conn !== null,
        registeredFiles: Array.from(registeredFiles),
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

// Start loading DuckDB immediately
initDuckDB().catch(console.error);
