import { useQuery } from "@tanstack/react-query";
import {
  Check,
  Database,
  Download,
  FileJson,
  FileText,
  FolderOpen,
  GitFork,
  Maximize2,
  Minimize2,
  Trash2,
} from "lucide-react";

import { vectorStoreListOptions } from "@/api/generated/@tanstack/react-query.gen";
import type { VectorStoreOwnerType } from "@/api/generated/types.gen";
import { Button } from "@/components/Button/Button";
import type { Conversation, ModelInstance, ModelParameters } from "@/components/chat-types";
import type { TotalUsageResult } from "@/stores/conversationStore";
import {
  Dropdown,
  DropdownContent,
  DropdownItem,
  DropdownLabel,
  DropdownTrigger,
} from "@/components/Dropdown/Dropdown";
import { ModeConfigPanel } from "@/components/ModeConfigPanel/ModeConfigPanel";
import { ModeSelector } from "@/components/ModeSelector/ModeSelector";
import { ModelSelector, type ModelInfo } from "@/components/ModelSelector/ModelSelector";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/Tooltip/Tooltip";
import {
  useChatUIStore,
  useConversationMode,
  useModeConfig,
  useWidescreenMode,
} from "@/stores/chatUIStore";
import { useUserProjects } from "@/hooks/useUserProjects";
import { downloadConversation } from "@/utils/exportConversation";
import { formatCost, formatTokens } from "@/utils/formatters";

interface ChatHeaderProps {
  totalUsage?: TotalUsageResult | null;
  /** Selected model instances */
  selectedInstances: ModelInstance[];
  /** Callback when instances change */
  onInstancesChange: (instances: ModelInstance[]) => void;
  availableModels: ModelInfo[];
  /** Whether models are still loading */
  isLoadingModels?: boolean;
  /** Callback when instance parameters change */
  onInstanceParametersChange?: (instanceId: string, params: ModelParameters) => void;
  /** Callback when instance label changes */
  onInstanceLabelChange?: (instanceId: string, label: string) => void;
  /** Instance IDs that are disabled (hidden from view, not queried) */
  disabledInstances?: string[];
  /** Callback when disabled instances change */
  onDisabledInstancesChange?: (instanceIds: string[]) => void;
  onClear?: () => void;
  canClear?: boolean;
  /** Whether there are messages in the conversation (enables model disable toggle) */
  hasMessages?: boolean;
  isStreaming?: boolean;
  /** Current conversation for export functionality */
  conversation?: Conversation | null;
  /** Callback to fork the current conversation */
  onFork?: () => void;
  /** Callback to change the project a conversation belongs to */
  onProjectChange?: (projectId: string | null, projectName?: string) => void;
  /** Callback to select a project before the conversation is created */
  onPendingProjectChange?: (projectId: string | null, projectName?: string) => void;
  /** Display name for the pending project selection (before conversation exists) */
  pendingProjectName?: string;
  /** Attached vector store IDs for RAG/file_search */
  vectorStoreIds?: string[];
  /** Owner type for vector store lookup */
  vectorStoreOwnerType?: VectorStoreOwnerType;
  /** Owner ID for vector store lookup */
  vectorStoreOwnerId?: string;
}

export function ChatHeader({
  totalUsage,
  selectedInstances,
  onInstancesChange,
  availableModels,
  isLoadingModels = false,
  onInstanceParametersChange,
  onInstanceLabelChange,
  disabledInstances = [],
  onDisabledInstancesChange,
  onClear,
  canClear = false,
  hasMessages = false,
  isStreaming = false,
  conversation,
  onFork,
  onProjectChange,
  onPendingProjectChange,
  pendingProjectName,
  vectorStoreIds = [],
  vectorStoreOwnerType,
  vectorStoreOwnerId,
}: ChatHeaderProps) {
  const conversationMode = useConversationMode();
  const modeConfig = useModeConfig();
  const widescreenMode = useWidescreenMode();
  const { setConversationMode, setModeConfig, toggleWidescreenMode } = useChatUIStore();
  const canExport = conversation && conversation.messages.length > 0;

  // Fetch user projects for the project picker
  const { projects } = useUserProjects();

  // Fetch vector store names for tooltip
  const { data: vectorStoresResponse } = useQuery({
    ...vectorStoreListOptions({
      query: {
        owner_type: vectorStoreOwnerType!,
        owner_id: vectorStoreOwnerId!,
        limit: 100,
      },
    }),
    enabled: vectorStoreIds.length > 0 && !!vectorStoreOwnerType && !!vectorStoreOwnerId,
  });

  // Get names for attached vector stores
  const attachedStoreNames =
    vectorStoreIds.length > 0 && vectorStoresResponse?.data
      ? vectorStoreIds
          .map((id) => vectorStoresResponse.data?.find((s) => s.id === id)?.name || "Unknown")
          .filter(Boolean)
      : [];

  return (
    <div className="shrink-0 border-b bg-background/95 px-3 py-2 backdrop-blur supports-[backdrop-filter]:bg-background/60 sm:px-4 sm:py-3">
      <div className="flex flex-col gap-2">
        {/* Row 1: Mode selector, mode config, (model selector on desktop) | usage, actions */}
        <div className="flex items-center justify-between gap-2">
          {/* Left side */}
          <div className="flex items-center gap-2 min-w-0">
            <ModeSelector
              mode={conversationMode}
              onModeChange={setConversationMode}
              selectedModelCount={selectedInstances.length}
              isStreaming={isStreaming}
            />
            <ModeConfigPanel
              mode={conversationMode}
              config={modeConfig}
              onConfigChange={setModeConfig}
              availableInstances={selectedInstances}
              disabled={isStreaming}
            />
            {/* Model selector - inline on desktop only */}
            <div className="hidden sm:block min-w-0">
              <ModelSelector
                selectedInstances={selectedInstances}
                onInstancesChange={onInstancesChange}
                availableModels={availableModels}
                isLoading={isLoadingModels}
                onInstanceParametersChange={onInstanceParametersChange}
                onInstanceLabelChange={onInstanceLabelChange}
                disabledInstances={disabledInstances}
                onDisabledInstancesChange={onDisabledInstancesChange}
                hasMessages={hasMessages}
              />
            </div>
          </div>
          {/* Right side: usage, actions */}
          <div className="flex items-center gap-1 sm:gap-2 shrink-0">
            {totalUsage && (
              <Tooltip>
                <TooltipTrigger asChild>
                  <span className="text-[10px] sm:text-xs text-muted-foreground cursor-help px-1.5 sm:px-2 py-1 rounded bg-muted/50 whitespace-nowrap">
                    {formatTokens(
                      totalUsage.grandTotal.totalTokens +
                        (conversation?.titleGenerationUsage?.totalTokens ?? 0)
                    )}
                    <span className="hidden sm:inline"> tokens</span>
                    {(totalUsage.grandTotal.cost ?? 0) +
                      (conversation?.titleGenerationUsage?.cost ?? 0) >
                      0 && (
                      <span className="ml-1 sm:ml-1.5 text-muted-foreground">
                        ·{" "}
                        {formatCost(
                          (totalUsage.grandTotal.cost ?? 0) +
                            (conversation?.titleGenerationUsage?.cost ?? 0)
                        )}
                      </span>
                    )}
                  </span>
                </TooltipTrigger>
                <TooltipContent side="bottom" className="text-xs">
                  <div className="space-y-1">
                    <div className="font-medium">Conversation Usage</div>
                    <div>Input: {formatTokens(totalUsage.total.inputTokens)} tokens</div>
                    <div>Output: {formatTokens(totalUsage.total.outputTokens)} tokens</div>
                    {totalUsage.total.cachedTokens !== undefined &&
                      totalUsage.total.cachedTokens > 0 && (
                        <div>Cached: {formatTokens(totalUsage.total.cachedTokens)} tokens</div>
                      )}
                    {totalUsage.total.reasoningTokens !== undefined &&
                      totalUsage.total.reasoningTokens > 0 && (
                        <div>
                          Reasoning: {formatTokens(totalUsage.total.reasoningTokens)} tokens
                        </div>
                      )}
                    {totalUsage.modeOverhead.totalTokens > 0 && (
                      <div className="pt-1 border-t border-border/50">
                        <div className="font-medium text-muted-foreground">Mode Overhead</div>
                        <div>
                          {formatTokens(totalUsage.modeOverhead.totalTokens)} tokens
                          {totalUsage.modeOverhead.cost !== undefined &&
                            totalUsage.modeOverhead.cost > 0 && (
                              <span className="ml-1">
                                · {formatCost(totalUsage.modeOverhead.cost)}
                              </span>
                            )}
                        </div>
                      </div>
                    )}
                    {conversation?.titleGenerationUsage && (
                      <div className="pt-1 border-t border-border/50">
                        <div className="font-medium text-muted-foreground">Title Generation</div>
                        <div>
                          {formatTokens(conversation.titleGenerationUsage.totalTokens)} tokens
                          {conversation.titleGenerationUsage.cost !== undefined &&
                            conversation.titleGenerationUsage.cost > 0 && (
                              <span className="ml-1">
                                · {formatCost(conversation.titleGenerationUsage.cost)}
                              </span>
                            )}
                        </div>
                      </div>
                    )}
                    {(totalUsage.grandTotal.cost ?? 0) +
                      (conversation?.titleGenerationUsage?.cost ?? 0) >
                      0 && (
                      <div className="pt-1 border-t border-border/50">
                        Total Cost:{" "}
                        {formatCost(
                          (totalUsage.grandTotal.cost ?? 0) +
                            (conversation?.titleGenerationUsage?.cost ?? 0)
                        )}
                      </div>
                    )}
                  </div>
                </TooltipContent>
              </Tooltip>
            )}
            {vectorStoreIds.length > 0 && (
              <Tooltip>
                <TooltipTrigger asChild>
                  <span className="text-[10px] sm:text-xs text-muted-foreground cursor-help px-1.5 sm:px-2 py-1 rounded bg-muted/50 whitespace-nowrap flex items-center gap-1">
                    <Database className="h-3 w-3" />
                    <span>{vectorStoreIds.length}</span>
                    <span className="hidden sm:inline">
                      {vectorStoreIds.length === 1 ? "knowledge base" : "knowledge bases"}
                    </span>
                  </span>
                </TooltipTrigger>
                <TooltipContent side="bottom" className="text-xs max-w-[250px]">
                  <div className="space-y-1">
                    <div className="font-medium">Knowledge Base</div>
                    {attachedStoreNames.length > 0 ? (
                      <ul className="space-y-0.5">
                        {attachedStoreNames.map((name, i) => (
                          <li key={i} className="flex items-center gap-1.5">
                            <Database className="h-3 w-3 shrink-0 text-muted-foreground" />
                            <span className="truncate">{name}</span>
                          </li>
                        ))}
                      </ul>
                    ) : (
                      <div className="text-muted-foreground">
                        {vectorStoreIds.length} knowledge base
                        {vectorStoreIds.length === 1 ? "" : "s"} attached
                      </div>
                    )}
                  </div>
                </TooltipContent>
              </Tooltip>
            )}
            {/* Widescreen toggle - hidden on mobile */}
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  variant="ghost"
                  size="icon"
                  onClick={toggleWidescreenMode}
                  aria-label={widescreenMode ? "Exit widescreen" : "Widescreen mode"}
                  className="hidden sm:flex h-8 w-8 text-muted-foreground hover:text-foreground"
                >
                  {widescreenMode ? (
                    <Minimize2 className="h-4 w-4" />
                  ) : (
                    <Maximize2 className="h-4 w-4" />
                  )}
                </Button>
              </TooltipTrigger>
              <TooltipContent side="bottom">
                <p>{widescreenMode ? "Exit widescreen" : "Widescreen mode"}</p>
              </TooltipContent>
            </Tooltip>
            {canExport && (
              <Dropdown>
                <Tooltip>
                  <TooltipTrigger asChild>
                    <DropdownTrigger
                      showChevron={false}
                      aria-label="Export conversation"
                      className="h-8 w-8 p-0 border-0 bg-transparent text-muted-foreground hover:text-foreground hover:bg-accent"
                    >
                      <Download className="h-4 w-4" />
                    </DropdownTrigger>
                  </TooltipTrigger>
                  <TooltipContent side="bottom">
                    <p>Export conversation</p>
                  </TooltipContent>
                </Tooltip>
                <DropdownContent align="end">
                  <DropdownLabel>Export as</DropdownLabel>
                  <DropdownItem
                    onClick={() => downloadConversation(conversation, "json")}
                    className="gap-2"
                  >
                    <FileJson className="h-4 w-4" />
                    JSON (full data)
                  </DropdownItem>
                  <DropdownItem
                    onClick={() => downloadConversation(conversation, "markdown")}
                    className="gap-2"
                  >
                    <FileText className="h-4 w-4" />
                    Markdown (readable)
                  </DropdownItem>
                </DropdownContent>
              </Dropdown>
            )}
            {/* Fork button - hidden on mobile */}
            {canExport && onFork && (
              <Tooltip>
                <TooltipTrigger asChild>
                  <Button
                    variant="ghost"
                    size="icon"
                    onClick={onFork}
                    disabled={isStreaming}
                    aria-label="Fork conversation"
                    className="hidden sm:flex h-8 w-8 text-muted-foreground hover:text-foreground"
                  >
                    <GitFork className="h-4 w-4" />
                  </Button>
                </TooltipTrigger>
                <TooltipContent side="bottom">
                  <p>Fork conversation</p>
                </TooltipContent>
              </Tooltip>
            )}
            {/* Project picker - hidden on mobile */}
            {(() => {
              // Show project picker for existing conversations or pre-conversation selection
              const projectChangeHandler = conversation ? onProjectChange : onPendingProjectChange;
              const currentProjectId = conversation?.projectId ?? null;
              const currentProjectName = conversation
                ? conversation.projectName
                : pendingProjectName;
              if (!projectChangeHandler) return null;
              return (
                <Dropdown>
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <DropdownTrigger
                        showChevron={false}
                        aria-label="Change project"
                        className="hidden sm:flex h-8 max-w-[140px] px-2 gap-1.5 border-0 bg-transparent text-muted-foreground hover:text-foreground hover:bg-accent items-center"
                      >
                        <FolderOpen className="h-4 w-4 shrink-0" />
                        <span className="truncate text-xs">{currentProjectName || "Personal"}</span>
                      </DropdownTrigger>
                    </TooltipTrigger>
                    <TooltipContent side="bottom">
                      <p>{conversation ? "Move to project" : "Select project"}</p>
                    </TooltipContent>
                  </Tooltip>
                  <DropdownContent align="end">
                    <DropdownLabel>Project</DropdownLabel>
                    <DropdownItem onClick={() => projectChangeHandler(null)} className="gap-2">
                      {!currentProjectId && <Check className="h-4 w-4 shrink-0" />}
                      <span className={!currentProjectId ? "" : "pl-6"}>Personal</span>
                    </DropdownItem>
                    {projects.map((project) => (
                      <DropdownItem
                        key={project.id}
                        onClick={() => projectChangeHandler(project.id, project.name)}
                        className="gap-2"
                      >
                        {currentProjectId === project.id && <Check className="h-4 w-4 shrink-0" />}
                        <span className={currentProjectId === project.id ? "" : "pl-6"}>
                          {project.name}
                        </span>
                      </DropdownItem>
                    ))}
                  </DropdownContent>
                </Dropdown>
              );
            })()}
            {canClear && onClear && (
              <Tooltip>
                <TooltipTrigger asChild>
                  <Button
                    variant="ghost"
                    size="icon"
                    onClick={onClear}
                    disabled={isStreaming}
                    aria-label="Clear conversation"
                    className="h-8 w-8 text-muted-foreground hover:text-destructive"
                  >
                    <Trash2 className="h-4 w-4" />
                  </Button>
                </TooltipTrigger>
                <TooltipContent side="bottom">
                  <p>Clear conversation</p>
                </TooltipContent>
              </Tooltip>
            )}
          </div>
        </div>

        {/* Row 2: Model selector - mobile only */}
        <div className="sm:hidden">
          <ModelSelector
            selectedInstances={selectedInstances}
            onInstancesChange={onInstancesChange}
            availableModels={availableModels}
            isLoading={isLoadingModels}
            onInstanceParametersChange={onInstanceParametersChange}
            onInstanceLabelChange={onInstanceLabelChange}
            disabledInstances={disabledInstances}
            onDisabledInstancesChange={onDisabledInstancesChange}
            hasMessages={hasMessages}
          />
        </div>
      </div>
    </div>
  );
}
