import { Plus } from "lucide-react";
import type { ReactNode } from "react";

import { Button } from "@/components/Button/Button";

export interface PageHeaderProps {
  title: string;
  description: string;
  actionLabel?: string;
  onAction?: () => void;
  actionDisabled?: boolean;
  actionIcon?: ReactNode;
}

export function PageHeader({
  title,
  description,
  actionLabel,
  onAction,
  actionDisabled,
  actionIcon = <Plus className="mr-2 h-4 w-4" />,
}: PageHeaderProps) {
  return (
    <div className="mb-6 flex items-center justify-between">
      <div>
        <h1 className="text-2xl font-semibold">{title}</h1>
        <p className="text-muted-foreground">{description}</p>
      </div>
      {actionLabel && onAction && (
        <Button onClick={onAction} disabled={actionDisabled}>
          {actionIcon}
          {actionLabel}
        </Button>
      )}
    </div>
  );
}
