import { Check, Star, Pin, Brain, Wrench, Eye, Braces, Scale } from "lucide-react";
import { useRef, useEffect, memo, useCallback } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";

import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/Tooltip/Tooltip";
import { cn } from "@/utils/cn";

import { CapabilityBadge } from "./CapabilityBadge";
import type { ModelInfo } from "./model-utils";
import {
  getModelName,
  getProviderFromId,
  getProviderInfo,
  getDynamicScope,
  formatContextLength,
  formatCatalogPricing,
} from "./model-utils";

// =============================================================================
// Constants for virtualization
// =============================================================================

/** Estimated row height for virtualization (two-line row + border) */
const ESTIMATED_ROW_HEIGHT = 54;

/** Number of extra rows to render above/below viewport */
const OVERSCAN_COUNT = 5;

// =============================================================================
// ModelRow - Memoized compact row for each model
// =============================================================================

interface ModelRowProps {
  model: ModelInfo;
  index: number;
  isSelected: boolean;
  isFocused: boolean;
  isFavorite: boolean;
  isDefault: boolean;
  isDisabled: boolean;
  isShowingDetails: boolean;
  onToggleModel: (modelId: string) => void;
  onToggleFavorite: (modelId: string, e: React.MouseEvent) => void;
  onToggleDefault: (modelId: string, e: React.MouseEvent) => void;
  onShowDetails: (model: ModelInfo) => void;
}

function areModelRowPropsEqual(prev: ModelRowProps, next: ModelRowProps): boolean {
  return (
    prev.model.id === next.model.id &&
    prev.index === next.index &&
    prev.isSelected === next.isSelected &&
    prev.isFocused === next.isFocused &&
    prev.isFavorite === next.isFavorite &&
    prev.isDefault === next.isDefault &&
    prev.isDisabled === next.isDisabled &&
    prev.isShowingDetails === next.isShowingDetails &&
    prev.onToggleModel === next.onToggleModel &&
    prev.onToggleFavorite === next.onToggleFavorite &&
    prev.onToggleDefault === next.onToggleDefault &&
    prev.onShowDetails === next.onShowDetails
  );
}

const ModelRow = memo(function ModelRow({
  model,
  index,
  isSelected,
  isFocused,
  isFavorite,
  isDefault,
  isDisabled,
  isShowingDetails,
  onToggleModel,
  onToggleFavorite,
  onToggleDefault,
  onShowDetails,
}: ModelRowProps) {
  const provider = getProviderFromId(model.id);
  const dynamicScope = model.source === "dynamic" ? getDynamicScope(model.id) : undefined;
  const providerInfo = getProviderInfo(provider, model.source);
  const contextLength = model.context_length;
  const capabilities = model.capabilities;
  const catalogPricing = model.catalog_pricing;

  return (
    // eslint-disable-next-line jsx-a11y/click-events-have-key-events -- keyboard handled at listbox level by ModelPicker
    <div
      data-index={index}
      tabIndex={-1}
      onClick={() => !isDisabled && onToggleModel(model.id)}
      onMouseEnter={() => onShowDetails(model)}
      role="option"
      aria-selected={isSelected}
      aria-disabled={isDisabled || undefined}
      className={cn(
        "flex items-start gap-3 px-3 py-2 text-sm cursor-pointer border-b border-border/50",
        "hover:bg-muted/50",
        "transition-colors",
        isFocused && "ring-2 ring-inset ring-primary",
        isSelected && "bg-primary/5",
        isShowingDetails && "bg-blue-500/5",
        isDisabled && "opacity-40 cursor-not-allowed hover:bg-transparent"
      )}
    >
      {/* Checkbox */}
      <div
        className={cn(
          "flex h-4.5 w-4.5 shrink-0 items-center justify-center rounded border transition-colors mt-0.5",
          isSelected
            ? "border-primary bg-primary text-primary-foreground"
            : "border-muted-foreground/30 bg-background"
        )}
      >
        {isSelected && <Check className="h-3 w-3" />}
      </div>

      {/* Two-line content */}
      <div className="flex flex-col gap-1 min-w-0 flex-1">
        {/* Line 1: Model name */}
        <span className="font-medium truncate" title={model.id}>
          {getModelName(model.id)}
        </span>

        {/* Line 2: Badges, capabilities, metadata */}
        <div className="flex items-center gap-1.5 flex-wrap">
          {/* Provider badge */}
          <span
            className={cn(
              "shrink-0 rounded px-1.5 py-0.5 text-[10px] font-medium leading-tight",
              providerInfo.color
            )}
          >
            {providerInfo.label}
          </span>

          {/* Scope badge for dynamic models */}
          {dynamicScope === "user" && (
            <span className="shrink-0 rounded px-1.5 py-0.5 text-[10px] font-medium bg-emerald-500/10 text-emerald-700 dark:text-emerald-400">
              User
            </span>
          )}
          {dynamicScope === "org" && (
            <span className="shrink-0 rounded px-1.5 py-0.5 text-[10px] font-medium bg-blue-500/10 text-blue-700 dark:text-blue-400">
              Org
            </span>
          )}
          {dynamicScope === "project" && (
            <span className="shrink-0 rounded px-1.5 py-0.5 text-[10px] font-medium bg-amber-500/10 text-amber-700 dark:text-amber-400">
              Project
            </span>
          )}

          {/* Capability icons */}
          {capabilities?.reasoning && (
            <CapabilityBadge icon={Brain} label="Reasoning" color="purple" className="p-0.5" />
          )}
          {capabilities?.tool_call && (
            <CapabilityBadge icon={Wrench} label="Tool Calling" color="green" className="p-0.5" />
          )}
          {capabilities?.vision && (
            <CapabilityBadge icon={Eye} label="Vision" color="cyan" className="p-0.5" />
          )}
          {capabilities?.structured_output && (
            <CapabilityBadge
              icon={Braces}
              label="Structured Output (JSON)"
              color="amber"
              className="p-0.5"
            />
          )}
          {model.open_weights && (
            <CapabilityBadge icon={Scale} label="Open Weights" color="indigo" className="p-0.5" />
          )}

          {/* Context length */}
          {contextLength && (
            <span className="shrink-0 text-[10px] text-muted-foreground tabular-nums">
              {formatContextLength(contextLength)} ctx
            </span>
          )}

          {/* Pricing */}
          {catalogPricing && catalogPricing.input > 0 && (
            <span
              className="shrink-0 text-[10px] font-medium text-primary tabular-nums"
              title="Price per 1M tokens (input/output)"
            >
              {formatCatalogPricing(catalogPricing.input)}/
              {formatCatalogPricing(catalogPricing.output)}
            </span>
          )}
          {catalogPricing && catalogPricing.input === 0 && (
            <span className="shrink-0 text-[10px] font-medium text-success">Free</span>
          )}
        </div>
      </div>

      {/* Favorite + Default */}
      <div className="flex items-center gap-0.5 shrink-0 mt-0.5">
        <Tooltip>
          <TooltipTrigger asChild>
            {/* eslint-disable-next-line jsx-a11y/click-events-have-key-events, jsx-a11y/no-static-element-interactions */}
            <span
              onClick={(e) => {
                e.stopPropagation();
                onToggleFavorite(model.id, e);
              }}
              aria-label={isFavorite ? "Remove from favorites" : "Add to favorites"}
              className={cn(
                "cursor-pointer rounded p-1 transition-colors",
                isFavorite
                  ? "text-yellow-500 hover:text-yellow-600"
                  : "text-muted-foreground/20 hover:text-yellow-500"
              )}
            >
              <Star className={cn("h-3.5 w-3.5", isFavorite && "fill-current")} />
            </span>
          </TooltipTrigger>
          <TooltipContent side="bottom">
            {isFavorite ? "Remove from favorites" : "Add to favorites"}
          </TooltipContent>
        </Tooltip>

        <Tooltip>
          <TooltipTrigger asChild>
            {/* eslint-disable-next-line jsx-a11y/click-events-have-key-events, jsx-a11y/no-static-element-interactions */}
            <span
              onClick={(e) => {
                e.stopPropagation();
                onToggleDefault(model.id, e);
              }}
              aria-label={isDefault ? "Remove from defaults" : "Set as default"}
              className={cn(
                "cursor-pointer rounded p-1 transition-colors",
                isDefault
                  ? "text-blue-500 hover:text-blue-700"
                  : "text-muted-foreground/20 hover:text-blue-500"
              )}
            >
              <Pin className={cn("h-3.5 w-3.5", isDefault && "fill-current")} />
            </span>
          </TooltipTrigger>
          <TooltipContent side="bottom">
            {isDefault ? "Remove from defaults" : "Set as default"}
          </TooltipContent>
        </Tooltip>
      </div>
    </div>
  );
}, areModelRowPropsEqual);

// =============================================================================
// ModelGrid - Virtualized list of model rows
// =============================================================================

interface ModelGridProps {
  models: ModelInfo[];
  selectedSet: Set<string>;
  favoriteSet: Set<string>;
  defaultSet: Set<string>;
  maxModels: number;
  focusedIndex: number;
  shouldScrollToFocused: boolean;
  detailModelId?: string;
  onToggleModel: (modelId: string) => void;
  onToggleFavorite: (modelId: string, e: React.MouseEvent) => void;
  onToggleDefault: (modelId: string, e: React.MouseEvent) => void;
  onShowDetails?: (model: ModelInfo) => void;
}

export function ModelGrid({
  models,
  selectedSet,
  favoriteSet,
  defaultSet,
  maxModels,
  focusedIndex,
  shouldScrollToFocused,
  detailModelId,
  onToggleModel,
  onToggleFavorite,
  onToggleDefault,
  onShowDetails,
}: ModelGridProps) {
  const selectedCount = selectedSet.size;
  const scrollContainerRef = useRef<HTMLDivElement>(null);

  const virtualizer = useVirtualizer({
    count: models.length,
    getScrollElement: () => scrollContainerRef.current,
    estimateSize: () => ESTIMATED_ROW_HEIGHT,
    overscan: OVERSCAN_COUNT,
  });

  // Scroll focused item into view
  useEffect(() => {
    if (shouldScrollToFocused && focusedIndex >= 0) {
      virtualizer.scrollToIndex(focusedIndex, { align: "auto" });
    }
  }, [focusedIndex, shouldScrollToFocused, virtualizer]);

  const handleShowDetails = useCallback(
    (model: ModelInfo) => {
      onShowDetails?.(model);
    },
    [onShowDetails]
  );

  if (models.length === 0) {
    return (
      <div className="flex items-center justify-center py-12 text-sm text-muted-foreground">
        No models found
      </div>
    );
  }

  return (
    <div
      ref={scrollContainerRef}
      className="h-full overflow-y-auto"
      role="listbox"
      aria-label="Available models"
      tabIndex={0}
    >
      <div className="relative w-full" style={{ height: virtualizer.getTotalSize() }}>
        {virtualizer.getVirtualItems().map((virtualRow) => {
          const model = models[virtualRow.index];
          const isSelected = selectedSet.has(model.id);
          const isFocused = virtualRow.index === focusedIndex;
          const isFavorite = favoriteSet.has(model.id);
          const isDefault = defaultSet.has(model.id);
          const isDisabled = !isSelected && selectedCount >= maxModels;
          const isShowingDetails = detailModelId === model.id;

          return (
            <div
              key={virtualRow.key}
              ref={virtualizer.measureElement}
              data-index={virtualRow.index}
              className="absolute left-0 right-0"
              style={{ transform: `translateY(${virtualRow.start}px)` }}
            >
              <ModelRow
                model={model}
                index={virtualRow.index}
                isSelected={isSelected}
                isFocused={isFocused}
                isFavorite={isFavorite}
                isDefault={isDefault}
                isDisabled={isDisabled}
                isShowingDetails={isShowingDetails}
                onToggleModel={onToggleModel}
                onToggleFavorite={onToggleFavorite}
                onToggleDefault={onToggleDefault}
                onShowDetails={handleShowDetails}
              />
            </div>
          );
        })}
      </div>
    </div>
  );
}
