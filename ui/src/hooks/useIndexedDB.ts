import { useState, useEffect, useCallback, useRef } from "react";
import { clearAllAudioFiles } from "@/services/opfs/opfsService";

const DB_NAME = "hadrian-storage";
const DB_VERSION = 1;
const STORE_NAME = "keyval";

let dbPromise: Promise<IDBDatabase> | null = null;

function openDB(): Promise<IDBDatabase> {
  if (dbPromise) return dbPromise;

  dbPromise = new Promise((resolve, reject) => {
    if (typeof window === "undefined" || !window.indexedDB) {
      reject(new Error("IndexedDB not available"));
      return;
    }

    const request = indexedDB.open(DB_NAME, DB_VERSION);

    request.onerror = () => reject(request.error);
    request.onsuccess = () => resolve(request.result);

    request.onupgradeneeded = (event) => {
      const db = (event.target as IDBOpenDBRequest).result;
      if (!db.objectStoreNames.contains(STORE_NAME)) {
        db.createObjectStore(STORE_NAME);
      }
    };
  });

  return dbPromise;
}

async function getValue<T>(key: string): Promise<T | undefined> {
  const db = await openDB();
  return new Promise((resolve, reject) => {
    const transaction = db.transaction(STORE_NAME, "readonly");
    const store = transaction.objectStore(STORE_NAME);
    const request = store.get(key);

    request.onerror = () => reject(request.error);
    request.onsuccess = () => resolve(request.result as T | undefined);
  });
}

async function setValue<T>(key: string, value: T): Promise<void> {
  const db = await openDB();
  return new Promise((resolve, reject) => {
    const transaction = db.transaction(STORE_NAME, "readwrite");
    const store = transaction.objectStore(STORE_NAME);
    const request = store.put(value, key);

    request.onerror = () => reject(request.error);
    request.onsuccess = () => resolve();
  });
}

/**
 * Export all data from IndexedDB as a key-value object.
 * Useful for data portability (GDPR Article 20).
 */
export async function exportAllIndexedDBData(): Promise<Record<string, unknown>> {
  try {
    const db = await openDB();
    return new Promise((resolve, reject) => {
      const transaction = db.transaction(STORE_NAME, "readonly");
      const store = transaction.objectStore(STORE_NAME);
      const request = store.openCursor();
      const result: Record<string, unknown> = {};

      request.onerror = () => reject(request.error);
      request.onsuccess = (event) => {
        const cursor = (event.target as IDBRequest<IDBCursorWithValue>).result;
        if (cursor) {
          result[cursor.key as string] = cursor.value;
          cursor.continue();
        } else {
          resolve(result);
        }
      };
    });
  } catch (error) {
    console.warn("Failed to export IndexedDB data:", error);
    return {};
  }
}

/**
 * Clear all data from IndexedDB.
 * Used for account deletion (GDPR Article 17) or user-initiated data clearing.
 */
export async function clearAllIndexedDBData(): Promise<void> {
  try {
    const db = await openDB();
    await new Promise<void>((resolve, reject) => {
      const transaction = db.transaction(STORE_NAME, "readwrite");
      const store = transaction.objectStore(STORE_NAME);
      const request = store.clear();

      request.onerror = () => reject(request.error);
      request.onsuccess = () => resolve();
    });
    // Also clear OPFS audio files
    await clearAllAudioFiles();
  } catch (error) {
    console.warn("Failed to clear IndexedDB data:", error);
    throw error;
  }
}

/**
 * Delete the entire IndexedDB database.
 * More thorough cleanup than clearAllIndexedDBData.
 */
export async function deleteIndexedDBDatabase(): Promise<void> {
  // Close any existing connection
  if (dbPromise) {
    try {
      const db = await dbPromise;
      db.close();
    } catch {
      // Ignore
    }
    dbPromise = null;
  }

  await new Promise<void>((resolve, reject) => {
    if (typeof window === "undefined" || !window.indexedDB) {
      resolve();
      return;
    }

    const request = indexedDB.deleteDatabase(DB_NAME);
    request.onerror = () => reject(request.error);
    request.onsuccess = () => resolve();
    request.onblocked = () => {
      console.warn("IndexedDB deletion blocked - other tabs may have connections");
      resolve(); // Resolve anyway, the browser will complete deletion when tabs close
    };
  });

  // Also clear OPFS audio files
  await clearAllAudioFiles();
}

export interface UseIndexedDBResult<T> {
  value: T;
  setValue: (value: T | ((prev: T) => T)) => void;
  isLoading: boolean;
}

/**
 * React hook for storing data in IndexedDB.
 * Provides a similar interface to useState but persists to IndexedDB.
 *
 * @param key - The key to store the value under
 * @param initialValue - The initial value to use if no stored value exists
 * @returns Object with value, setValue function, and loading state
 */
export function useIndexedDB<T>(key: string, initialValue: T): UseIndexedDBResult<T> {
  const [value, setValueState] = useState<T>(initialValue);
  const [isLoading, setIsLoading] = useState(true);
  const valueRef = useRef<T>(initialValue);
  const pendingWriteRef = useRef<Promise<void> | null>(null);

  // Keep ref in sync
  valueRef.current = value;

  // Load initial value from IndexedDB
  useEffect(() => {
    let cancelled = false;

    async function load() {
      try {
        const stored = await getValue<T>(key);
        if (cancelled) return;

        if (stored !== undefined) {
          setValueState(stored);
          valueRef.current = stored;
        }
      } catch (error) {
        console.warn("Failed to load from IndexedDB:", error);
      } finally {
        if (!cancelled) {
          setIsLoading(false);
        }
      }
    }

    load();

    return () => {
      cancelled = true;
    };
  }, [key]);

  const setValueCallback = useCallback(
    (updater: T | ((prev: T) => T)) => {
      setValueState((prev) => {
        const newValue = updater instanceof Function ? updater(prev) : updater;
        valueRef.current = newValue;

        // Queue write to IndexedDB (fire and forget, but chain to prevent races)
        const previousWrite = pendingWriteRef.current ?? Promise.resolve();
        pendingWriteRef.current = previousWrite.then(async () => {
          try {
            await setValue(key, newValue);
          } catch (error) {
            console.warn("Failed to write to IndexedDB:", error);
          }
        });

        return newValue;
      });
    },
    [key]
  );

  return { value, setValue: setValueCallback, isLoading };
}
