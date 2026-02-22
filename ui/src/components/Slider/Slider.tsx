import { type InputHTMLAttributes, forwardRef, useId } from "react";
import { cn } from "@/utils/cn";

interface SliderProps extends Omit<InputHTMLAttributes<HTMLInputElement>, "type" | "onChange"> {
  value: number;
  onChange: (value: number) => void;
  min?: number;
  max?: number;
  step?: number;
  showValue?: boolean;
  label?: string;
}

export const Slider = forwardRef<HTMLInputElement, SliderProps>(
  (
    { className, value, onChange, min = 0, max = 100, step = 1, showValue, label, ...props },
    ref
  ) => {
    const inputId = useId();
    const percentage = ((value - min) / (max - min)) * 100;

    // Format value for display
    const displayValue = Number.isInteger(step) ? value : value.toFixed(step < 0.1 ? 2 : 1);

    return (
      <div className="space-y-2.5">
        {(label || showValue) && (
          <div className="flex items-center justify-between">
            {label && (
              <label htmlFor={inputId} className="text-sm text-muted-foreground">
                {label}
              </label>
            )}
            {showValue && (
              <span className="min-w-[3rem] rounded-md bg-muted/50 px-2 py-0.5 text-right text-sm font-medium tabular-nums">
                {displayValue}
              </span>
            )}
          </div>
        )}
        <div className="relative flex h-5 items-center">
          <div className="absolute inset-x-0 h-1.5 rounded-full bg-muted" />
          <div
            className="absolute left-0 h-1.5 rounded-full bg-gradient-to-r from-primary/80 to-primary transition-all"
            style={{ width: `${percentage}%` }}
          />
          <input
            ref={ref}
            id={inputId}
            type="range"
            min={min}
            max={max}
            step={step}
            value={value}
            onChange={(e) => onChange(Number(e.target.value))}
            className={cn(
              "absolute inset-0 w-full cursor-pointer appearance-none bg-transparent",
              // Webkit (Chrome, Safari, Edge)
              "[&::-webkit-slider-thumb]:appearance-none",
              "[&::-webkit-slider-thumb]:h-4 [&::-webkit-slider-thumb]:w-4",
              "[&::-webkit-slider-thumb]:rounded-full",
              "[&::-webkit-slider-thumb]:bg-primary",
              "[&::-webkit-slider-thumb]:shadow-[0_0_0_3px_hsl(var(--background)),0_2px_6px_rgba(0,0,0,0.2)]",
              "[&::-webkit-slider-thumb]:transition-all [&::-webkit-slider-thumb]:duration-150",
              "[&::-webkit-slider-thumb]:hover:scale-110 [&::-webkit-slider-thumb]:hover:shadow-[0_0_0_3px_hsl(var(--background)),0_3px_8px_rgba(0,0,0,0.3)]",
              "[&::-webkit-slider-thumb]:active:scale-95",
              // Firefox
              "[&::-moz-range-thumb]:h-4 [&::-moz-range-thumb]:w-4",
              "[&::-moz-range-thumb]:rounded-full [&::-moz-range-thumb]:border-0",
              "[&::-moz-range-thumb]:bg-primary",
              "[&::-moz-range-thumb]:shadow-[0_0_0_3px_hsl(var(--background)),0_2px_6px_rgba(0,0,0,0.2)]",
              "[&::-moz-range-thumb]:transition-all [&::-moz-range-thumb]:duration-150",
              "[&::-moz-range-thumb]:hover:scale-110",
              "[&::-moz-range-track]:bg-transparent",
              "focus:outline-none focus-visible:[&::-webkit-slider-thumb]:ring-2 focus-visible:[&::-webkit-slider-thumb]:ring-ring",
              className
            )}
            {...props}
          />
        </div>
      </div>
    );
  }
);

Slider.displayName = "Slider";
