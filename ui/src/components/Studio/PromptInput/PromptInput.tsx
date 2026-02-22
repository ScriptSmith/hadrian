import { useRef, useCallback, useEffect, useId, type KeyboardEvent } from "react";
import { cn } from "@/utils/cn";

interface PromptInputProps {
  value: string;
  onChange: (value: string) => void;
  onSubmit: () => void;
  placeholder?: string;
  disabled?: boolean;
  maxLength?: number;
  minHeight?: number;
  maxHeight?: number;
  className?: string;
}

export function PromptInput({
  value,
  onChange,
  onSubmit,
  placeholder = "Enter your prompt...",
  disabled = false,
  maxLength,
  minHeight = 80,
  maxHeight = 240,
  className,
}: PromptInputProps) {
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const id = useId();

  const adjustHeight = useCallback(() => {
    const el = textareaRef.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${Math.min(Math.max(el.scrollHeight, minHeight), maxHeight)}px`;
  }, [minHeight, maxHeight]);

  useEffect(() => {
    adjustHeight();
  }, [value, adjustHeight]);

  const handleKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
      e.preventDefault();
      if (!disabled && value.trim()) {
        onSubmit();
      }
    }
  };

  const isMac = typeof navigator !== "undefined" && /Mac/.test(navigator.userAgent);
  const shortcutHint = isMac ? "\u2318\u21B5" : "Ctrl+\u21B5";

  return (
    <div className={cn("relative", className)}>
      <label htmlFor={id} className="sr-only">
        Prompt
      </label>
      <textarea
        ref={textareaRef}
        id={id}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        onKeyDown={handleKeyDown}
        placeholder={placeholder}
        disabled={disabled}
        maxLength={maxLength}
        rows={1}
        className={cn(
          "w-full resize-none rounded-xl border border-input bg-background px-4 py-3",
          "text-sm leading-relaxed placeholder:text-muted-foreground/60 placeholder:italic",
          "ring-offset-background",
          "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2",
          "disabled:cursor-not-allowed disabled:opacity-50",
          "motion-safe:transition-shadow motion-safe:duration-200",
          "focus-visible:shadow-md"
        )}
        style={{ minHeight }}
      />

      {/* Bottom row: character count + shortcut hint */}
      <div className="pointer-events-none absolute bottom-2 right-3 flex items-center gap-2">
        {maxLength && value.length > 0 && (
          <span
            className={cn(
              "text-[11px] tabular-nums motion-safe:transition-opacity motion-safe:duration-200",
              value.length > maxLength * 0.9 ? "text-destructive" : "text-muted-foreground"
            )}
          >
            {value.length}/{maxLength}
          </span>
        )}
        {value.trim().length > 0 && (
          <kbd className="rounded bg-muted/70 px-1.5 py-0.5 text-[10px] text-muted-foreground">
            {shortcutHint}
          </kbd>
        )}
      </div>
    </div>
  );
}
