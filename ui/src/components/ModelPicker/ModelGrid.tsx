import { Check, Star, Pin, Brain, Wrench, Eye, Braces, Scale } from "lucide-react";
import { useRef, useEffect, memo, useState, useCallback } from "react";
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

/** Minimum card width thresholds for determining column count from container width */
const MIN_CARD_WIDTH = 280;
const GRID_GAP = 12; // gap-3 = 12px
const GRID_PADDING = 16; // px-2 = 8px each side

/** Estimated row height for virtualization (card height + gap) */
const ESTIMATED_ROW_HEIGHT = 130;

/** Number of extra rows to render above/below viewport */
const OVERSCAN_COUNT = 2;

/** Maximum columns to display */
const MAX_COLUMNS = 4;

// =============================================================================
// ModelCard - Memoized individual model card for performance
// =============================================================================

interface ModelCardProps {
  model: ModelInfo;
  index: number;
  isSelected: boolean;
  isFocused: boolean;
  isFavorite: boolean;
  isDefault: boolean;
  isDisabled: boolean;
  /** Whether this card's details are currently shown in the side panel */
  isShowingDetails: boolean;
  onToggleModel: (modelId: string) => void;
  onToggleFavorite: (modelId: string, e: React.MouseEvent) => void;
  onToggleDefault: (modelId: string, e: React.MouseEvent) => void;
  /** Called when user clicks the info button to show details */
  onShowDetails: (model: ModelInfo) => void;
}

/** Custom comparator for ModelCard - only re-render when display-affecting props change */
function areModelCardPropsEqual(prev: ModelCardProps, next: ModelCardProps): boolean {
  return (
    prev.model.id === next.model.id &&
    prev.index === next.index &&
    prev.isSelected === next.isSelected &&
    prev.isFocused === next.isFocused &&
    prev.isFavorite === next.isFavorite &&
    prev.isDefault === next.isDefault &&
    prev.isDisabled === next.isDisabled &&
    prev.isShowingDetails === next.isShowingDetails &&
    // Callbacks should be stable (wrapped in useCallback), but check identity
    prev.onToggleModel === next.onToggleModel &&
    prev.onToggleFavorite === next.onToggleFavorite &&
    prev.onToggleDefault === next.onToggleDefault &&
    prev.onShowDetails === next.onShowDetails
  );
}

const ModelCard = memo(function ModelCard({
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
}: ModelCardProps) {
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
        "relative flex flex-col rounded-lg border p-3.5 text-left text-sm cursor-pointer",
        "hover:border-primary/50 hover:bg-muted/50",
        "transition-colors",
        isFocused && "ring-2 ring-primary ring-offset-1",
        isSelected && "border-primary bg-primary/5",
        isShowingDetails && "ring-2 ring-blue-500/50",
        isDisabled && "opacity-40 cursor-not-allowed hover:border-border hover:bg-transparent"
      )}
    >
      {/* Model name */}
      <div className="font-semibold truncate" title={model.id}>
        {getModelName(model.id)}
      </div>

      {/* Badges row: Provider + source + capabilities */}
      <div className="flex items-center gap-1.5 flex-wrap mt-2">
        <span className={cn("rounded px-2 py-0.5 text-xs font-medium", providerInfo.color)}>
          {providerInfo.label}
        </span>
        {dynamicScope === "user" && (
          <span className="rounded px-1.5 py-0.5 text-[10px] font-medium bg-emerald-500/10 text-emerald-700 dark:text-emerald-400 border border-emerald-500/20">
            User
          </span>
        )}
        {dynamicScope === "org" && (
          <span className="rounded px-1.5 py-0.5 text-[10px] font-medium bg-blue-500/10 text-blue-700 dark:text-blue-400 border border-blue-500/20">
            Org
          </span>
        )}
        {dynamicScope === "project" && (
          <span className="rounded px-1.5 py-0.5 text-[10px] font-medium bg-amber-500/10 text-amber-700 dark:text-amber-400 border border-amber-500/20">
            Project
          </span>
        )}
        {capabilities?.reasoning && (
          <CapabilityBadge icon={Brain} label="Reasoning" color="purple" />
        )}
        {capabilities?.tool_call && (
          <CapabilityBadge icon={Wrench} label="Tool Calling" color="green" />
        )}
        {capabilities?.vision && <CapabilityBadge icon={Eye} label="Vision" color="cyan" />}
        {capabilities?.structured_output && (
          <CapabilityBadge icon={Braces} label="Structured Output (JSON)" color="amber" />
        )}
        {model.open_weights && <CapabilityBadge icon={Scale} label="Open Weights" color="indigo" />}
      </div>

      {/* Metadata row */}
      <div className="flex items-center gap-3 text-xs text-muted-foreground mt-2">
        {contextLength && <span>{formatContextLength(contextLength)} ctx</span>}
        {catalogPricing && catalogPricing.input > 0 && (
          <>
            {contextLength && <span className="h-3 w-px bg-border" />}
            <span className="font-medium text-primary" title="Price per 1M tokens (input/output)">
              {formatCatalogPricing(catalogPricing.input)}/
              {formatCatalogPricing(catalogPricing.output)}
            </span>
          </>
        )}
        {catalogPricing && catalogPricing.input === 0 && (
          <>
            {contextLength && <span className="h-3 w-px bg-border" />}
            <span className="font-medium text-success">Free</span>
          </>
        )}
      </div>

      {/* Actions row: favorite, default, select */}
      <div className="flex items-center gap-1.5 mt-2 pt-2 border-t">
        <Tooltip>
          <TooltipTrigger asChild>
            {/* eslint-disable-next-line jsx-a11y/click-events-have-key-events, jsx-a11y/no-static-element-interactions -- intentionally non-interactive in a11y tree to avoid nested-interactive inside role="option" */}
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
                  : "text-muted-foreground/30 hover:text-yellow-500"
              )}
            >
              <Star className={cn("h-4 w-4", isFavorite && "fill-current")} />
            </span>
          </TooltipTrigger>
          <TooltipContent side="bottom">
            {isFavorite ? "Remove from favorites" : "Add to favorites"}
          </TooltipContent>
        </Tooltip>

        <Tooltip>
          <TooltipTrigger asChild>
            {/* eslint-disable-next-line jsx-a11y/click-events-have-key-events, jsx-a11y/no-static-element-interactions -- intentionally non-interactive in a11y tree to avoid nested-interactive inside role="option" */}
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
                  : "text-muted-foreground/30 hover:text-blue-500"
              )}
            >
              <Pin className={cn("h-4 w-4", isDefault && "fill-current")} />
            </span>
          </TooltipTrigger>
          <TooltipContent side="bottom">
            {isDefault ? "Remove from defaults" : "Set as default"}
          </TooltipContent>
        </Tooltip>

        <div className="flex-1" />

        {/* Selection checkbox */}
        <div
          className={cn(
            "flex h-5 w-5 items-center justify-center rounded border transition-colors",
            isSelected
              ? "border-primary bg-primary text-primary-foreground"
              : "border-muted-foreground/30 bg-background"
          )}
        >
          {isSelected && <Check className="h-3 w-3" />}
        </div>
      </div>
    </div>
  );
}, areModelCardPropsEqual);

// =============================================================================
// useColumnCount - Hook to detect column count from actual container width
// Uses ResizeObserver so the virtualizer and grid always agree on column count
// =============================================================================

function getColumnsForWidth(containerWidth: number): number {
  const available = containerWidth - GRID_PADDING;
  // How many columns fit? n columns need: n * MIN_CARD_WIDTH + (n-1) * GRID_GAP
  const cols = Math.floor((available + GRID_GAP) / (MIN_CARD_WIDTH + GRID_GAP));
  return Math.max(1, Math.min(cols, MAX_COLUMNS));
}

function useColumnCount(containerRef: React.RefObject<HTMLElement | null>): number {
  const [columnCount, setColumnCount] = useState(1);

  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;

    const observer = new ResizeObserver((entries) => {
      const width = entries[0]?.contentRect.width ?? 0;
      setColumnCount(getColumnsForWidth(width));
    });
    // Set initial value
    setColumnCount(getColumnsForWidth(el.clientWidth));
    observer.observe(el);
    return () => observer.disconnect();
  }, [containerRef]);

  return columnCount;
}

// =============================================================================
// ModelGrid - Virtualized grid of model cards
// =============================================================================

interface ModelGridProps {
  models: ModelInfo[];
  /** Set of selected model IDs for O(1) lookup */
  selectedSet: Set<string>;
  /** Set of favorite model IDs for O(1) lookup */
  favoriteSet: Set<string>;
  /** Set of default model IDs for O(1) lookup */
  defaultSet: Set<string>;
  maxModels: number;
  /** Index of keyboard-focused item (for arrow key navigation) */
  focusedIndex: number;
  /** When true, scroll the focused item into view (used for keyboard navigation) */
  shouldScrollToFocused: boolean;
  /** ID of model currently shown in details panel */
  detailModelId?: string;
  onToggleModel: (modelId: string) => void;
  onToggleFavorite: (modelId: string, e: React.MouseEvent) => void;
  onToggleDefault: (modelId: string, e: React.MouseEvent) => void;
  /** Called when user clicks info button to show model details */
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
  const columnCount = useColumnCount(scrollContainerRef);

  // Calculate number of rows
  const rowCount = Math.ceil(models.length / columnCount);

  // Set up row virtualizer
  const virtualizer = useVirtualizer({
    count: rowCount,
    getScrollElement: () => scrollContainerRef.current,
    estimateSize: () => ESTIMATED_ROW_HEIGHT,
    overscan: OVERSCAN_COUNT,
  });

  // Scroll focused item into view using virtualizer
  const scrollToFocused = useCallback(
    (index: number) => {
      const rowIndex = Math.floor(index / columnCount);
      virtualizer.scrollToIndex(rowIndex, { align: "auto" });
    },
    [columnCount, virtualizer]
  );

  useEffect(() => {
    if (shouldScrollToFocused && focusedIndex >= 0) {
      scrollToFocused(focusedIndex);
    }
  }, [focusedIndex, shouldScrollToFocused, scrollToFocused]);

  // Stable callback for showing details
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
      {/* Container with total height for proper scrollbar */}
      <div className="relative w-full" style={{ height: virtualizer.getTotalSize() }}>
        {/* Render only visible rows */}
        {virtualizer.getVirtualItems().map((virtualRow) => {
          const rowStartIndex = virtualRow.index * columnCount;
          const rowModels = models.slice(rowStartIndex, rowStartIndex + columnCount);

          return (
            <div
              key={virtualRow.key}
              ref={virtualizer.measureElement}
              data-index={virtualRow.index}
              className="absolute left-0 right-0 px-2"
              style={{ transform: `translateY(${virtualRow.start}px)` }}
            >
              <div
                className="grid gap-3 py-1.5"
                style={{ gridTemplateColumns: `repeat(${columnCount}, minmax(0, 1fr))` }}
              >
                {rowModels.map((model, colIndex) => {
                  const modelIndex = rowStartIndex + colIndex;
                  const isSelected = selectedSet.has(model.id);
                  const isFocused = modelIndex === focusedIndex;
                  const isFavorite = favoriteSet.has(model.id);
                  const isDefault = defaultSet.has(model.id);
                  const isDisabled = !isSelected && selectedCount >= maxModels;
                  const isShowingDetails = detailModelId === model.id;

                  return (
                    <ModelCard
                      key={model.id}
                      model={model}
                      index={modelIndex}
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
                  );
                })}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
