/**
 * DuckDB Service
 *
 * Provides in-browser SQL execution using DuckDB WASM.
 *
 * Note: SQLite support is NOT available in DuckDB-WASM due to extension limitations.
 *
 * ## Features
 *
 * - Lazy loading: DuckDB WASM is only downloaded when first needed
 * - Web Worker isolation: SQL runs in a separate thread, preventing UI blocking
 * - File support: Query CSV, Parquet, JSON files directly
 * - Full SQL support: DuckDB's complete SQL dialect with analytics functions
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
 * // Query a CSV file
 * const csvData = new TextEncoder().encode("name,value\nfoo,1\nbar,2");
 * await duckdbService.registerFile("data.csv", csvData.buffer, "csv");
 * const csvResult = await duckdbService.execute("SELECT * FROM 'data.csv'");
 * ```
 */

export {
  duckdbService,
  DuckDBService,
  type QueryResult,
  type ColumnInfo,
  type TableInfo,
  type TableColumnInfo,
  type ExecuteOptions,
  type DuckDBStatus,
  type StatusCallback,
  type FileType,
} from "./duckdbService";
