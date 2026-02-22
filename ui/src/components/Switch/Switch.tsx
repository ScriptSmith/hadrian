import { forwardRef, type InputHTMLAttributes } from "react";

export interface SwitchProps extends Omit<InputHTMLAttributes<HTMLInputElement>, "type"> {
  label?: string;
  description?: string;
}

export const Switch = forwardRef<HTMLInputElement, SwitchProps>(
  ({ label, description, className, id, ...props }, ref) => {
    const inputId = id || `switch-${Math.random().toString(36).slice(2, 9)}`;

    const switchElement = (
      <label className="relative inline-flex cursor-pointer items-center">
        <input ref={ref} type="checkbox" id={inputId} className="peer sr-only" {...props} />
        <div className="peer h-6 w-11 rounded-full bg-muted after:absolute after:left-[2px] after:top-[2px] after:h-5 after:w-5 after:rounded-full after:bg-background after:transition-all after:content-[''] peer-checked:bg-primary peer-checked:after:translate-x-full peer-disabled:cursor-not-allowed peer-disabled:opacity-50" />
      </label>
    );

    if (!label && !description) {
      return switchElement;
    }

    return (
      <div className={`flex items-center justify-between ${className || ""}`}>
        <div>
          {label && (
            <label htmlFor={inputId} className="font-medium cursor-pointer">
              {label}
            </label>
          )}
          {description && <p className="text-sm text-muted-foreground">{description}</p>}
        </div>
        {switchElement}
      </div>
    );
  }
);

Switch.displayName = "Switch";
