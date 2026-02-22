import { forwardRef, type LabelHTMLAttributes } from "react";

import { cn } from "@/utils/cn";

export type LabelProps = LabelHTMLAttributes<HTMLLabelElement>;

export const Label = forwardRef<HTMLLabelElement, LabelProps>(({ className, ...props }, ref) => {
  return (
    // eslint-disable-next-line jsx-a11y/label-has-associated-control -- generic component; htmlFor is passed via props at usage sites
    <label
      ref={ref}
      className={cn(
        "text-sm font-medium leading-none peer-disabled:cursor-not-allowed peer-disabled:opacity-70",
        className
      )}
      {...props}
    />
  );
});

Label.displayName = "Label";
