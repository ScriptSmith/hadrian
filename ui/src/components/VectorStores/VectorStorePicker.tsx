import { Check, Search, Database, FileText, X } from "lucide-react";
import { useState, useMemo, useEffect, useRef } from "react";
import { createPortal } from "react-dom";

import type { VectorStore } from "@/api/generated/types.gen";
import { Button } from "@/components/Button/Button";
import { Spinner } from "@/components/Spinner/Spinner";
import { cn } from "@/utils/cn";

interface VectorStorePickerProps {
  open: boolean;
  onClose: () => void;
  selectedIds: string[];
  onIdsChange: (ids: string[]) => void;
  availableStores: VectorStore[];
  maxStores?: number;
  isLoading?: boolean;
}

function getStatusInfo(status: string): { color: string; label: string } {
  const statuses: Record<string, { color: string; label: string }> = {
    completed: { color: "bg-success/10 text-success", label: "Ready" },
    in_progress: {
      color: "bg-amber-500/10 text-amber-800 dark:text-amber-400",
      label: "Processing",
    },
    expired: { color: "bg-gray-500/10 text-gray-600", label: "Expired" },
  };
  return statuses[status] || { color: "bg-gray-500/10 text-gray-600", label: status };
}

function formatFileCount(counts: {
  in_progress: number;
  completed: number;
  failed: number;
  cancelled: number;
  total: number;
}): string {
  if (counts.total === 0) return "No files";
  if (counts.total === 1) return "1 file";
  return `${counts.total} files`;
}

export function VectorStorePicker({
  open,
  onClose,
  selectedIds,
  onIdsChange,
  availableStores,
  maxStores = 10,
  isLoading = false,
}: VectorStorePickerProps) {
  const [searchQuery, setSearchQuery] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const [focusedIndex, setFocusedIndex] = useState(0);

  useEffect(() => {
    if (open) {
      setSearchQuery("");
      setFocusedIndex(0);
      setTimeout(() => inputRef.current?.focus(), 0);
    }
  }, [open]);

  // Handle escape key at document level
  useEffect(() => {
    if (!open) return;
    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("keydown", handleEscape);
    return () => document.removeEventListener("keydown", handleEscape);
  }, [open, onClose]);

  const { selectedStoreInfos, unselectedStores, flatStores, totalCount } = useMemo(() => {
    const query = searchQuery.toLowerCase();
    const filtered = availableStores.filter((store) => {
      const name = store.name?.toLowerCase() || "";
      const description = store.description?.toLowerCase() || "";
      const model = store.embedding_model?.toLowerCase() || "";
      return name.includes(query) || description.includes(query) || model.includes(query);
    });

    const selected = filtered.filter((s) => selectedIds.includes(s.id));
    const unselected = filtered.filter((s) => !selectedIds.includes(s.id));
    const flat = [...selected, ...unselected];

    return {
      selectedStoreInfos: selected,
      unselectedStores: unselected,
      flatStores: flat,
      totalCount: filtered.length,
    };
  }, [availableStores, searchQuery, selectedIds]);

  // Reset focused index when search changes
  useEffect(() => {
    setFocusedIndex(0);
  }, [searchQuery]);

  // Scroll focused item into view
  useEffect(() => {
    const el = listRef.current?.querySelector(`[data-index="${focusedIndex}"]`);
    el?.scrollIntoView({ block: "nearest" });
  }, [focusedIndex]);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    switch (e.key) {
      case "ArrowDown":
        e.preventDefault();
        setFocusedIndex((prev) => Math.min(prev + 1, flatStores.length - 1));
        break;
      case "ArrowUp":
        e.preventDefault();
        setFocusedIndex((prev) => Math.max(prev - 1, 0));
        break;
      case "Enter":
        e.preventDefault();
        if (flatStores[focusedIndex]) handleToggleStore(flatStores[focusedIndex].id);
        break;
    }
  };

  const handleToggleStore = (storeId: string) => {
    if (selectedIds.includes(storeId)) {
      onIdsChange(selectedIds.filter((id) => id !== storeId));
    } else if (selectedIds.length < maxStores) {
      onIdsChange([...selectedIds, storeId]);
    }
  };

  const renderStoreRow = (store: VectorStore, index: number) => {
    const isSelected = selectedIds.includes(store.id);
    const isFocused = index === focusedIndex;
    const isDisabled = !isSelected && selectedIds.length >= maxStores;
    const statusInfo = getStatusInfo(store.status);
    const fileCountText = formatFileCount(store.file_counts);

    return (
      <button
        key={store.id}
        data-index={index}
        onClick={() => !isDisabled && handleToggleStore(store.id)}
        onMouseEnter={() => setFocusedIndex(index)}
        disabled={isDisabled}
        className={cn(
          "flex w-full items-center gap-3 rounded-lg px-3 py-2.5 text-left text-sm transition-colors",
          isFocused ? "bg-accent text-accent-foreground" : "hover:bg-accent/50",
          isDisabled && "opacity-40 cursor-not-allowed"
        )}
      >
        {/* Selection indicator */}
        <span
          className={cn(
            "flex h-5 w-5 shrink-0 items-center justify-center rounded-md border transition-colors",
            isSelected
              ? "border-primary bg-primary text-primary-foreground"
              : "border-border bg-muted/50"
          )}
        >
          {isSelected && <Check className="h-3 w-3" />}
        </span>

        {/* Store info */}
        <div className="flex flex-1 flex-col min-w-0">
          <div className="flex items-center gap-2">
            <Database className="h-4 w-4 shrink-0 text-muted-foreground" />
            <span className="font-medium truncate">{store.name || "Unnamed"}</span>
            <span
              className={cn(
                "rounded px-1.5 py-0.5 text-[10px] font-medium shrink-0",
                statusInfo.color
              )}
            >
              {statusInfo.label}
            </span>
          </div>
          <div className="flex items-center gap-2 text-xs text-muted-foreground mt-0.5 pl-6">
            <span className="flex items-center gap-1">
              <FileText className="h-3 w-3" />
              {fileCountText}
            </span>
            <span className="text-muted-foreground/40">·</span>
            <span className="truncate">{store.embedding_model}</span>
          </div>
          {store.description && (
            <div className="text-xs text-muted-foreground mt-0.5 pl-6 truncate">
              {store.description}
            </div>
          )}
        </div>
      </button>
    );
  };

  if (!open) return null;

  let runningIndex = 0;

  return createPortal(
    <>
      {/* Backdrop */}
      <div
        className="fixed inset-0 z-50 bg-black/50 backdrop-blur-sm animate-in fade-in-0"
        onClick={onClose}
        aria-hidden="true"
      />

      {/* Dialog */}
      <div className="fixed left-1/2 top-[15%] z-50 w-full max-w-xl -translate-x-1/2 animate-in fade-in-0 zoom-in-95 slide-in-from-top-4">
        <div className="overflow-hidden rounded-xl border bg-popover shadow-2xl ring-1 ring-black/5">
          {/* Header with close button */}
          <div className="flex items-center justify-between border-b px-4 py-3">
            <h2 className="text-sm font-semibold">Select Knowledge Bases</h2>
            <Button variant="ghost" size="icon" onClick={onClose} className="h-7 w-7">
              <X className="h-4 w-4" />
              <span className="sr-only">Close</span>
            </Button>
          </div>

          {/* Search input */}
          <div className="flex items-center border-b px-4">
            <Search className="h-5 w-5 shrink-0 text-muted-foreground" aria-hidden="true" />
            <input
              ref={inputRef}
              type="text"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder="Search knowledge bases..."
              aria-label="Search knowledge bases"
              className="flex-1 bg-transparent px-4 py-3 text-sm outline-none placeholder:text-muted-foreground"
            />
            <div className="flex items-center gap-2">
              <span className="text-xs text-muted-foreground">
                {selectedIds.length}/{maxStores}
              </span>
            </div>
          </div>

          {/* Store list */}
          <div ref={listRef} className="max-h-[400px] overflow-y-auto p-2">
            {isLoading ? (
              <div className="flex items-center justify-center py-12">
                <Spinner className="h-6 w-6 text-muted-foreground" />
              </div>
            ) : totalCount === 0 ? (
              <div className="py-8 text-center text-sm text-muted-foreground">
                {searchQuery ? "No knowledge bases found" : "No knowledge bases available"}
              </div>
            ) : (
              <>
                {/* Selected section */}
                {selectedStoreInfos.length > 0 && (
                  <div className="mb-2">
                    <div className="px-2 py-1.5 text-xs font-semibold text-muted-foreground flex items-center gap-2">
                      <span className="rounded bg-primary/10 px-1.5 py-0.5 text-primary">
                        Selected
                      </span>
                      <span>{selectedStoreInfos.length}</span>
                    </div>
                    {selectedStoreInfos.map((store) => {
                      const idx = runningIndex++;
                      return renderStoreRow(store, idx);
                    })}
                  </div>
                )}

                {/* Available section */}
                {unselectedStores.length > 0 && (
                  <div className="mb-2">
                    {selectedStoreInfos.length > 0 && (
                      <div className="px-2 py-1.5 text-xs font-semibold text-muted-foreground flex items-center gap-2">
                        <span>Available</span>
                        <span className="text-muted-foreground">{unselectedStores.length}</span>
                      </div>
                    )}
                    {unselectedStores.map((store) => {
                      const idx = runningIndex++;
                      return renderStoreRow(store, idx);
                    })}
                  </div>
                )}
              </>
            )}
          </div>

          {/* Footer */}
          <div className="flex items-center justify-between border-t px-4 py-2 text-xs text-muted-foreground">
            <div className="flex items-center gap-4">
              <span className="flex items-center gap-1">
                <kbd className="h-4 rounded border bg-muted px-1 text-[10px]">↑</kbd>
                <kbd className="h-4 rounded border bg-muted px-1 text-[10px]">↓</kbd>
                navigate
              </span>
              <span className="flex items-center gap-1">
                <kbd className="h-4 rounded border bg-muted px-1 text-[10px]">↵</kbd>
                toggle
              </span>
              <span className="flex items-center gap-1">
                <kbd className="h-4 rounded border bg-muted px-1 text-[10px]">ESC</kbd>
                close
              </span>
            </div>
            {selectedIds.length > 0 && (
              <button
                onClick={() => onIdsChange([])}
                className="text-muted-foreground hover:text-foreground transition-colors"
              >
                Clear all
              </button>
            )}
          </div>
        </div>
      </div>
    </>,
    document.body
  );
}
