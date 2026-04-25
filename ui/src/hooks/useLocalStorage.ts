import { useState, useEffect, useCallback } from "react";
import type { ZodType } from "zod";

// `storage` events only fire in *other* tabs. To keep multiple hook instances
// of the same key inside the same tab in sync, mirror writes onto a custom
// event we dispatch ourselves.
const SAME_TAB_EVENT = "hadrian:local-storage";

interface SameTabPayload {
  key: string;
  newValue: string | null;
}

/**
 * Persist state to `localStorage` with same-tab and cross-tab sync.
 *
 * Pass an optional zod `schema` to validate values arriving from
 * `localStorage` (initial read, `storage` events, same-tab broadcasts).
 * Anything that fails validation is discarded — without a schema, a
 * malicious or stale tab could write any JSON-shaped value into the key
 * and surface it as a typed `T`. Callers handling user-controlled keys
 * (auth tokens, preferences, settings) should always supply a schema.
 */
export function useLocalStorage<T>(
  key: string,
  initialValue: T,
  schema?: ZodType<T>
): [T, (value: T | ((prev: T) => T)) => void] {
  const parse = useCallback(
    (raw: string | null): T | undefined => {
      if (raw === null) return undefined;
      try {
        const parsed: unknown = JSON.parse(raw);
        if (!schema) return parsed as T;
        const result = schema.safeParse(parsed);
        return result.success ? result.data : undefined;
      } catch {
        return undefined;
      }
    },
    [schema]
  );

  const [storedValue, setStoredValue] = useState<T>(() => {
    if (typeof window === "undefined") {
      return initialValue;
    }
    return parse(window.localStorage.getItem(key)) ?? initialValue;
  });

  const setValue = useCallback(
    (value: T | ((prev: T) => T)) => {
      setStoredValue((prev) => {
        const valueToStore = value instanceof Function ? value(prev) : value;
        if (typeof window !== "undefined") {
          const serialized = JSON.stringify(valueToStore);
          window.localStorage.setItem(key, serialized);
          window.dispatchEvent(
            new CustomEvent<SameTabPayload>(SAME_TAB_EVENT, {
              detail: { key, newValue: serialized },
            })
          );
        }
        return valueToStore;
      });
    },
    [key]
  );

  useEffect(() => {
    const apply = (newValue: string | null) => {
      const next = parse(newValue);
      if (next !== undefined) setStoredValue(next);
    };

    const handleStorageChange = (e: StorageEvent) => {
      if (e.key === key) apply(e.newValue);
    };
    const handleSameTabChange = (e: Event) => {
      const detail = (e as CustomEvent<SameTabPayload>).detail;
      if (detail?.key === key) apply(detail.newValue);
    };

    window.addEventListener("storage", handleStorageChange);
    window.addEventListener(SAME_TAB_EVENT, handleSameTabChange);
    return () => {
      window.removeEventListener("storage", handleStorageChange);
      window.removeEventListener(SAME_TAB_EVENT, handleSameTabChange);
    };
  }, [key, parse]);

  return [storedValue, setValue];
}
