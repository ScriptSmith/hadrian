import {
  closestCenter,
  DndContext,
  type DragEndEvent,
  KeyboardSensor,
  PointerSensor,
  useSensor,
  useSensors,
} from "@dnd-kit/core";
import {
  arrayMove,
  horizontalListSortingStrategy,
  SortableContext,
  sortableKeyboardCoordinates,
  useSortable,
} from "@dnd-kit/sortable";
import { GripVertical, Plus, X } from "lucide-react";
import { useState } from "react";

import { Button } from "@/components/Button/Button";
import { Spinner } from "@/components/Spinner/Spinner";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/Tooltip/Tooltip";

import type { ModelInstance, ModelParameters } from "@/components/chat-types";
import { createInstanceId, getInstanceLabel } from "@/components/chat-types";
import { ModelParametersPopover } from "@/components/ModelParametersPopover/ModelParametersPopover";
import { ModelPicker, type ModelInfo } from "@/components/ModelPicker/ModelPicker";
import type { ModelTask } from "@/preferences/types";
import { cn } from "@/utils/cn";
import { getModelDisplayName } from "@/utils/modelNames";

// Re-export ModelInfo from ModelPicker for consumers that import from this module
export type { ModelInfo } from "@/components/ModelPicker/ModelPicker";

interface ModelSelectorProps {
  /** Selected model instances */
  selectedInstances: ModelInstance[];
  /** Callback when instances change */
  onInstancesChange: (instances: ModelInstance[]) => void;
  availableModels: ModelInfo[];
  /** Whether models are still loading from the API */
  isLoading?: boolean;
  maxModels?: number;
  /** Callback when instance parameters change */
  onInstanceParametersChange?: (instanceId: string, params: ModelParameters) => void;
  /** Callback when instance label changes */
  onInstanceLabelChange?: (instanceId: string, label: string) => void;
  /** Instances that are disabled (hidden from view, not queried) */
  disabledInstances?: string[];
  /** Callback when disabled instances change */
  onDisabledInstancesChange?: (instanceIds: string[]) => void;
  /** Whether we're in an active conversation (shows disable option) */
  hasMessages?: boolean;
  /** Which task context this selector is used in (scopes favorites/defaults) */
  task?: ModelTask;
}

interface SortableInstanceChipProps {
  instance: ModelInstance;
  displayName: string;
  isDisabled: boolean;
  canToggleDisabled: boolean;
  onToggleDisabled: (instanceId: string) => void;
  onRemove: (instanceId: string, e: React.MouseEvent) => void;
  onParametersChange?: (instanceId: string, params: ModelParameters) => void;
  onLabelChange?: (instanceId: string, label: string) => void;
  onDuplicate?: () => void;
}

function SortableInstanceChip({
  instance,
  displayName,
  isDisabled,
  canToggleDisabled,
  onToggleDisabled,
  onRemove,
  onParametersChange,
  onLabelChange,
  onDuplicate,
}: SortableInstanceChipProps) {
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({
    id: instance.id,
  });

  const style: React.CSSProperties = {
    // Only use translate, not the full transform (which includes scale)
    transform: transform ? `translate3d(${transform.x}px, ${transform.y}px, 0)` : undefined,
    transition,
    // Ensure dragging item is above others
    zIndex: isDragging ? 50 : undefined,
  };

  // Check if this instance has a custom label (different from model ID)
  const hasCustomLabel = instance.label && instance.label !== instance.modelId;

  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <div
          ref={setNodeRef}
          style={style}
          className={cn(
            "group flex items-center gap-1 sm:gap-1.5 rounded-lg border px-2 sm:px-2.5 py-1 sm:py-1.5 text-xs sm:text-sm shrink-0",
            isDisabled
              ? "border-dashed border-muted-foreground/30 bg-muted/30 text-muted-foreground"
              : "border-border bg-secondary/50 hover:bg-secondary",
            isDragging && "opacity-90 shadow-lg"
          )}
        >
          {/* Drag handle - hidden on mobile */}
          <button
            aria-label={`Reorder ${displayName}`}
            {...attributes}
            {...listeners}
            className="hidden sm:block cursor-grab touch-none rounded p-0.5 text-muted-foreground/40 transition-colors hover:text-muted-foreground active:cursor-grabbing"
            onClick={(e) => e.stopPropagation()}
          >
            <GripVertical className="h-3.5 w-3.5" />
          </button>

          <span
            role={canToggleDisabled ? "button" : undefined}
            tabIndex={canToggleDisabled ? 0 : undefined}
            onClick={(e) => {
              if (canToggleDisabled) {
                e.stopPropagation();
                onToggleDisabled(instance.id);
              }
            }}
            onKeyDown={(e) => {
              if (canToggleDisabled && (e.key === "Enter" || e.key === " ")) {
                e.preventDefault();
                onToggleDisabled(instance.id);
              }
            }}
            className={cn(
              "font-medium",
              isDisabled && "line-through",
              canToggleDisabled && "cursor-pointer select-none"
            )}
          >
            {displayName}
            {/* Show instance suffix if this is a duplicate (ID differs from modelId) */}
            {instance.id !== instance.modelId && !hasCustomLabel && (
              <span className="ml-1 text-xs text-muted-foreground">
                #{instance.id.split("-").pop()}
              </span>
            )}
          </span>

          {/* Settings popover */}
          {onParametersChange && !isDisabled && (
            <ModelParametersPopover
              modelName={displayName}
              parameters={instance.parameters ?? {}}
              onParametersChange={(params) => onParametersChange(instance.id, params)}
              instanceLabel={instance.label}
              onLabelChange={
                onLabelChange ? (label) => onLabelChange(instance.id, label) : undefined
              }
              onDuplicate={onDuplicate}
            />
          )}

          {/* Remove button */}
          <button
            aria-label={`Remove ${displayName}`}
            onClick={(e) => onRemove(instance.id, e)}
            className="rounded p-0.5 text-muted-foreground transition-colors hover:text-destructive"
          >
            <X className="h-3.5 w-3.5" />
          </button>
        </div>
      </TooltipTrigger>
      {canToggleDisabled && (
        <TooltipContent side="bottom">
          {isDisabled ? "Click to enable for future queries" : "Click to skip in future queries"}
        </TooltipContent>
      )}
    </Tooltip>
  );
}

export function ModelSelector({
  selectedInstances,
  onInstancesChange,
  availableModels,
  isLoading: isLoadingProp,
  maxModels = 10,
  onInstanceParametersChange,
  onInstanceLabelChange,
  disabledInstances = [],
  onDisabledInstancesChange,
  hasMessages = false,
  task,
}: ModelSelectorProps) {
  const [pickerOpen, setPickerOpen] = useState(false);

  const sensors = useSensors(
    useSensor(PointerSensor, {
      activationConstraint: {
        distance: 8,
      },
    }),
    useSensor(KeyboardSensor, {
      coordinateGetter: sortableKeyboardCoordinates,
    })
  );

  // Get instance IDs for SortableContext
  const instanceIds = selectedInstances.map((i) => i.id);

  const handleDragEnd = (event: DragEndEvent) => {
    const { active, over } = event;

    if (over && active.id !== over.id) {
      const oldIndex = instanceIds.indexOf(active.id as string);
      const newIndex = instanceIds.indexOf(over.id as string);
      onInstancesChange(arrayMove(selectedInstances, oldIndex, newIndex));
    }
  };

  const handleRemoveInstance = (instanceId: string, e: React.MouseEvent) => {
    e.stopPropagation();
    onInstancesChange(selectedInstances.filter((i) => i.id !== instanceId));
    // Also remove from disabled if present
    if (disabledInstances.includes(instanceId)) {
      onDisabledInstancesChange?.(disabledInstances.filter((id) => id !== instanceId));
    }
  };

  const handleToggleDisabled = (instanceId: string) => {
    if (!onDisabledInstancesChange || !hasMessages) return;

    if (disabledInstances.includes(instanceId)) {
      onDisabledInstancesChange(disabledInstances.filter((id) => id !== instanceId));
    } else {
      onDisabledInstancesChange([...disabledInstances, instanceId]);
    }
  };

  // Handle adding a model from the picker
  // This creates a new instance for the model
  const handleModelsFromPicker = (modelIds: string[]) => {
    // Find which models were added (new ones not in current instances)
    const currentModelIds = new Set(selectedInstances.map((i) => i.modelId));

    // For removed models, filter out their instances
    const newInstances = selectedInstances.filter((i) => modelIds.includes(i.modelId));

    // For added models, create new instances
    for (const modelId of modelIds) {
      if (!currentModelIds.has(modelId)) {
        const instanceId = createInstanceId(modelId, newInstances);
        newInstances.push({
          id: instanceId,
          modelId,
        });
      }
    }

    onInstancesChange(newInstances);
  };

  // Handle adding a duplicate instance of an existing model (from picker - blank instance)
  const handleAddDuplicate = (modelId: string) => {
    const instanceId = createInstanceId(modelId, selectedInstances);
    onInstancesChange([
      ...selectedInstances,
      {
        id: instanceId,
        modelId,
      },
    ]);
  };

  // Handle duplicating an existing instance with all its settings
  const handleDuplicateInstance = (instance: ModelInstance) => {
    const instanceId = createInstanceId(instance.modelId, selectedInstances);

    // Create a label for the copy
    const sourceLabel = instance.label || getModelDisplayName(instance.modelId);
    const copyLabel = sourceLabel.includes("(Copy)")
      ? sourceLabel.replace(/\(Copy( \d+)?\)/, (_, num) => `(Copy ${(parseInt(num) || 1) + 1})`)
      : `${sourceLabel} (Copy)`;

    onInstancesChange([
      ...selectedInstances,
      {
        id: instanceId,
        modelId: instance.modelId,
        label: copyLabel,
        // Deep copy parameters to avoid shared references
        parameters: instance.parameters ? { ...instance.parameters } : undefined,
      },
    ]);
  };

  const isLoading = isLoadingProp ?? false;
  const canToggleDisabled = hasMessages && !!onDisabledInstancesChange;

  // Convert instances to model IDs for the picker (for backward compatibility)
  const selectedModelIds = [...new Set(selectedInstances.map((i) => i.modelId))];

  return (
    <div className="flex items-center gap-2 min-w-0">
      {/* Horizontally scrollable chip container */}
      <div className="flex flex-wrap items-center gap-2 min-w-0">
        <TooltipProvider>
          <DndContext
            sensors={sensors}
            collisionDetection={closestCenter}
            onDragEnd={handleDragEnd}
          >
            <SortableContext items={instanceIds} strategy={horizontalListSortingStrategy}>
              {selectedInstances.map((instance) => {
                const isDisabled = disabledInstances.includes(instance.id);
                // Use instance label if set, otherwise use model display name
                const instanceLabel = getInstanceLabel(instance);
                const displayName =
                  instanceLabel !== instance.modelId
                    ? instanceLabel
                    : getModelDisplayName(instance.modelId);

                return (
                  <SortableInstanceChip
                    key={instance.id}
                    instance={instance}
                    displayName={displayName}
                    isDisabled={isDisabled}
                    canToggleDisabled={canToggleDisabled}
                    onToggleDisabled={handleToggleDisabled}
                    onRemove={handleRemoveInstance}
                    onParametersChange={onInstanceParametersChange}
                    onLabelChange={onInstanceLabelChange}
                    onDuplicate={() => handleDuplicateInstance(instance)}
                  />
                );
              })}
            </SortableContext>
          </DndContext>
        </TooltipProvider>
      </div>

      {isLoading ? (
        <div className="flex items-center gap-2 text-sm text-muted-foreground shrink-0">
          <Spinner size="sm" />
          <span>Loading models...</span>
        </div>
      ) : availableModels.length > 0 ? (
        <Button
          variant="outline"
          size="sm"
          className="h-7 sm:h-8 gap-1 sm:gap-1.5 rounded-lg border-dashed px-2 sm:px-3 text-xs sm:text-sm text-muted-foreground hover:text-foreground shrink-0"
          onClick={() => setPickerOpen(true)}
        >
          <Plus className="h-3.5 sm:h-4 w-3.5 sm:w-4" />
          {selectedInstances.length === 0 ? "Select Model" : "Add"}
        </Button>
      ) : null}

      {selectedInstances.length === 0 && !isLoading && availableModels.length === 0 && (
        <span className="text-sm text-muted-foreground">No models available</span>
      )}

      {/* Model Picker Modal */}
      <ModelPicker
        open={pickerOpen}
        onClose={() => setPickerOpen(false)}
        selectedModels={selectedModelIds}
        onModelsChange={handleModelsFromPicker}
        availableModels={availableModels as ModelInfo[]}
        maxModels={maxModels}
        onAddDuplicate={handleAddDuplicate}
        allowDuplicates={true}
        task={task}
      />
    </div>
  );
}
