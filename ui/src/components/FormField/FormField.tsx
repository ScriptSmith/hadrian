import { type ReactNode, useId, cloneElement, isValidElement, Children } from "react";

export interface FormFieldProps {
  label: ReactNode;
  htmlFor?: string;
  helpText?: string;
  error?: string;
  required?: boolean;
  children: ReactNode;
  className?: string;
}

export function FormField({
  label,
  htmlFor,
  helpText,
  error,
  required,
  children,
  className,
}: FormFieldProps) {
  const generatedId = useId();
  const inputId = htmlFor || generatedId;
  const helpTextId = `${inputId}-help`;
  const errorId = `${inputId}-error`;

  // Build aria-describedby value based on what's shown
  const ariaDescribedBy = error ? errorId : helpText ? helpTextId : undefined;

  // Clone child to add aria-describedby if it's a single element
  const enhancedChildren = Children.map(children, (child) => {
    if (isValidElement(child) && ariaDescribedBy) {
      return cloneElement(child, {
        "aria-describedby": ariaDescribedBy,
        "aria-invalid": error ? true : undefined,
      } as React.HTMLAttributes<HTMLElement>);
    }
    return child;
  });

  return (
    <div className={className}>
      <label htmlFor={inputId} className="mb-1 block text-sm font-medium">
        {label}
        {required && (
          <span className="text-destructive ml-1" aria-hidden="true">
            *
          </span>
        )}
        {required && <span className="sr-only">(required)</span>}
      </label>
      {enhancedChildren}
      {helpText && !error && (
        <p id={helpTextId} className="mt-1 text-xs text-muted-foreground">
          {helpText}
        </p>
      )}
      {error && (
        <p id={errorId} className="mt-1 text-xs text-destructive" role="alert">
          {error}
        </p>
      )}
    </div>
  );
}
