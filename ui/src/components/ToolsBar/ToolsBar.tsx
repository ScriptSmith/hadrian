/**
 * ToolsBar - Expandable toolbar for chat tools
 *
 * Default state shows wrench icon + enabled tool icons.
 * On hover, expands to reveal ALL tool icons flying out from the wrench.
 * Hovering a tool shows a flyout with description and toggle.
 */

import { ChevronDown, Loader2, Plus, Settings, Wrench, X } from "lucide-react";
import { useState, useEffect, useCallback, useMemo, useRef } from "react";

import type { VectorStoreOwnerType } from "@/api/generated/types.gen";
import { Button } from "@/components/Button/Button";
import { DataFileUpload } from "@/components/DataFileUpload";
import { ModelPicker, type ModelInfo } from "@/components/ModelPicker/ModelPicker";
import { Popover, PopoverAnchor, PopoverContent } from "@/components/Popover/Popover";
import { getToolIcon } from "@/components/ToolIcons";
import { TOOL_METADATA, type ToolMetadata } from "@/pages/chat/utils/toolExecutors";
import { cn } from "@/utils/cn";
import { pyodideService, type PyodideStatus } from "@/services/pyodide";
import { quickjsService, type QuickJSStatus } from "@/services/quickjs";
import { duckdbService, type DuckDBStatus } from "@/services/duckdb";
import { VectorStoreSelector } from "@/components/VectorStores/VectorStoreSelector";
import { getModelName } from "@/components/ModelPicker/model-utils";
import {
  useMCPServers,
  useConnectedServerCount,
  useMCPToolCount,
  useHasMCPError,
} from "@/stores/mcpStore";

export interface ToolsBarProps {
  /** Currently enabled tool IDs */
  enabledTools: string[];
  /** Callback when enabled tools change */
  onEnabledToolsChange: (tools: string[]) => void;
  /** Attached vector store IDs (needed for file_search) */
  vectorStoreIds?: string[];
  /** Callback when vector store IDs change */
  onVectorStoreIdsChange?: (ids: string[]) => void;
  /** Owner type for vector store filtering */
  vectorStoreOwnerType?: VectorStoreOwnerType;
  /** Owner ID for vector store filtering */
  vectorStoreOwnerId?: string;
  /** Whether the bar is disabled */
  disabled?: boolean;
  /** Available models for sub-agent selection */
  availableModels?: ModelInfo[];
  /** Currently selected sub-agent model (null = use current model) */
  subAgentModel?: string | null;
  /** Callback when sub-agent model changes */
  onSubAgentModelChange?: (model: string | null) => void;
  /** Callback to open MCP server configuration modal */
  onOpenMCPConfig?: () => void;
}

/** Extended tool data with resolved icon component */
interface ToolWithIcon extends ToolMetadata {
  loading?: boolean;
  IconComponent: React.ComponentType<{ className?: string }>;
}

/** Props for the ToolButton component */
interface ToolButtonProps {
  tool: ToolWithIcon;
  enabled: boolean;
  canEnable: boolean;
  disabled: boolean;
  onToggle: () => void;
  /** Additional classes for animation */
  className?: string;
  /** Inline styles for animation */
  style?: React.CSSProperties;
  /** Optional extra content to render in the flyout (e.g., settings panel) */
  extraContent?: React.ReactNode;
}

/** Individual tool button with hover flyout */
function ToolButton({
  tool,
  enabled,
  canEnable,
  disabled,
  onToggle,
  className,
  style,
  extraContent,
}: ToolButtonProps) {
  const [isOpen, setIsOpen] = useState(false);
  const timeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const isDisabledTool = !tool.implemented || !canEnable;
  const ToolIcon = tool.IconComponent;
  const hasExtraContent = !!extraContent;

  const handleMouseEnter = () => {
    if (timeoutRef.current) clearTimeout(timeoutRef.current);
    timeoutRef.current = setTimeout(() => setIsOpen(true), 200);
  };

  const handleMouseLeave = () => {
    if (timeoutRef.current) clearTimeout(timeoutRef.current);
    timeoutRef.current = setTimeout(() => setIsOpen(false), 150);
  };

  // Clear timeout on unmount
  useEffect(() => {
    return () => {
      if (timeoutRef.current) clearTimeout(timeoutRef.current);
    };
  }, []);

  // Wrap in a span to capture hover events even when button is "disabled"
  // (disabled buttons don't receive mouse events in browsers)
  return (
    <Popover open={isOpen} onOpenChange={setIsOpen}>
      <PopoverAnchor asChild>
        <span
          className={cn("inline-flex", className)}
          style={style}
          onMouseEnter={handleMouseEnter}
          onMouseLeave={handleMouseLeave}
        >
          <button
            type="button"
            onClick={(e) => {
              // Prevent click from toggling the popover - keep it open for configuration
              e.stopPropagation();
              if (!isDisabledTool && !disabled) onToggle();
            }}
            aria-disabled={isDisabledTool || disabled}
            aria-label={`Toggle ${tool.name}`}
            className={cn(
              "flex items-center justify-center w-7 h-7 rounded-md",
              "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
              enabled
                ? "bg-primary/10 text-primary hover:bg-primary/20"
                : "text-muted-foreground/50 hover:text-muted-foreground hover:bg-accent-foreground/5",
              isDisabledTool || disabled
                ? "opacity-40 cursor-not-allowed hover:bg-transparent"
                : "cursor-pointer"
            )}
          >
            {tool.loading ? (
              <Loader2 className="h-4 w-4 animate-spin" />
            ) : (
              <ToolIcon className="h-4 w-4" />
            )}
          </button>
        </span>
      </PopoverAnchor>
      <PopoverContent
        side="top"
        align="center"
        className={cn("p-3", hasExtraContent ? "w-72" : "w-56")}
        onMouseEnter={handleMouseEnter}
        onMouseLeave={handleMouseLeave}
      >
        <div className="space-y-2">
          {/* Header with icon and name */}
          <div className="flex items-center gap-2">
            <ToolIcon
              className={cn("h-4 w-4 shrink-0", enabled ? "text-primary" : "text-muted-foreground")}
            />
            <span className="font-medium text-sm">{tool.name}</span>
            {!tool.implemented && (
              <span className="text-[10px] px-1 py-0.5 rounded bg-muted text-muted-foreground">
                Soon
              </span>
            )}
            {enabled && (
              <span className="text-[10px] px-1.5 py-0.5 rounded-full bg-primary/10 text-primary">
                On
              </span>
            )}
          </div>

          {/* Description */}
          <p className="text-xs text-muted-foreground leading-relaxed">{tool.description}</p>

          {/* Disabled reason - show why the tool can't be enabled */}
          {!tool.implemented && (
            <div className="flex items-center gap-2 px-2 py-1.5 rounded-md bg-muted/50 text-muted-foreground">
              <span className="text-xs">This tool is not yet available</span>
            </div>
          )}
          {tool.implemented && tool.requiresConfig && !canEnable && (
            <div className="flex items-center gap-2 px-2 py-1.5 rounded-md bg-amber-500/10 text-amber-800 dark:text-amber-500">
              <span className="text-xs">{tool.configDescription}</span>
            </div>
          )}

          {/* Extra content (e.g., settings panel) */}
          {extraContent}

          {/* Loading status */}
          {tool.loading && (
            <div className="flex items-center gap-2 text-xs text-muted-foreground">
              <Loader2 className="h-3 w-3 animate-spin" />
              <span>Loading runtime...</span>
            </div>
          )}

          {/* Hint text */}
          {!isDisabledTool && (
            <p className="text-[11px] text-muted-foreground pt-1">
              Click icon to {enabled ? "disable" : "enable"}
            </p>
          )}
        </div>
      </PopoverContent>
    </Popover>
  );
}

/** Tools to display (excluding internal tools like display_artifacts) */
const VISIBLE_TOOLS = TOOL_METADATA.filter((tool) => tool.id !== "display_artifacts");

/** Sub-agent model selector component with ModelPicker integration */
interface SubAgentModelSelectorProps {
  availableModels: ModelInfo[];
  selectedModel: string | null | undefined;
  onModelChange: (model: string | null) => void;
  disabled?: boolean;
}

function SubAgentModelSelector({
  availableModels,
  selectedModel,
  onModelChange,
  disabled,
}: SubAgentModelSelectorProps) {
  const [pickerOpen, setPickerOpen] = useState(false);

  const handleModelsChange = useCallback(
    (models: string[]) => {
      // Single selection - take the first model or null
      const newModel = models.length > 0 ? models[0] : null;
      onModelChange(newModel);
      // Only close picker when a model is selected, not when deselected
      if (newModel) {
        setPickerOpen(false);
      }
    },
    [onModelChange]
  );

  const handleClear = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      onModelChange(null);
    },
    [onModelChange]
  );

  const selectedModelName = selectedModel ? getModelName(selectedModel) : null;

  return (
    <div className="pt-2 border-t mt-2 space-y-1.5">
      <span className="text-xs font-medium text-muted-foreground">Default Model</span>
      <Button
        variant="outline"
        size="sm"
        className={cn(
          "w-full h-8 justify-between text-xs font-normal",
          !selectedModel && "text-muted-foreground"
        )}
        onClick={() => setPickerOpen(true)}
        disabled={disabled}
      >
        <span className="truncate">{selectedModelName || "Use current model"}</span>
        <div className="flex items-center gap-1 shrink-0">
          {selectedModel && (
            <X
              className="h-3 w-3 text-muted-foreground hover:text-foreground"
              onClick={handleClear}
            />
          )}
          <ChevronDown className="h-3 w-3 text-muted-foreground" />
        </div>
      </Button>
      <p className="text-[10px] text-muted-foreground">
        Model used when delegating tasks to sub-agents
      </p>
      <ModelPicker
        open={pickerOpen}
        onClose={() => setPickerOpen(false)}
        selectedModels={selectedModel ? [selectedModel] : []}
        onModelsChange={handleModelsChange}
        availableModels={availableModels}
        maxModels={1}
      />
    </div>
  );
}

/** MCP server status component for flyout */
interface MCPServerStatusProps {
  onOpenConfig?: () => void;
  disabled?: boolean;
}

function MCPServerStatus({ onOpenConfig, disabled }: MCPServerStatusProps) {
  const servers = useMCPServers();
  const connectedCount = useConnectedServerCount();
  const toolCount = useMCPToolCount();
  const hasError = useHasMCPError();

  const enabledCount = servers.filter((s) => s.enabled).length;
  const totalServers = servers.length;

  return (
    <div className="pt-2 border-t mt-2 space-y-3">
      {/* Server stats */}
      <div className="space-y-1.5">
        <div className="flex items-center justify-between text-xs">
          <span className="text-muted-foreground">Servers</span>
          <span className="font-medium">
            {connectedCount}/{enabledCount} connected
            {totalServers > enabledCount && (
              <span className="text-muted-foreground"> ({totalServers} total)</span>
            )}
          </span>
        </div>
        <div className="flex items-center justify-between text-xs">
          <span className="text-muted-foreground">Tools available</span>
          <span className="font-medium">{toolCount}</span>
        </div>
      </div>

      {/* Error indicator */}
      {hasError && (
        <div className="flex items-center gap-2 px-2 py-1.5 rounded-md bg-destructive/10 text-destructive">
          <span className="text-xs">Some servers have connection errors</span>
        </div>
      )}

      {/* No servers hint */}
      {totalServers === 0 && (
        <div className="flex items-center gap-2 px-2 py-1.5 rounded-md bg-muted/50 text-muted-foreground">
          <span className="text-xs">No MCP servers configured</span>
        </div>
      )}

      {/* Manage servers button */}
      <Button
        variant="outline"
        size="sm"
        className="w-full h-8 text-xs"
        onClick={onOpenConfig}
        disabled={disabled || !onOpenConfig}
      >
        <Settings className="h-3.5 w-3.5 mr-1.5" />
        Manage Servers
      </Button>
    </div>
  );
}

export function ToolsBar({
  enabledTools,
  onEnabledToolsChange,
  vectorStoreIds,
  onVectorStoreIdsChange,
  vectorStoreOwnerType,
  vectorStoreOwnerId,
  disabled = false,
  availableModels,
  subAgentModel,
  onSubAgentModelChange,
  onOpenMCPConfig,
}: ToolsBarProps) {
  const [isHovering, setIsHovering] = useState(false);
  // Track whether we're in "stable mode" - delays layout changes to prevent jumping
  const [isStableMode, setIsStableMode] = useState(false);
  const stableModeTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [pyodideStatus, setPyodideStatus] = useState<PyodideStatus>(pyodideService.getStatus());
  const [quickjsStatus, setQuickjsStatus] = useState<QuickJSStatus>(quickjsService.getStatus());
  const [duckdbStatus, setDuckdbStatus] = useState<DuckDBStatus>(duckdbService.getStatus());

  // Enter stable mode immediately on hover, exit with delay
  useEffect(() => {
    if (isHovering) {
      // Clear any pending exit
      if (stableModeTimeoutRef.current) {
        clearTimeout(stableModeTimeoutRef.current);
        stableModeTimeoutRef.current = null;
      }
      setIsStableMode(true);
    } else {
      // Delay exiting stable mode to allow popover transitions
      stableModeTimeoutRef.current = setTimeout(() => {
        setIsStableMode(false);
      }, 300);
    }
    return () => {
      if (stableModeTimeoutRef.current) {
        clearTimeout(stableModeTimeoutRef.current);
      }
    };
  }, [isHovering]);

  // Subscribe to runtime status changes
  useEffect(() => {
    return pyodideService.onStatusChange(setPyodideStatus);
  }, []);

  useEffect(() => {
    return quickjsService.onStatusChange(setQuickjsStatus);
  }, []);

  useEffect(() => {
    return duckdbService.onStatusChange(setDuckdbStatus);
  }, []);

  const canEnableTool = useCallback(
    (toolId: string): boolean => {
      if (toolId === "file_search") {
        return Boolean(vectorStoreIds && vectorStoreIds.length > 0);
      }
      return true;
    },
    [vectorStoreIds]
  );

  const isToolLoading = useCallback(
    (toolId: string) => {
      if (toolId === "code_interpreter") return pyodideStatus === "loading";
      if (toolId === "js_code_interpreter") return quickjsStatus === "loading";
      if (toolId === "sql_query") return duckdbStatus === "loading";
      return false;
    },
    [pyodideStatus, quickjsStatus, duckdbStatus]
  );

  const toggleTool = useCallback(
    (toolId: string) => {
      if (enabledTools.includes(toolId)) {
        onEnabledToolsChange(enabledTools.filter((t) => t !== toolId));
      } else {
        onEnabledToolsChange([...enabledTools, toolId]);
      }
    },
    [enabledTools, onEnabledToolsChange]
  );

  const hasEnabledTools = enabledTools.length > 0;

  // All tools with metadata and icons
  const allToolsData = useMemo(
    (): ToolWithIcon[] =>
      VISIBLE_TOOLS.map((tool) => ({
        ...tool,
        loading: isToolLoading(tool.id),
        IconComponent: getToolIcon(tool.id),
      })),
    [isToolLoading]
  );

  // Enabled tools only (for collapsed view) - maintains VISIBLE_TOOLS order
  const enabledToolsData = useMemo(
    (): ToolWithIcon[] => allToolsData.filter((tool) => enabledTools.includes(tool.id)),
    [allToolsData, enabledTools]
  );

  // Get extra content for specific tools (settings panels)
  const getToolExtraContent = useCallback(
    (toolId: string): React.ReactNode => {
      // Show vector store selector for file_search tool
      // When ownerType/ownerId are provided, filter by owner; otherwise show all accessible
      if (toolId === "file_search" && onVectorStoreIdsChange) {
        return (
          <div className="pt-2 border-t mt-2">
            <VectorStoreSelector
              selectedIds={vectorStoreIds || []}
              onIdsChange={onVectorStoreIdsChange}
              ownerType={vectorStoreOwnerType}
              ownerId={vectorStoreOwnerId}
              maxStores={10}
              disabled={disabled}
              compact
            />
          </div>
        );
      }
      if (toolId === "sql_query") {
        return (
          <div className="pt-2 border-t mt-2">
            <DataFileUpload disabled={disabled} compact />
          </div>
        );
      }
      if (toolId === "sub_agent" && onSubAgentModelChange && availableModels) {
        return (
          <SubAgentModelSelector
            availableModels={availableModels}
            selectedModel={subAgentModel}
            onModelChange={onSubAgentModelChange}
            disabled={disabled}
          />
        );
      }
      if (toolId === "mcp") {
        return <MCPServerStatus onOpenConfig={onOpenMCPConfig} disabled={disabled} />;
      }
      return null;
    },
    [
      vectorStoreIds,
      onVectorStoreIdsChange,
      vectorStoreOwnerType,
      vectorStoreOwnerId,
      disabled,
      availableModels,
      subAgentModel,
      onSubAgentModelChange,
      onOpenMCPConfig,
    ]
  );

  // Tools to display: all tools when in stable mode, only enabled when collapsed
  const displayedTools = isStableMode ? allToolsData : enabledToolsData;

  return (
    <div
      className={cn(
        "relative flex items-center h-8 rounded-lg transition-colors duration-150 min-w-0",
        "hover:bg-accent",
        disabled && "opacity-50 pointer-events-none"
      )}
      onMouseEnter={() => setIsHovering(true)}
      onMouseLeave={() => setIsHovering(false)}
    >
      {/* Wrench icon - always visible */}
      <div className="flex items-center justify-center w-8 h-8 shrink-0">
        <Wrench
          className={cn("h-4 w-4", hasEnabledTools ? "text-primary" : "text-muted-foreground")}
        />
      </div>

      {/* Plus icon when no tools enabled and collapsed */}
      {!hasEnabledTools && !isStableMode && (
        <Plus className="h-3 w-3 text-muted-foreground -ml-1" />
      )}

      {/* Tools section - stable ordering when hovering, only enabled when collapsed */}
      <div
        className={cn(
          "flex items-center overflow-x-auto scrollbar-none min-w-0",
          "transition-all duration-200 ease-out"
        )}
        style={{
          // Calculate max-width based on displayed tools, but allow shrinking on mobile
          maxWidth:
            displayedTools.length > 0
              ? `${displayedTools.length * 28 + (displayedTools.length > 0 ? 8 : 0)}px`
              : "0px",
          opacity: displayedTools.length > 0 ? 1 : 0,
        }}
      >
        {displayedTools.length > 0 && <span className="text-muted-foreground/50 mr-0.5">|</span>}
        {displayedTools.map((tool, index) => {
          const isEnabled = enabledTools.includes(tool.id);
          // Calculate animation delay - only for non-enabled tools when expanding
          const isExtraTool = !isEnabled;
          const extraToolIndex = isExtraTool
            ? allToolsData.filter((t, i) => i < index && !enabledTools.includes(t.id)).length
            : 0;

          return (
            <ToolButton
              key={tool.id}
              tool={tool}
              enabled={isEnabled}
              canEnable={canEnableTool(tool.id)}
              disabled={disabled}
              onToggle={() => toggleTool(tool.id)}
              extraContent={getToolExtraContent(tool.id)}
              className={cn(
                "transition-all duration-200 ease-out",
                // Animate non-enabled tools when expanding
                isExtraTool && !isStableMode && "-translate-x-2 opacity-0",
                isExtraTool && isStableMode && "translate-x-0 opacity-100"
              )}
              style={{
                // Stagger animation for non-enabled tools
                transitionDelay: isExtraTool && isStableMode ? `${extraToolIndex * 30}ms` : "0ms",
              }}
            />
          );
        })}
      </div>
    </div>
  );
}
