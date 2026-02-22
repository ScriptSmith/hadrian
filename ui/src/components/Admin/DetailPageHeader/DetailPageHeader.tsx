import { ArrowLeft, Pencil } from "lucide-react";

import { Button } from "@/components/Button/Button";
import { formatDateTime } from "@/utils/formatters";

export interface DetailPageHeaderProps {
  title: string;
  slug?: string;
  createdAt?: string;
  onBack: () => void;
  onEdit?: () => void;
  backLabel?: string;
}

export function DetailPageHeader({
  title,
  slug,
  createdAt,
  onBack,
  onEdit,
}: DetailPageHeaderProps) {
  return (
    <div className="flex items-center gap-4">
      <Button variant="ghost" size="icon" onClick={onBack} aria-label="Go back">
        <ArrowLeft className="h-4 w-4" />
      </Button>
      <div className="flex-1">
        <div className="flex items-center gap-3">
          <h1 className="text-2xl font-semibold">{title}</h1>
          {slug && <code className="rounded bg-muted px-2 py-1 text-sm">{slug}</code>}
        </div>
        {createdAt && (
          <p className="text-muted-foreground text-sm">Created {formatDateTime(createdAt)}</p>
        )}
      </div>
      {onEdit && (
        <Button variant="outline" onClick={onEdit}>
          <Pencil className="mr-2 h-4 w-4" />
          Edit
        </Button>
      )}
    </div>
  );
}
