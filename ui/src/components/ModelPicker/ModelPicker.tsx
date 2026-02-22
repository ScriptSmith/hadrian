import { Search, X, Brain, Wrench, Eye, Braces, Scale } from "lucide-react";
import { useState, useMemo, useEffect, useRef, useCallback } from "react";
import { createPortal } from "react-dom";

import { usePreferences } from "@/preferences/PreferencesProvider";
import type { ModelTask } from "@/preferences/types";
import { cn } from "@/utils/cn";

import { ModelDetailsPanel } from "./ModelDetailsPanel";
import { ModelGrid } from "./ModelGrid";
import type { ModelInfo, CapabilityFilter, DynamicScope } from "./model-utils";
import {
  getProviderFromId,
  getProviderInfo,
  getDynamicScope,
  matchesCapabilityFilter,
} from "./model-utils";
import { ProviderList } from "./ProviderList";
import type { ProviderFilter, ProviderInfo } from "./ProviderList";

export type { ModelInfo };

interface ModelPickerProps {
  open: boolean;
  onClose: () => void;
  selectedModels: string[];
  onModelsChange: (models: string[]) => void;
  availableModels: ModelInfo[];
  maxModels?: number;
  /** Allow adding duplicate instances of selected models */
  allowDuplicates?: boolean;
  /** Callback when adding a duplicate instance of an already-selected model */
  onAddDuplicate?: (modelId: string) => void;
  /** Which task context this picker is used in (scopes favorites/defaults) */
  task?: ModelTask;
}

export function ModelPicker({
  open,
  onClose,
  selectedModels,
  onModelsChange,
  availableModels,
  maxModels = 10,
  task: taskProp,
}: ModelPickerProps) {
  const task = taskProp ?? "chat";
  const [searchQuery, setSearchQuery] = useState("");
  const [selectedProvider, setSelectedProvider] = useState<ProviderFilter>("all");
  const [capabilityFilter, setCapabilityFilter] = useState<CapabilityFilter>("all");
  const [focusedIndex, setFocusedIndex] = useState(0);
  const [shouldScrollToFocused, setShouldScrollToFocused] = useState(false);
  const [detailModel, setDetailModel] = useState<ModelInfo | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const { preferences, setPreferences } = usePreferences();

  const favoriteModels = useMemo(
    () => preferences.favoriteModels?.[task] || [],
    [preferences.favoriteModels, task]
  );
  const defaultModels = useMemo(
    () => preferences.defaultModels?.[task] || [],
    [preferences.defaultModels, task]
  );

  // Sets for O(1) lookups - critical for performance with 900+ models
  const selectedSet = useMemo(() => new Set(selectedModels), [selectedModels]);
  const favoriteSet = useMemo(() => new Set(favoriteModels), [favoriteModels]);
  const defaultSet = useMemo(() => new Set(defaultModels), [defaultModels]);

  // Reset state when opening
  useEffect(() => {
    if (open) {
      setSearchQuery("");
      setSelectedProvider("all");
      setCapabilityFilter("all");
      setFocusedIndex(0);
      setDetailModel(null);
      setTimeout(() => inputRef.current?.focus(), 0);
    }
  }, [open]);

  // Handle escape key
  useEffect(() => {
    if (!open) return;
    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("keydown", handleEscape);
    return () => document.removeEventListener("keydown", handleEscape);
  }, [open, onClose]);

  // Compute providers and filtered models
  const { providers, filteredModels, totalCount, favoriteCount } = useMemo(() => {
    // Deduplicate models
    const seenIds = new Set<string>();
    let deduped = availableModels.filter((model) => {
      if (seenIds.has(model.id)) return false;
      seenIds.add(model.id);
      return true;
    });

    // Task filter: for chat, exclude models with explicit non-chat tasks
    if (task === "chat") {
      deduped = deduped.filter((m) => !m.tasks?.length || m.tasks.includes("chat"));
    }

    // Search filter
    const searchFiltered = deduped.filter((model) => {
      if (!searchQuery) return true;
      const query = searchQuery.toLowerCase();
      return (
        model.id.toLowerCase().includes(query) ||
        (model.owned_by?.toLowerCase().includes(query) ?? false)
      );
    });

    // Build provider list with counts, tracking source and scope.
    // Use a composite key (scope:name) so same-named providers in different scopes stay separate.
    const providerCounts: Record<
      string,
      { name: string; count: number; isDynamic: boolean; dynamicScope?: DynamicScope }
    > = {};
    for (const model of searchFiltered) {
      const provider = getProviderFromId(model.id);
      const scope = model.source === "dynamic" ? getDynamicScope(model.id) : undefined;
      const key = scope ? `${scope}:${provider}` : provider;
      if (!providerCounts[key]) {
        providerCounts[key] = {
          name: provider,
          count: 0,
          isDynamic: model.source === "dynamic",
          dynamicScope: scope,
        };
      }
      providerCounts[key].count += 1;
    }

    const providerList: ProviderInfo[] = Object.entries(providerCounts)
      .map(([id, { name, count, isDynamic, dynamicScope }]) => ({
        id,
        label: getProviderInfo(name, isDynamic ? "dynamic" : "static").label,
        color: getProviderInfo(name, isDynamic ? "dynamic" : "static").color,
        modelCount: count,
        isDynamic,
        dynamicScope,
      }))
      .sort((a, b) => b.modelCount - a.modelCount);

    // Apply provider filter (keys are scope-prefixed for dynamic providers)
    let filtered = searchFiltered;
    if (selectedProvider === "favorites") {
      filtered = searchFiltered.filter((m) => favoriteSet.has(m.id));
    } else if (selectedProvider !== "all") {
      filtered = searchFiltered.filter((m) => {
        const provider = getProviderFromId(m.id);
        const scope = m.source === "dynamic" ? getDynamicScope(m.id) : undefined;
        const key = scope ? `${scope}:${provider}` : provider;
        return key === selectedProvider;
      });
    }

    // Apply capability filter
    if (capabilityFilter !== "all") {
      filtered = filtered.filter((m) => matchesCapabilityFilter(m, capabilityFilter));
    }

    // Sort: selected first, then favorites, then rest alphabetically
    filtered = [...filtered].sort((a, b) => {
      const aSelected = selectedSet.has(a.id) ? 0 : 1;
      const bSelected = selectedSet.has(b.id) ? 0 : 1;
      if (aSelected !== bSelected) return aSelected - bSelected;

      const aFavorite = favoriteSet.has(a.id) ? 0 : 1;
      const bFavorite = favoriteSet.has(b.id) ? 0 : 1;
      return aFavorite - bFavorite;
    });

    const favCount = searchFiltered.filter((m) => favoriteSet.has(m.id)).length;

    return {
      providers: providerList,
      filteredModels: filtered,
      totalCount: searchFiltered.length,
      favoriteCount: favCount,
    };
  }, [
    availableModels,
    searchQuery,
    selectedProvider,
    capabilityFilter,
    selectedSet,
    favoriteSet,
    task,
  ]);

  // Reset focus when filter changes
  useEffect(() => {
    setFocusedIndex(0);
  }, [searchQuery, selectedProvider, capabilityFilter]);

  const handleToggleModel = useCallback(
    (modelId: string) => {
      if (selectedSet.has(modelId)) {
        onModelsChange(selectedModels.filter((m) => m !== modelId));
      } else if (selectedModels.length < maxModels) {
        onModelsChange([...selectedModels, modelId]);
      }
    },
    [selectedModels, selectedSet, onModelsChange, maxModels]
  );

  const handleToggleFavorite = useCallback(
    (modelId: string, e: React.MouseEvent) => {
      e.stopPropagation();
      const current = preferences.favoriteModels || {};
      const taskFavs = current[task] || [];
      const newTaskFavs = favoriteSet.has(modelId)
        ? taskFavs.filter((m) => m !== modelId)
        : [...taskFavs, modelId];
      setPreferences({ favoriteModels: { ...current, [task]: newTaskFavs } });
    },
    [preferences.favoriteModels, task, favoriteSet, setPreferences]
  );

  const handleToggleDefault = useCallback(
    (modelId: string, e: React.MouseEvent) => {
      e.stopPropagation();
      const current = preferences.defaultModels || {};
      const taskDefaults = current[task] || [];
      const newTaskDefaults = defaultSet.has(modelId)
        ? taskDefaults.filter((m) => m !== modelId)
        : [...taskDefaults, modelId];
      setPreferences({ defaultModels: { ...current, [task]: newTaskDefaults } });
    },
    [preferences.defaultModels, task, defaultSet, setPreferences]
  );

  const handleShowDetails = useCallback((model: ModelInfo) => {
    setDetailModel(model);
  }, []);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    // Grid columns vary by breakpoint (1/2/3), use 3 for desktop keyboard nav
    const cols = 3;
    const total = filteredModels.length;

    switch (e.key) {
      case "ArrowDown":
        e.preventDefault();
        setShouldScrollToFocused(true);
        setFocusedIndex((prev) => Math.min(prev + cols, total - 1));
        break;
      case "ArrowUp":
        e.preventDefault();
        setShouldScrollToFocused(true);
        setFocusedIndex((prev) => Math.max(prev - cols, 0));
        break;
      case "ArrowRight":
        e.preventDefault();
        setShouldScrollToFocused(true);
        setFocusedIndex((prev) => Math.min(prev + 1, total - 1));
        break;
      case "ArrowLeft":
        e.preventDefault();
        setShouldScrollToFocused(true);
        setFocusedIndex((prev) => Math.max(prev - 1, 0));
        break;
      case "Enter":
        e.preventDefault();
        if (filteredModels[focusedIndex]) {
          handleToggleModel(filteredModels[focusedIndex].id);
        }
        break;
    }
  };

  if (!open) return null;

  return createPortal(
    <>
      {/* Backdrop */}
      <div
        className="fixed inset-0 z-50 bg-black/50 backdrop-blur-sm animate-in fade-in-0"
        onClick={onClose}
        aria-hidden="true"
      />

      {/* Dialog - responsive: fullscreen on mobile, centered modal on desktop */}
      <div className="fixed inset-2 z-50 sm:inset-6 sm:top-[3%] sm:bottom-[3%] animate-in fade-in-0 zoom-in-95 slide-in-from-top-4">
        <div className="flex h-full flex-col overflow-hidden rounded-xl border bg-popover shadow-2xl ring-1 ring-black/5">
          {/* Search input - full width */}
          <div className="flex items-center border-b px-4">
            <Search className="h-5 w-5 shrink-0 text-muted-foreground" aria-hidden="true" />
            <input
              ref={inputRef}
              type="text"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder="Search models..."
              aria-label="Search models"
              className="flex-1 bg-transparent px-4 py-4 text-sm outline-none placeholder:text-muted-foreground"
            />
            <div className="flex items-center gap-2">
              <span className="text-xs text-muted-foreground">
                {selectedModels.length}/{maxModels}
              </span>
              <kbd className="pointer-events-none hidden h-6 select-none items-center gap-1 rounded border bg-muted px-1.5 font-mono text-xs text-muted-foreground sm:flex">
                ESC
              </kbd>
              <button
                type="button"
                onClick={onClose}
                className="flex h-8 w-8 items-center justify-center rounded-md text-muted-foreground hover:bg-accent hover:text-foreground"
                aria-label="Close"
              >
                <X className="h-5 w-5" />
              </button>
            </div>
          </div>

          {/* Capability filter chips */}
          <div className="flex items-center gap-2 px-4 py-2 border-b overflow-x-auto">
            <span className="text-xs text-muted-foreground shrink-0">Filter:</span>
            <div className="flex items-center gap-1.5">
              <button
                type="button"
                onClick={() => setCapabilityFilter("all")}
                className={cn(
                  "rounded-full px-2.5 py-1 text-xs font-medium transition-colors",
                  capabilityFilter === "all"
                    ? "bg-primary text-primary-foreground"
                    : "bg-muted text-muted-foreground hover:bg-muted/80"
                )}
              >
                All
              </button>
              <button
                type="button"
                onClick={() =>
                  setCapabilityFilter(capabilityFilter === "reasoning" ? "all" : "reasoning")
                }
                className={cn(
                  "inline-flex items-center gap-1 rounded-full px-2.5 py-1 text-xs font-medium transition-colors",
                  capabilityFilter === "reasoning"
                    ? "bg-purple-500 text-white"
                    : "bg-purple-500/10 text-purple-700 hover:bg-purple-500/20"
                )}
              >
                <Brain className="h-3 w-3" />
                Reasoning
              </button>
              <button
                type="button"
                onClick={() =>
                  setCapabilityFilter(capabilityFilter === "tool_call" ? "all" : "tool_call")
                }
                className={cn(
                  "inline-flex items-center gap-1 rounded-full px-2.5 py-1 text-xs font-medium transition-colors",
                  capabilityFilter === "tool_call"
                    ? "bg-green-500 text-white"
                    : "bg-green-500/10 text-green-800 hover:bg-green-500/20"
                )}
              >
                <Wrench className="h-3 w-3" />
                Tools
              </button>
              <button
                type="button"
                onClick={() =>
                  setCapabilityFilter(capabilityFilter === "vision" ? "all" : "vision")
                }
                className={cn(
                  "inline-flex items-center gap-1 rounded-full px-2.5 py-1 text-xs font-medium transition-colors",
                  capabilityFilter === "vision"
                    ? "bg-cyan-500 text-white"
                    : "bg-cyan-500/10 text-cyan-800 hover:bg-cyan-500/20"
                )}
              >
                <Eye className="h-3 w-3" />
                Vision
              </button>
              <button
                type="button"
                onClick={() =>
                  setCapabilityFilter(
                    capabilityFilter === "structured_output" ? "all" : "structured_output"
                  )
                }
                className={cn(
                  "inline-flex items-center gap-1 rounded-full px-2.5 py-1 text-xs font-medium transition-colors",
                  capabilityFilter === "structured_output"
                    ? "bg-amber-500 text-white"
                    : "bg-amber-500/10 text-amber-800 hover:bg-amber-500/20"
                )}
              >
                <Braces className="h-3 w-3" />
                JSON
              </button>
              <button
                type="button"
                onClick={() =>
                  setCapabilityFilter(capabilityFilter === "open_weights" ? "all" : "open_weights")
                }
                className={cn(
                  "inline-flex items-center gap-1 rounded-full px-2.5 py-1 text-xs font-medium transition-colors",
                  capabilityFilter === "open_weights"
                    ? "bg-indigo-500 text-white"
                    : "bg-indigo-500/10 text-indigo-700 hover:bg-indigo-500/20"
                )}
              >
                <Scale className="h-3 w-3" />
                Open
              </button>
            </div>
          </div>

          {/* Layout: stacked on mobile, three-column on lg+ */}
          <div className="flex flex-1 flex-col overflow-hidden sm:flex-row min-h-0">
            {/* Provider filter - horizontal scroll on mobile, vertical sidebar on sm+ */}
            <div className="shrink-0 border-b sm:border-b-0 sm:border-r sm:w-[180px] md:w-[200px] overflow-y-auto">
              <ProviderList
                providers={providers}
                selectedProvider={selectedProvider}
                onSelectProvider={setSelectedProvider}
                totalModelCount={totalCount}
                favoriteCount={favoriteCount}
                selectedCount={selectedModels.length}
                horizontal={true}
              />
            </div>

            {/* Model grid */}
            <div className="flex-1 overflow-y-auto min-h-0">
              <ModelGrid
                models={filteredModels}
                selectedSet={selectedSet}
                favoriteSet={favoriteSet}
                defaultSet={defaultSet}
                maxModels={maxModels}
                focusedIndex={focusedIndex}
                shouldScrollToFocused={shouldScrollToFocused}
                detailModelId={detailModel?.id}
                onToggleModel={handleToggleModel}
                onToggleFavorite={handleToggleFavorite}
                onToggleDefault={handleToggleDefault}
                onShowDetails={handleShowDetails}
              />
            </div>

            {/* Details panel - hidden on mobile/tablet, visible on lg+ */}
            <div className="hidden lg:block shrink-0 w-80 border-l overflow-y-auto bg-muted/30">
              <ModelDetailsPanel
                model={detailModel}
                className="h-full"
                onClose={() => setDetailModel(null)}
              />
            </div>
          </div>

          {/* Footer - simplified on mobile */}
          <div className="flex items-center justify-between border-t px-4 py-2 text-xs text-muted-foreground">
            <div className="hidden items-center gap-4 sm:flex">
              <span className="flex items-center gap-1">
                <kbd className="h-4 rounded border bg-muted px-1 text-[10px]">←</kbd>
                <kbd className="h-4 rounded border bg-muted px-1 text-[10px]">→</kbd>
                <kbd className="h-4 rounded border bg-muted px-1 text-[10px]">↑</kbd>
                <kbd className="h-4 rounded border bg-muted px-1 text-[10px]">↓</kbd>
                navigate
              </span>
              <span className="flex items-center gap-1">
                <kbd className="h-4 rounded border bg-muted px-1 text-[10px]">↵</kbd>
                toggle
              </span>
            </div>
            <span className="text-muted-foreground sm:hidden">{filteredModels.length} models</span>
            {selectedModels.length > 0 && (
              <button
                onClick={() => onModelsChange([])}
                className="text-muted-foreground hover:text-foreground"
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
