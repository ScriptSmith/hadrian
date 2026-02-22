import { Sun, Moon, Monitor } from "lucide-react";
import { usePreferences } from "@/preferences/PreferencesProvider";
import type { Theme } from "@/preferences/types";
import { cn } from "@/utils/cn";

const themes: { value: Theme; icon: typeof Sun; label: string }[] = [
  { value: "light", icon: Sun, label: "Light" },
  { value: "dark", icon: Moon, label: "Dark" },
  { value: "system", icon: Monitor, label: "System" },
];

interface ThemeToggleProps {
  className?: string;
}

export function ThemeToggle({ className }: ThemeToggleProps) {
  const { preferences, setTheme } = usePreferences();

  return (
    <div className={cn("flex items-center rounded-full bg-secondary p-0.5", className)}>
      {themes.map(({ value, icon: Icon, label }) => (
        <button
          key={value}
          type="button"
          onClick={() => setTheme(value)}
          className={cn(
            "rounded-full p-1.5 text-muted-foreground transition-all duration-200",
            "hover:text-foreground",
            preferences.theme === value && "bg-background text-foreground shadow-sm"
          )}
          title={label}
        >
          <Icon className="h-3.5 w-3.5" />
          <span className="sr-only">{label}</span>
        </button>
      ))}
    </div>
  );
}
