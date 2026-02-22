import type { ReactNode } from "react";

export interface CodeBadgeProps {
  children: ReactNode;
  className?: string;
  truncate?: boolean;
}

export function CodeBadge({ children, className = "", truncate }: CodeBadgeProps) {
  return (
    <code
      className={`rounded bg-muted px-1.5 py-0.5 text-sm ${truncate ? "truncate" : ""} ${className}`}
    >
      {children}
    </code>
  );
}
