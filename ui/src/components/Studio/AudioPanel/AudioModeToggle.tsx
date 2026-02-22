import { cn } from "@/utils/cn";

export type AudioMode = "speak" | "transcribe" | "translate";

const MODES: { id: AudioMode; label: string }[] = [
  { id: "speak", label: "Speak" },
  { id: "transcribe", label: "Transcribe" },
  { id: "translate", label: "Translate" },
];

interface AudioModeToggleProps {
  value: AudioMode;
  onChange: (mode: AudioMode) => void;
}

export function AudioModeToggle({ value, onChange }: AudioModeToggleProps) {
  return (
    <div className="flex gap-1 rounded-lg bg-muted/50 p-1" role="radiogroup" aria-label="Mode">
      {MODES.map((m) => (
        <button
          key={m.id}
          type="button"
          role="radio"
          aria-checked={value === m.id}
          className={cn(
            "flex-1 rounded-md px-3 py-1.5 text-sm font-medium",
            "motion-safe:transition-colors motion-safe:duration-150",
            "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
            value === m.id
              ? "bg-background text-foreground shadow-sm"
              : "text-muted-foreground hover:text-foreground"
          )}
          onClick={() => onChange(m.id)}
        >
          {m.label}
        </button>
      ))}
    </div>
  );
}
