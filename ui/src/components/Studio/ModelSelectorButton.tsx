import { useState, useCallback } from "react";
import { ChevronDown } from "lucide-react";
import { cn } from "@/utils/cn";
import { ModelPicker, type ModelInfo } from "@/components/ModelPicker/ModelPicker";

interface ModelSelectorButtonProps {
  model: string;
  onModelChange: (model: string) => void;
  availableModels: ModelInfo[];
  disabled?: boolean;
  label?: string;
}

export function ModelSelectorButton({
  model,
  onModelChange,
  availableModels,
  disabled,
  label = "Model",
}: ModelSelectorButtonProps) {
  const [open, setOpen] = useState(false);

  const handleModelsChange = useCallback(
    (models: string[]) => {
      if (models.length > 0) {
        onModelChange(models[models.length - 1]);
      }
      setOpen(false);
    },
    [onModelChange]
  );

  return (
    <div>
      <span className="mb-1.5 block text-xs font-medium text-muted-foreground">{label}</span>
      <button
        type="button"
        disabled={disabled}
        onClick={() => setOpen(true)}
        className={cn(
          "flex w-full items-center justify-between gap-2 rounded-lg border border-input bg-background px-3 py-2 text-sm",
          "hover:bg-accent/50 motion-safe:transition-colors",
          "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2",
          "disabled:cursor-not-allowed disabled:opacity-50"
        )}
      >
        <span className="truncate">{model || "Select model..."}</span>
        <ChevronDown className="h-4 w-4 shrink-0 text-muted-foreground" aria-hidden="true" />
      </button>

      <ModelPicker
        open={open}
        onClose={() => setOpen(false)}
        selectedModels={[model]}
        onModelsChange={handleModelsChange}
        availableModels={availableModels}
        maxModels={1}
      />
    </div>
  );
}
