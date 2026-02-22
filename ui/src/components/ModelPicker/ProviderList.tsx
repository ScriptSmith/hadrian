import { Star, Layers, User, Building2 } from "lucide-react";

import type { DynamicScope } from "./model-utils";
import { PROVIDER_COLORS } from "@/pages/providers/shared";
import { cn } from "@/utils/cn";

export type ProviderFilter = "all" | "favorites" | string;

export interface ProviderInfo {
  id: string;
  label: string;
  color: string;
  modelCount: number;
  /** Whether this provider is user-added (dynamic) or built-in (static) */
  isDynamic?: boolean;
  /** Scope of the dynamic provider (user, org, project) */
  dynamicScope?: DynamicScope;
}

interface ProviderListProps {
  providers: ProviderInfo[];
  selectedProvider: ProviderFilter;
  onSelectProvider: (provider: ProviderFilter) => void;
  totalModelCount: number;
  favoriteCount: number;
  selectedCount: number;
  /** When true, renders as horizontal scrollable pills on mobile, vertical list on sm+ */
  horizontal?: boolean;
}

function getProviderColor(providerId: string): string {
  return PROVIDER_COLORS[providerId.toLowerCase()]?.solid ?? "bg-gray-500";
}

export function ProviderList({
  providers,
  selectedProvider,
  onSelectProvider,
  totalModelCount,
  favoriteCount,
  selectedCount,
  horizontal = false,
}: ProviderListProps) {
  const builtInProviders = providers.filter((p) => !p.isDynamic);
  const userProviders = providers.filter((p) => p.isDynamic && p.dynamicScope === "user");
  const orgProviders = providers.filter((p) => p.isDynamic && p.dynamicScope === "org");
  const projectProviders = providers.filter((p) => p.isDynamic && p.dynamicScope === "project");

  // Shared button styles
  const getButtonClass = (isActive: boolean) =>
    cn(
      "flex items-center gap-2 text-sm rounded-md whitespace-nowrap",
      // Horizontal (mobile): pill style
      horizontal && "px-3 py-1.5 sm:px-3 sm:py-2 sm:text-left",
      // Vertical (desktop): full width
      !horizontal && "px-3 py-2 text-left w-full",
      isActive
        ? "bg-accent text-accent-foreground font-medium"
        : "text-muted-foreground hover:bg-accent/50 hover:text-foreground"
    );

  return (
    <div
      className={cn(
        horizontal
          ? // Mobile: horizontal scrollable, Desktop: vertical list
            "flex gap-1 p-2 overflow-x-auto sm:flex-col sm:gap-0.5 sm:overflow-x-visible"
          : // Always vertical
            "flex flex-col gap-0.5 py-2"
      )}
    >
      {/* All Models */}
      <button
        type="button"
        onClick={() => onSelectProvider("all")}
        className={getButtonClass(selectedProvider === "all")}
      >
        <Layers className="h-4 w-4 shrink-0" />
        <span className={cn(horizontal ? "" : "flex-1 truncate")}>All</span>
        <span className="text-xs tabular-nums">{totalModelCount}</span>
      </button>

      {/* Favorites */}
      {favoriteCount > 0 && (
        <button
          type="button"
          onClick={() => onSelectProvider("favorites")}
          className={getButtonClass(selectedProvider === "favorites")}
        >
          <Star className="h-4 w-4 shrink-0 text-yellow-500 fill-yellow-500" />
          <span className={cn(horizontal ? "" : "flex-1 truncate")}>Favorites</span>
          <span className="text-xs tabular-nums">{favoriteCount}</span>
        </button>
      )}

      {/* Selected indicator - only show in vertical mode */}
      {selectedCount > 0 && !horizontal && (
        <div className="mx-3 my-2 text-xs text-muted-foreground">
          <span className="font-medium text-primary">{selectedCount}</span> selected
        </div>
      )}

      {/* Divider */}
      {horizontal ? (
        <div className="w-px bg-border shrink-0 my-1 sm:hidden" />
      ) : (
        <div className="mx-3 my-1 border-t" />
      )}

      {/* Built-in providers */}
      {builtInProviders.map((provider) => (
        <button
          key={provider.id}
          type="button"
          onClick={() => onSelectProvider(provider.id)}
          className={getButtonClass(selectedProvider === provider.id)}
        >
          <span
            className={cn("h-3 w-3 rounded-sm shrink-0", getProviderColor(provider.id))}
            aria-hidden="true"
          />
          <span className={cn(horizontal ? "" : "flex-1 truncate")}>{provider.label}</span>
          <span className="text-xs tabular-nums">{provider.modelCount}</span>
        </button>
      ))}

      {/* Org dynamic providers */}
      {orgProviders.length > 0 && (
        <>
          <div
            className={cn(
              "flex items-center gap-1.5 text-[10px] font-medium uppercase tracking-wider text-muted-foreground",
              horizontal ? "hidden sm:flex mx-3 mt-3 mb-1" : "mx-3 mt-2 mb-1"
            )}
          >
            <Building2 className="h-3 w-3" />
            Org Providers
          </div>
          {horizontal && <div className="w-px bg-border shrink-0 my-1 sm:hidden" />}
          {orgProviders.map((provider) => (
            <button
              key={provider.id}
              type="button"
              onClick={() => onSelectProvider(provider.id)}
              className={getButtonClass(selectedProvider === provider.id)}
            >
              <span className="h-3 w-3 rounded-sm shrink-0 bg-blue-500" aria-hidden="true" />
              <span className={cn(horizontal ? "" : "flex-1 truncate")}>{provider.label}</span>
              <span className="text-xs tabular-nums">{provider.modelCount}</span>
            </button>
          ))}
        </>
      )}

      {/* User dynamic providers */}
      {userProviders.length > 0 && (
        <>
          <div
            className={cn(
              "flex items-center gap-1.5 text-[10px] font-medium uppercase tracking-wider text-muted-foreground",
              horizontal ? "hidden sm:flex mx-3 mt-3 mb-1" : "mx-3 mt-2 mb-1"
            )}
          >
            <User className="h-3 w-3" />
            User Providers
          </div>
          {horizontal && <div className="w-px bg-border shrink-0 my-1 sm:hidden" />}
          {userProviders.map((provider) => (
            <button
              key={provider.id}
              type="button"
              onClick={() => onSelectProvider(provider.id)}
              className={getButtonClass(selectedProvider === provider.id)}
            >
              <span className="h-3 w-3 rounded-sm shrink-0 bg-emerald-500" aria-hidden="true" />
              <span className={cn(horizontal ? "" : "flex-1 truncate")}>{provider.label}</span>
              <span className="text-xs tabular-nums">{provider.modelCount}</span>
            </button>
          ))}
        </>
      )}

      {/* Project dynamic providers */}
      {projectProviders.length > 0 && (
        <>
          <div
            className={cn(
              "flex items-center gap-1.5 text-[10px] font-medium uppercase tracking-wider text-muted-foreground",
              horizontal ? "hidden sm:flex mx-3 mt-3 mb-1" : "mx-3 mt-2 mb-1"
            )}
          >
            <Layers className="h-3 w-3" />
            Project Providers
          </div>
          {horizontal && <div className="w-px bg-border shrink-0 my-1 sm:hidden" />}
          {projectProviders.map((provider) => (
            <button
              key={provider.id}
              type="button"
              onClick={() => onSelectProvider(provider.id)}
              className={getButtonClass(selectedProvider === provider.id)}
            >
              <span className="h-3 w-3 rounded-sm shrink-0 bg-amber-500" aria-hidden="true" />
              <span className={cn(horizontal ? "" : "flex-1 truncate")}>{provider.label}</span>
              <span className="text-xs tabular-nums">{provider.modelCount}</span>
            </button>
          ))}
        </>
      )}
    </div>
  );
}
