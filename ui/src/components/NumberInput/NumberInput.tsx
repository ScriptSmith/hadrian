import { type InputHTMLAttributes, forwardRef, useState, useEffect, useId } from "react";
import { Minus, Plus } from "lucide-react";
import { cn } from "@/utils/cn";

interface NumberInputProps extends Omit<
  InputHTMLAttributes<HTMLInputElement>,
  "type" | "onChange"
> {
  value: number;
  onChange: (value: number) => void;
  min?: number;
  max?: number;
  step?: number;
  label?: string;
  showButtons?: boolean;
}

export const NumberInput = forwardRef<HTMLInputElement, NumberInputProps>(
  (
    { className, value, onChange, min, max, step = 1, label, showButtons = true, ...props },
    ref
  ) => {
    const inputId = useId();
    const [inputValue, setInputValue] = useState(String(value));

    // Sync input value when prop changes
    useEffect(() => {
      setInputValue(String(value));
    }, [value]);

    const clamp = (v: number): number => {
      if (min !== undefined && v < min) return min;
      if (max !== undefined && v > max) return max;
      return v;
    };

    const handleInputChange = (e: React.ChangeEvent<HTMLInputElement>) => {
      const raw = e.target.value;
      setInputValue(raw);

      const num = Number(raw);
      if (!isNaN(num)) {
        onChange(clamp(num));
      }
    };

    const handleBlur = () => {
      const num = Number(inputValue);
      if (isNaN(num)) {
        setInputValue(String(value));
      } else {
        const clamped = clamp(num);
        setInputValue(String(clamped));
        onChange(clamped);
      }
    };

    const handleKeyDown = (e: React.KeyboardEvent) => {
      if (e.key === "ArrowUp") {
        e.preventDefault();
        const newValue = clamp(value + step);
        onChange(newValue);
      } else if (e.key === "ArrowDown") {
        e.preventDefault();
        const newValue = clamp(value - step);
        onChange(newValue);
      }
    };

    const increment = () => onChange(clamp(value + step));
    const decrement = () => onChange(clamp(value - step));

    const canDecrement = min === undefined || value > min;
    const canIncrement = max === undefined || value < max;

    return (
      <div className={cn("space-y-1.5", className)}>
        {label && (
          <label htmlFor={inputId} className="text-sm text-muted-foreground">
            {label}
          </label>
        )}
        <div className="relative flex items-center">
          {showButtons && (
            <button
              type="button"
              onClick={decrement}
              disabled={!canDecrement || props.disabled}
              aria-label="Decrease value"
              className={cn(
                "absolute left-0 z-10 flex h-full w-8 items-center justify-center",
                "rounded-l-md border-r border-input bg-muted/50 transition-colors",
                "hover:bg-muted focus:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-inset",
                "disabled:cursor-not-allowed disabled:opacity-40"
              )}
            >
              <Minus className="h-3 w-3" />
            </button>
          )}
          <input
            ref={ref}
            id={inputId}
            type="text"
            inputMode="numeric"
            value={inputValue}
            onChange={handleInputChange}
            onBlur={handleBlur}
            onKeyDown={handleKeyDown}
            className={cn(
              "h-8 w-full rounded-md border border-input bg-background text-center text-sm tabular-nums",
              "focus:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-1",
              "disabled:cursor-not-allowed disabled:opacity-50",
              showButtons && "px-9"
            )}
            {...props}
          />
          {showButtons && (
            <button
              type="button"
              onClick={increment}
              disabled={!canIncrement || props.disabled}
              aria-label="Increase value"
              className={cn(
                "absolute right-0 z-10 flex h-full w-8 items-center justify-center",
                "rounded-r-md border-l border-input bg-muted/50 transition-colors",
                "hover:bg-muted focus:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-inset",
                "disabled:cursor-not-allowed disabled:opacity-40"
              )}
            >
              <Plus className="h-3 w-3" />
            </button>
          )}
        </div>
      </div>
    );
  }
);

NumberInput.displayName = "NumberInput";
