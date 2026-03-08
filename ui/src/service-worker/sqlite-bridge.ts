/**
 * sql.js bridge for the Hadrian WASM gateway.
 *
 * Implements `globalThis.__hadrian_sqlite` with three methods that the Rust
 * WASM FFI bridge (`src/db/wasm_sqlite/bridge.rs`) calls via `wasm_bindgen`:
 *
 * - `init_database()` — load sql.js WASM and create an in-memory database
 * - `query(sql, params)` — run a SELECT, return `Array<Record<string, unknown>>`
 * - `execute(sql, params)` — run INSERT/UPDATE/DELETE, return `{ changes, last_insert_rowid }`
 *
 * The database is persisted to IndexedDB so state survives hard refreshes.
 *
 * This must be imported *before* the Hadrian WASM module so the bridge exists
 * when the Rust constructor calls `init_database()`.
 */

// sql.js ships a factory function that loads its own WASM binary.
// We import the ESM build and point it at the WASM file we serve from /wasm/.
import initSqlJs, { type Database } from "sql.js";

let db: Database | null = null;

// ---------------------------------------------------------------------------
// IndexedDB persistence
// ---------------------------------------------------------------------------

const IDB_NAME = "hadrian-wasm";
const IDB_STORE = "data";
const IDB_KEY = "db";

async function loadFromIndexedDB(): Promise<Uint8Array | null> {
  return new Promise((resolve) => {
    const req = indexedDB.open(IDB_NAME, 1);
    req.onupgradeneeded = () => req.result.createObjectStore(IDB_STORE);
    req.onsuccess = () => {
      const tx = req.result.transaction(IDB_STORE, "readonly");
      const get = tx.objectStore(IDB_STORE).get(IDB_KEY);
      get.onsuccess = () => resolve(get.result ?? null);
      get.onerror = () => resolve(null);
    };
    req.onerror = () => resolve(null);
  });
}

function saveToIndexedDB(data: Uint8Array): void {
  const req = indexedDB.open(IDB_NAME, 1);
  req.onupgradeneeded = () => req.result.createObjectStore(IDB_STORE);
  req.onsuccess = () => {
    const tx = req.result.transaction(IDB_STORE, "readwrite");
    tx.objectStore(IDB_STORE).put(data, IDB_KEY);
  };
}

let saveTimer: ReturnType<typeof setTimeout> | null = null;
function debouncedSave(): void {
  if (!db) return;
  if (saveTimer) clearTimeout(saveTimer);
  saveTimer = setTimeout(() => {
    const data = db!.export();
    saveToIndexedDB(new Uint8Array(data));
  }, 500);
}

// ---------------------------------------------------------------------------
// Parameter binding
// ---------------------------------------------------------------------------

/**
 * Bind values into a prepared statement.
 * The Rust bridge sends params as a JSON array of `WasmParam` values:
 *   - { Text: "..." } | { Integer: n } | { Real: f } | { Blob: [...] } | "Null"
 * sql.js accepts `(string | number | Uint8Array | null)[]`.
 */
function bindParams(params: unknown[]): (string | number | Uint8Array | null)[] {
  return params.map((p) => {
    if (p === null || p === "Null") return null;
    if (typeof p === "string" || typeof p === "number") return p;
    if (typeof p === "object" && p !== null) {
      const obj = p as Record<string, unknown>;
      if ("Text" in obj) return obj.Text as string;
      if ("Integer" in obj) return obj.Integer as number;
      if ("Real" in obj) return obj.Real as number;
      if ("Blob" in obj) return new Uint8Array(obj.Blob as number[]);
      if ("Null" in obj) return null;
    }
    return String(p);
  });
}

const bridge = {
  async init_database(): Promise<void> {
    // Fetch the WASM binary ourselves to avoid filename ambiguity.
    // esbuild resolves `sql.js` to the browser build which expects
    // "sql-wasm-browser.wasm", but we serve "sql-wasm.wasm".
    const wasmBinary = await fetch("/wasm/sql-wasm.wasm").then((r) => {
      if (!r.ok) throw new Error(`Failed to fetch sql-wasm.wasm: ${r.status}`);
      return r.arrayBuffer();
    });
    const SQL = await initSqlJs({ wasmBinary });

    // Try to restore from IndexedDB, otherwise create fresh
    const saved = await loadFromIndexedDB();
    db = saved ? new SQL.Database(saved) : new SQL.Database();

    // Enable WAL-like pragmas for better performance
    db.run("PRAGMA journal_mode = MEMORY");
    db.run("PRAGMA synchronous = OFF");
    db.run("PRAGMA foreign_keys = ON");

    console.log(
      `[sqlite-bridge] Database initialized${saved ? " (restored from IndexedDB)" : " (fresh)"}`
    );
  },

  async query(sql: string, params: unknown[]): Promise<Record<string, unknown>[]> {
    if (!db) throw new Error("Database not initialized — call init_database() first");

    const stmt = db.prepare(sql);
    stmt.bind(bindParams(params));

    const rows: Record<string, unknown>[] = [];
    while (stmt.step()) {
      const row = stmt.getAsObject();
      rows.push(row as Record<string, unknown>);
    }
    stmt.free();
    return rows;
  },

  async execute(
    sql: string,
    params: unknown[]
  ): Promise<{ changes: number; last_insert_rowid: number }> {
    if (!db) throw new Error("Database not initialized — call init_database() first");

    db.run(sql, bindParams(params));

    // sql.js doesn't return affected rows directly from run(),
    // so we query the SQLite functions.
    const changes = (db.exec("SELECT changes()")[0]?.values[0]?.[0] as number) ?? 0;
    const lastId = (db.exec("SELECT last_insert_rowid()")[0]?.values[0]?.[0] as number) ?? 0;

    debouncedSave();
    return { changes, last_insert_rowid: lastId };
  },

  async execute_script(sql: string): Promise<void> {
    if (!db) throw new Error("Database not initialized — call init_database() first");
    db.exec(sql);
    debouncedSave();
  },
};

// Expose on globalThis so the Rust wasm_bindgen FFI can find it.
(globalThis as unknown as Record<string, unknown>).__hadrian_sqlite = bridge;

console.log("[sqlite-bridge] Bridge registered on globalThis.__hadrian_sqlite");
