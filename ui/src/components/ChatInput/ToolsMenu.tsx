/**
 * ToolsMenu - Dropdown menu for enabling/disabling chat tools
 *
 * Displays available tools (file_search, code_interpreter, web_search) with
 * toggle switches. Tools that aren't implemented or have missing requirements
 * are shown as disabled.
 */

import { Wrench, Loader2, Plus, Database } from "lucide-react";

import { DataFileUpload } from "@/components/DataFileUpload";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/Popover/Popover";
import { Switch } from "@/components/Switch/Switch";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/Tooltip/Tooltip";
import { getToolIcon, TOOL_ICON_MAP } from "@/components/ToolIcons";
import { TOOL_METADATA } from "@/pages/chat/utils/toolExecutors";
import { cn } from "@/utils/cn";
import { pyodideService, type PyodideStatus } from "@/services/pyodide";
import { quickjsService, type QuickJSStatus } from "@/services/quickjs";
import { duckdbService, type DuckDBStatus } from "@/services/duckdb";
import { useDataFiles } from "@/stores/chatUIStore";
import { useState, useEffect } from "react";

export interface ToolsMenuProps {
  /** Currently enabled tool IDs */
  enabledTools: string[];
  /** Callback when enabled tools change */
  onEnabledToolsChange: (tools: string[]) => void;
  /** Attached vector store IDs (needed for file_search) */
  vectorStoreIds?: string[];
  /** Whether the menu is disabled */
  disabled?: boolean;
}

export function ToolsMenu({
  enabledTools,
  onEnabledToolsChange,
  vectorStoreIds,
  disabled = false,
}: ToolsMenuProps) {
  const [pyodideStatus, setPyodideStatus] = useState<PyodideStatus>(pyodideService.getStatus());
  const [quickjsStatus, setQuickjsStatus] = useState<QuickJSStatus>(quickjsService.getStatus());
  const [duckdbStatus, setDuckdbStatus] = useState<DuckDBStatus>(duckdbService.getStatus());
  const dataFiles = useDataFiles();

  const isSqlQueryEnabled = enabledTools.includes("sql_query");
  const hasDataFiles = dataFiles.length > 0;

  // Subscribe to Pyodide status changes
  useEffect(() => {
    return pyodideService.onStatusChange((status) => {
      setPyodideStatus(status);
    });
  }, []);

  // Subscribe to QuickJS status changes
  useEffect(() => {
    return quickjsService.onStatusChange((status) => {
      setQuickjsStatus(status);
    });
  }, []);

  // Subscribe to DuckDB status changes
  useEffect(() => {
    return duckdbService.onStatusChange((status) => {
      setDuckdbStatus(status);
    });
  }, []);

  const toggleTool = (toolId: string) => {
    if (enabledTools.includes(toolId)) {
      onEnabledToolsChange(enabledTools.filter((t) => t !== toolId));
    } else {
      onEnabledToolsChange([...enabledTools, toolId]);
    }
  };

  const isToolEnabled = (toolId: string) => enabledTools.includes(toolId);

  /** Check if a tool can be enabled (requirements met) */
  const canEnableTool = (toolId: string) => {
    if (toolId === "file_search") {
      return vectorStoreIds && vectorStoreIds.length > 0;
    }
    return true;
  };

  /** Check if a tool is currently loading */
  const isToolLoading = (toolId: string) => {
    if (toolId === "code_interpreter") return pyodideStatus === "loading";
    if (toolId === "js_code_interpreter") return quickjsStatus === "loading";
    if (toolId === "sql_query") return duckdbStatus === "loading";
    return false;
  };

  const hasEnabledTools = enabledTools.length > 0;

  // Get enabled tools with their icons for display
  const enabledToolsWithIcons = enabledTools
    .map((toolId) => ({
      id: toolId,
      Icon: TOOL_ICON_MAP[toolId] || Wrench,
      isLoading: isToolLoading(toolId),
    }))
    .filter((t) => t.Icon); // Only show tools we have icons for

  return (
    <Popover>
      <Tooltip>
        <TooltipTrigger asChild>
          <PopoverTrigger asChild>
            <button
              type="button"
              className={cn(
                "flex items-center gap-1 rounded-lg px-1.5 h-8 transition-colors",
                "hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
                disabled && "opacity-50 pointer-events-none"
              )}
              disabled={disabled}
              aria-label={hasEnabledTools ? "Manage tools" : "Add tools"}
            >
              {hasEnabledTools ? (
                // Show Wrench | ToolIcons format
                <>
                  <Wrench className="h-4 w-4 text-muted-foreground" />
                  <span className="text-muted-foreground/50">|</span>
                  <div className="flex items-center gap-0.5">
                    {enabledToolsWithIcons.map(({ id, Icon, isLoading }) =>
                      isLoading ? (
                        <Loader2 key={id} className="h-4 w-4 text-primary animate-spin" />
                      ) : (
                        <Icon key={id} className="h-4 w-4 text-primary" />
                      )
                    )}
                  </div>
                </>
              ) : (
                // Show wrench with plus when no tools enabled
                <>
                  <Wrench className="h-4 w-4 text-muted-foreground" />
                  <Plus className="h-3 w-3 text-muted-foreground" />
                </>
              )}
            </button>
          </PopoverTrigger>
        </TooltipTrigger>
        <TooltipContent side="top">
          <p>{hasEnabledTools ? "Manage tools" : "Add tools"}</p>
        </TooltipContent>
      </Tooltip>

      <PopoverContent align="start" className="w-72 p-3">
        <div className="space-y-1 mb-3">
          <h4 className="text-sm font-medium">Tools</h4>
          <p className="text-xs text-muted-foreground">
            Enable tools for the model to use during conversation.
          </p>
        </div>

        <div className="space-y-3">
          {TOOL_METADATA.filter((tool) => tool.id !== "display_artifacts").map((tool) => {
            const ToolIcon = getToolIcon(tool.id);
            const enabled = isToolEnabled(tool.id);
            const canEnable = canEnableTool(tool.id);
            const isDisabled = !tool.implemented || !canEnable;

            // Show loading state for interpreters when actively loading
            const showLoading = enabled && isToolLoading(tool.id);

            return (
              <div key={tool.id} className="flex items-start justify-between gap-3">
                <div className="flex items-start gap-2.5 min-w-0">
                  <div className="mt-0.5 shrink-0">
                    {showLoading ? (
                      <Loader2 className="h-4 w-4 text-primary animate-spin" />
                    ) : (
                      <ToolIcon
                        className={cn(
                          "h-4 w-4",
                          enabled ? "text-primary" : "text-muted-foreground"
                        )}
                      />
                    )}
                  </div>
                  <div className="min-w-0">
                    <div className="flex items-center gap-1.5">
                      <label className="text-sm font-medium">{tool.name}</label>
                      {!tool.implemented && (
                        <span className="text-[10px] px-1 py-0.5 rounded bg-muted text-muted-foreground">
                          Soon
                        </span>
                      )}
                    </div>
                    <p className="text-xs text-muted-foreground mt-0.5 line-clamp-2">
                      {tool.description}
                    </p>
                    {tool.requiresConfig && !canEnable && (
                      <p className="text-xs text-amber-800 dark:text-amber-500 mt-1">
                        {tool.configDescription}
                      </p>
                    )}
                  </div>
                </div>
                <Switch
                  checked={enabled}
                  onChange={() => toggleTool(tool.id)}
                  disabled={isDisabled}
                  className="shrink-0"
                />
              </div>
            );
          })}
        </div>

        {/* Data Files section - shown when SQL Query is enabled */}
        {isSqlQueryEnabled && (
          <div className="mt-4 pt-4 border-t">
            <div className="space-y-1 mb-3">
              <div className="flex items-center gap-1.5">
                <Database className="h-3.5 w-3.5 text-muted-foreground" />
                <h4 className="text-sm font-medium">Data Files</h4>
                {hasDataFiles && (
                  <span className="text-[10px] px-1.5 py-0.5 rounded-full bg-primary/10 text-primary">
                    {dataFiles.length}
                  </span>
                )}
              </div>
              <p className="text-xs text-muted-foreground">
                Upload files to query with SQL. Files reset on page reload.
              </p>
            </div>
            <DataFileUpload compact disabled={disabled} />
          </div>
        )}
      </PopoverContent>
    </Popover>
  );
}
