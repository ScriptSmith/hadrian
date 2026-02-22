import { createContext, useContext, useEffect, useCallback, type ReactNode } from "react";
import { useLocalStorage } from "@/hooks/useLocalStorage";
import { usePrefersDarkMode } from "@/hooks/useMediaQuery";
import type { UserPreferences, Theme } from "./types";
import { defaultPreferences } from "./types";

interface PreferencesContextValue {
  preferences: UserPreferences;
  setPreferences: (prefs: Partial<UserPreferences>) => void;
  setTheme: (theme: Theme) => void;
  resolvedTheme: "light" | "dark";
}

const PreferencesContext = createContext<PreferencesContextValue | null>(null);

const STORAGE_KEY = "hadrian-preferences";

interface PreferencesProviderProps {
  children: ReactNode;
}

export function PreferencesProvider({ children }: PreferencesProviderProps) {
  const [preferences, setStoredPreferences] = useLocalStorage<UserPreferences>(
    STORAGE_KEY,
    defaultPreferences
  );

  const prefersDark = usePrefersDarkMode();

  const resolvedTheme =
    preferences.theme === "system" ? (prefersDark ? "dark" : "light") : preferences.theme;

  // Apply theme to document
  useEffect(() => {
    const root = document.documentElement;
    root.classList.remove("light", "dark");
    root.classList.add(resolvedTheme);
  }, [resolvedTheme]);

  const setPreferences = useCallback(
    (updates: Partial<UserPreferences>) => {
      setStoredPreferences((prev) => ({ ...prev, ...updates }));
    },
    [setStoredPreferences]
  );

  const setTheme = useCallback(
    (theme: Theme) => {
      setPreferences({ theme });
    },
    [setPreferences]
  );

  return (
    <PreferencesContext.Provider
      value={{
        preferences,
        setPreferences,
        setTheme,
        resolvedTheme,
      }}
    >
      {children}
    </PreferencesContext.Provider>
  );
}

export function usePreferences(): PreferencesContextValue {
  const context = useContext(PreferencesContext);
  if (!context) {
    throw new Error("usePreferences must be used within a PreferencesProvider");
  }
  return context;
}
