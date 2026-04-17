import type { VectorStoreOwnerType } from "@/api/generated/types.gen";
import type { Conversation, ModelParameters } from "@/components/chat-types";
import { ChatHeader } from "@/components/ChatHeader/ChatHeader";
import { ChatInput } from "@/components/ChatInput/ChatInput";
import { ChatMessageList } from "@/components/ChatMessageList/ChatMessageList";
import { ConversationSettingsModal } from "@/components/ConversationSettingsModal/ConversationSettingsModal";
import { MCPConfigModal, type MCPServerPrefill } from "@/components/MCPConfigModal";
import type { ModelInfo } from "@/components/ModelSelector/ModelSelector";
import {
  useChatUIStore,
  useSystemPrompt,
  useDisabledModels,
  useActionConfig,
  useHistoryMode,
  useVectorStoreIds,
  useClientSideRAG,
  useEnabledTools,
  useMaxToolIterations,
  useCaptureRawSSEEvents,
  useTTSVoice,
  useTTSSpeed,
  useWidescreenMode,
  useSubAgentModel,
  useMCPConfigModalOpen,
} from "@/stores/chatUIStore";
import {
  useConversationStore,
  useSelectedInstances,
  useHasMessages,
  useTotalUsage,
  useCurrentConversationForExport,
} from "@/stores/conversationStore";
import { useMCPStore } from "@/stores/mcpStore";
import { useMemo, useCallback, useState, useEffect } from "react";

export interface ChatFile {
  id: string;
  name: string;
  type: string;
  size: number;
  base64: string;
  preview?: string;
}

export interface ChatViewProps {
  /** List of available models */
  availableModels: ModelInfo[];
  /** Current conversation (from ConversationsProvider for accurate metadata like titleGenerationUsage) */
  conversation?: Conversation | null;
  /** Whether models are loading */
  isStreaming?: boolean;
  /** Whether to show loading state for models */
  isLoadingModels?: boolean;
  /** Send a message */
  onSendMessage: (content: string, files?: ChatFile[]) => void;
  /** Stop streaming */
  onStopStreaming?: () => void;
  /** Clear messages */
  onClearMessages?: () => void;
  /** Callback to regenerate a response */
  onRegenerate?: (messageId: string, model: string) => void;
  /** Callback to regenerate all responses for a user message */
  onRegenerateAll?: (messageId: string) => void;
  /** Callback to fork conversation from a specific message */
  onForkFromMessage?: (messageId: string) => void;
  /** Callback to fork the entire current conversation */
  onFork?: () => void;
  /** Callback to change the project a conversation belongs to */
  onProjectChange?: (projectId: string | null, projectName?: string) => void;
  /** Callback to select a project before the conversation is created */
  onPendingProjectChange?: (projectId: string | null, projectName?: string) => void;
  /** Display name for the pending project selection */
  pendingProjectName?: string;
  /** Callback to edit a message and re-run from that point */
  onEditAndRerun?: (messageId: string, newContent: string) => void;
  /** Owner type for vector store filtering (e.g., "user", "organization") */
  vectorStoreOwnerType?: VectorStoreOwnerType;
  /** Owner ID for vector store filtering (e.g., user id, org id) */
  vectorStoreOwnerId?: string;
}

export function ChatView({
  availableModels,
  conversation: conversationProp,
  isStreaming = false,
  isLoadingModels = false,
  onSendMessage,
  onStopStreaming,
  onClearMessages,
  onRegenerate,
  onRegenerateAll,
  onForkFromMessage,
  onFork,
  onProjectChange,
  onPendingProjectChange,
  pendingProjectName,
  onEditAndRerun,
  vectorStoreOwnerType,
  vectorStoreOwnerId,
}: ChatViewProps) {
  // Subscribe to stores
  const selectedInstances = useSelectedInstances();
  const totalUsage = useTotalUsage();
  // Note: disabledModels in chatUIStore stores instance IDs when using instances
  const disabledInstances = useDisabledModels();
  const systemPrompt = useSystemPrompt();
  const actionConfig = useActionConfig();
  const historyMode = useHistoryMode();
  const vectorStoreIds = useVectorStoreIds();
  const clientSideRAG = useClientSideRAG();
  const enabledTools = useEnabledTools();
  const maxToolIterations = useMaxToolIterations();
  const captureRawSSEEvents = useCaptureRawSSEEvents();
  const ttsVoice = useTTSVoice();
  const ttsSpeed = useTTSSpeed();
  const widescreenMode = useWidescreenMode();
  const subAgentModel = useSubAgentModel();
  const mcpConfigModalOpen = useMCPConfigModalOpen();
  const [mcpPrefill, setMcpPrefill] = useState<MCPServerPrefill | null>(null);

  // Check for ?mcp_server_url= query param. If the server is already
  // configured, just enable (and connect) it; otherwise open the config
  // modal pre-filled with the URL.
  useEffect(() => {
    const params = new URLSearchParams(window.location.search);
    const serverUrl = params.get("mcp_server_url");
    if (serverUrl) {
      const serverName = params.get("mcp_server_name") ?? undefined;
      const mcp = useMCPStore.getState();
      const existing = mcp.servers.find((s) => s.url === serverUrl);
      if (existing) {
        if (!existing.enabled) {
          mcp.toggleServerEnabled(existing.id);
        } else if (existing.status !== "connected" && existing.status !== "connecting") {
          mcp.connectServer(existing.id).catch((err) => {
            console.debug("MCP connect from URL param failed:", err);
          });
        }
      } else {
        setMcpPrefill({ url: serverUrl, name: serverName });
        setMCPConfigModalOpen(true);
      }
      // Clean the URL to prevent re-triggering
      const cleanUrl = new URL(window.location.href);
      cleanUrl.searchParams.delete("mcp_server_url");
      cleanUrl.searchParams.delete("mcp_server_name");
      window.history.replaceState({}, "", cleanUrl.toString());
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps -- only run on mount
  }, []);

  const { setSelectedInstances, updateInstance } = useConversationStore();
  const {
    settingsModalOpen,
    setSettingsModalOpen,
    setMCPConfigModalOpen,
    setSystemPrompt,
    setDisabledModels: setDisabledInstances,
    setActionConfig,
    setHistoryMode,
    setVectorStoreIds,
    setClientSideRAG,
    setEnabledTools,
    setMaxToolIterations,
    setCaptureRawSSEEvents,
    setTTSVoice,
    setTTSSpeed,
    setSubAgentModel,
    setPendingPrompt,
  } = useChatUIStore();

  // Stable callback for instance parameter changes
  const handleInstanceParametersChange = useCallback(
    (instanceId: string, params: ModelParameters) => {
      updateInstance(instanceId, { parameters: params });
    },
    [updateInstance]
  );

  // Stable callback for instance label changes
  const handleInstanceLabelChange = useCallback(
    (instanceId: string, label: string) => {
      // Empty string means reset to default (no custom label)
      updateInstance(instanceId, { label: label || undefined });
    },
    [updateInstance]
  );

  const hasMessages = useHasMessages();
  const storeConversation = useCurrentConversationForExport();
  // Use prop if provided (from ConversationsProvider with full metadata like titleGenerationUsage)
  // Fall back to store version for export functionality
  const currentConversation = conversationProp ?? storeConversation;

  // Active instances are selected instances that aren't disabled
  const activeInstances = useMemo(
    () => selectedInstances.filter((i) => !disabledInstances.includes(i.id)),
    [selectedInstances, disabledInstances]
  );

  const inputDisabled = activeInstances.length === 0;
  const inputPlaceholder = inputDisabled
    ? selectedInstances.length === 0
      ? "Select a model to start chatting..."
      : "All models are disabled. Enable a model to continue..."
    : "Type a message...";

  return (
    <div className="flex h-full flex-col" role="region" aria-label="Chat">
      {/* Header */}
      <header>
        <ChatHeader
          totalUsage={totalUsage}
          selectedInstances={selectedInstances}
          onInstancesChange={setSelectedInstances}
          availableModels={availableModels}
          isLoadingModels={isLoadingModels}
          onInstanceParametersChange={handleInstanceParametersChange}
          onInstanceLabelChange={handleInstanceLabelChange}
          disabledInstances={disabledInstances}
          onDisabledInstancesChange={setDisabledInstances}
          onClear={onClearMessages}
          canClear={hasMessages}
          hasMessages={hasMessages}
          isStreaming={isStreaming}
          conversation={currentConversation}
          onFork={onFork}
          onProjectChange={onProjectChange}
          onPendingProjectChange={onPendingProjectChange}
          pendingProjectName={pendingProjectName}
          vectorStoreIds={vectorStoreIds}
          vectorStoreOwnerType={vectorStoreOwnerType}
          vectorStoreOwnerId={vectorStoreOwnerId}
        />
      </header>

      {/* Messages */}
      <main className="flex flex-1 flex-col overflow-hidden">
        <ChatMessageList
          isLoadingModels={isLoadingModels}
          noModelsAvailable={!isLoadingModels && availableModels.length === 0}
          onRegenerate={onRegenerate}
          onRegenerateAll={onRegenerateAll}
          onForkFromMessage={onForkFromMessage}
          onEditAndRerun={onEditAndRerun}
        />
      </main>

      {/* Input area */}
      <footer className="shrink-0 border-t bg-background/95 px-3 py-2 backdrop-blur supports-[backdrop-filter]:bg-background/60 sm:px-4 sm:py-3">
        <div className={`mx-auto ${widescreenMode ? "" : "max-w-3xl"}`}>
          <ChatInput
            onSend={onSendMessage}
            onStop={onStopStreaming}
            isStreaming={isStreaming}
            disabled={inputDisabled}
            noModelsSelected={selectedInstances.length === 0}
            noModelsAvailable={!isLoadingModels && availableModels.length === 0}
            placeholder={inputPlaceholder}
            onSettingsClick={() => setSettingsModalOpen(true)}
            hasSystemPrompt={!!systemPrompt}
            hasMultipleModels={activeInstances.length > 1}
            historyMode={historyMode}
            onHistoryModeChange={setHistoryMode}
            enabledTools={enabledTools}
            onEnabledToolsChange={setEnabledTools}
            vectorStoreIds={vectorStoreIds}
            onVectorStoreIdsChange={setVectorStoreIds}
            vectorStoreOwnerType={vectorStoreOwnerType}
            vectorStoreOwnerId={vectorStoreOwnerId}
            availableModels={availableModels}
            subAgentModel={subAgentModel}
            onSubAgentModelChange={setSubAgentModel}
            onOpenMCPConfig={() => setMCPConfigModalOpen(true)}
            onApplyPrompt={setPendingPrompt}
          />
        </div>
      </footer>

      {/* Settings Modal */}
      <ConversationSettingsModal
        open={settingsModalOpen}
        onClose={() => setSettingsModalOpen(false)}
        systemPrompt={systemPrompt}
        onSystemPromptChange={setSystemPrompt}
        actionConfig={actionConfig}
        onActionConfigChange={setActionConfig}
        vectorStoreIds={vectorStoreIds}
        onVectorStoreIdsChange={setVectorStoreIds}
        vectorStoreOwnerType={vectorStoreOwnerType}
        vectorStoreOwnerId={vectorStoreOwnerId}
        clientSideRAG={clientSideRAG}
        onClientSideRAGChange={setClientSideRAG}
        maxToolIterations={maxToolIterations}
        onMaxToolIterationsChange={setMaxToolIterations}
        captureRawSSEEvents={captureRawSSEEvents}
        onCaptureRawSSEEventsChange={setCaptureRawSSEEvents}
        ttsVoice={ttsVoice}
        onTTSVoiceChange={setTTSVoice}
        ttsSpeed={ttsSpeed}
        onTTSSpeedChange={setTTSSpeed}
      />

      {/* MCP Config Modal */}
      <MCPConfigModal
        open={mcpConfigModalOpen}
        onClose={() => {
          setMCPConfigModalOpen(false);
          setMcpPrefill(null);
        }}
        prefill={mcpPrefill}
      />
    </div>
  );
}
