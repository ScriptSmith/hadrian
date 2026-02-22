import { useQuery } from "@tanstack/react-query";
import { Database, Plus, X } from "lucide-react";
import { useState, useCallback } from "react";

import { vectorStoreListOptions } from "@/api/generated/@tanstack/react-query.gen";
import type { VectorStore, VectorStoreOwnerType } from "@/api/generated/types.gen";
import { Badge } from "@/components/Badge/Badge";
import { Button } from "@/components/Button/Button";
import { Spinner } from "@/components/Spinner/Spinner";
import { VectorStorePicker } from "./VectorStorePicker";
import { cn } from "@/utils/cn";

export interface VectorStoreSelectorProps {
  /** Currently selected vector store IDs */
  selectedIds: string[];
  /** Callback when selection changes */
  onIdsChange: (ids: string[]) => void;
  /** Owner type to filter vector stores (optional - omit to show all accessible) */
  ownerType?: VectorStoreOwnerType;
  /** Owner ID to filter vector stores (optional - omit to show all accessible) */
  ownerId?: string;
  /** Maximum number of stores that can be selected */
  maxStores?: number;
  /** Optional className for the container */
  className?: string;
  /** Whether the selector is disabled */
  disabled?: boolean;
  /** Compact mode - only show add button when empty */
  compact?: boolean;
}

export function VectorStoreSelector({
  selectedIds,
  onIdsChange,
  ownerType,
  ownerId,
  maxStores = 10,
  className,
  disabled = false,
  compact = false,
}: VectorStoreSelectorProps) {
  const [pickerOpen, setPickerOpen] = useState(false);

  // Fetch available vector stores
  // When ownerType/ownerId are provided, filter by owner
  // When omitted, fetch all accessible stores
  const { data: vectorStoresResponse, isLoading } = useQuery({
    ...vectorStoreListOptions({
      query: {
        ...(ownerType && ownerId ? { owner_type: ownerType, owner_id: ownerId } : {}),
        limit: 100,
      },
    }),
  });

  const availableStores = vectorStoresResponse?.data || [];

  // Get store info for selected IDs
  const selectedStores = selectedIds
    .map((id) => availableStores.find((s) => s.id === id))
    .filter((s): s is VectorStore => s !== undefined);

  const handleRemove = useCallback(
    (id: string) => {
      onIdsChange(selectedIds.filter((selectedId) => selectedId !== id));
    },
    [selectedIds, onIdsChange]
  );

  const handleOpenPicker = useCallback(() => {
    if (!disabled) {
      setPickerOpen(true);
    }
  }, [disabled]);

  const handleClosePicker = useCallback(() => {
    setPickerOpen(false);
  }, []);

  // Compact mode: just show a button that indicates count
  if (compact && selectedIds.length === 0) {
    return (
      <>
        <Button
          variant="outline"
          size="sm"
          onClick={handleOpenPicker}
          disabled={disabled || isLoading}
          className={cn("gap-1.5", className)}
        >
          <Database className="h-4 w-4" />
          <span>Add Knowledge</span>
          {isLoading && <Spinner className="h-3 w-3 ml-1" />}
        </Button>
        <VectorStorePicker
          open={pickerOpen}
          onClose={handleClosePicker}
          selectedIds={selectedIds}
          onIdsChange={onIdsChange}
          availableStores={availableStores}
          maxStores={maxStores}
          isLoading={isLoading}
        />
      </>
    );
  }

  return (
    <>
      <div className={cn("flex flex-wrap items-center gap-2", className)}>
        {/* Selected vector stores as badges */}
        {selectedStores.map((store) => (
          <Badge key={store.id} variant="secondary" className="gap-1.5 pr-1">
            <Database className="h-3 w-3" />
            <span className="font-medium max-w-[150px] truncate">{store.name || "Unnamed"}</span>
            <button
              onClick={(e) => {
                e.stopPropagation();
                handleRemove(store.id);
              }}
              disabled={disabled}
              className="ml-0.5 rounded-full p-0.5 text-muted-foreground hover:bg-muted hover:text-destructive transition-colors disabled:opacity-50"
              aria-label={`Remove ${store.name}`}
            >
              <X className="h-3 w-3" />
            </button>
          </Badge>
        ))}

        {/* Show badges for IDs that weren't found (might still be loading) */}
        {selectedIds
          .filter((id) => !selectedStores.find((s) => s.id === id))
          .map((id) => (
            <Badge key={id} variant="outline" className="gap-1.5 pr-1">
              <Database className="h-3 w-3" />
              <span className="font-medium text-muted-foreground">Loading...</span>
              <button
                onClick={(e) => {
                  e.stopPropagation();
                  handleRemove(id);
                }}
                disabled={disabled}
                className="ml-0.5 rounded-full p-0.5 text-muted-foreground hover:bg-muted hover:text-destructive transition-colors disabled:opacity-50"
                aria-label="Remove"
              >
                <X className="h-3 w-3" />
              </button>
            </Badge>
          ))}

        {/* Add button */}
        {selectedIds.length < maxStores && (
          <Button
            variant="outline"
            size="sm"
            onClick={handleOpenPicker}
            disabled={disabled || isLoading}
            className="gap-1 h-7 px-2"
          >
            <Plus className="h-3.5 w-3.5" />
            {selectedIds.length === 0 ? "Add Knowledge" : "Add"}
            {isLoading && <Spinner className="h-3 w-3 ml-1" />}
          </Button>
        )}
      </div>

      <VectorStorePicker
        open={pickerOpen}
        onClose={handleClosePicker}
        selectedIds={selectedIds}
        onIdsChange={onIdsChange}
        availableStores={availableStores}
        maxStores={maxStores}
        isLoading={isLoading}
      />
    </>
  );
}
