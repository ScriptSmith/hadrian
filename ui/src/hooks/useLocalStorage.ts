import { useState, useEffect, useCallback } from "react";

// `storage` events only fire in *other* tabs. To keep multiple hook instances
// of the same key inside the same tab in sync, mirror writes onto a custom
// event we dispatch ourselves.
const SAME_TAB_EVENT = "hadrian:local-storage";

interface SameTabPayload {
  key: string;
  newValue: string | null;
}

export function useLocalStorage<T>(
  key: string,
  initialValue: T
): [T, (value: T | ((prev: T) => T)) => void] {
  const [storedValue, setStoredValue] = useState<T>(() => {
    if (typeof window === "undefined") {
      return initialValue;
    }
    try {
      const item = window.localStorage.getItem(key);
      return item ? (JSON.parse(item) as T) : initialValue;
    } catch {
      return initialValue;
    }
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
      if (newValue === null) return;
      try {
        setStoredValue(JSON.parse(newValue) as T);
      } catch {
        // Ignore parse errors
      }
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
  }, [key]);

  return [storedValue, setValue];
}
