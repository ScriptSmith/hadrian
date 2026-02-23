import { useEffect, useRef, useCallback, useMemo, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { useQuery } from "@tanstack/react-query";

import { apiV1ModelsOptions } from "@/api/generated/@tanstack/react-query.gen";
import { ChatView, type ChatFile } from "@/components/ChatView/ChatView";
import { useConversationsContext } from "@/components/ConversationsProvider/ConversationsProvider";
import {
  ForkConversationModal,
  type ForkConversationResult,
} from "@/components/ForkConversationModal/ForkConversationModal";
import type { ModelInfo } from "@/components/ModelSelector/ModelSelector";
import { useConversationSync } from "@/hooks/useConversationSync";
import { usePreferences } from "@/preferences/PreferencesProvider";
import { useConversationStore, useSelectedModels, useMessages } from "@/stores/conversationStore";
import {
  useSystemPrompt,
  useDisabledModels,
  useHistoryMode,
  useConversationMode,
  useModeConfig,
  usePerModelSettings,
  useVectorStoreIds,
  useClientSideRAG,
  useEnabledTools,
  useDataFiles,
  useCaptureRawSSEEvents,
  useSubAgentModel,
} from "@/stores/chatUIStore";

import type { ModelSettings } from "./types";
import { useChat } from "./useChat";

export default function ChatPage() {
  const { conversationId } = useParams();
  const navigate = useNavigate();
  const { preferences } = usePreferences();

  // Fetch models from API
  const { data: modelsResponse, isPending: isLoadingModels } = useQuery(apiV1ModelsOptions());
  const availableModels: ModelInfo[] = useMemo(
    () => modelsResponse?.data?.map((m) => m as ModelInfo).filter((m) => m.id) || [],
    [modelsResponse?.data]
  );

  // Use stores directly - they are the source of truth
  const selectedModels = useSelectedModels();
  const disabledModels = useDisabledModels();
  const messages = useMessages();
  const systemPrompt = useSystemPrompt();
  const historyMode = useHistoryMode();
  const conversationMode = useConversationMode();
  const modeConfig = useModeConfig();
  const perModelSettings = usePerModelSettings();
  const vectorStoreIds = useVectorStoreIds();
  const clientSideRAG = useClientSideRAG();
  const enabledTools = useEnabledTools();
  const dataFiles = useDataFiles();
  const captureRawSSEEvents = useCaptureRawSSEEvents();
  const subAgentModel = useSubAgentModel();

  const { setSelectedModels } = useConversationStore();

  // Track whether we've initialized default models from preferences
  const hasInitializedModelsRef = useRef(false);

  // Active models are selected models that aren't disabled
  const activeModels = useMemo(
    () => selectedModels.filter((m) => !disabledModels.includes(m)),
    [selectedModels, disabledModels]
  );

  // Build settings for useChat - only include systemPrompt
  // Per-model parameters come from perModelSettings
  const modelSettings: ModelSettings = useMemo(
    () => ({
      systemPrompt: systemPrompt || undefined,
    }),
    [systemPrompt]
  );

  // Set default models from preferences when models load (only once on initial load)
  useEffect(() => {
    if (hasInitializedModelsRef.current || availableModels.length === 0) return;

    hasInitializedModelsRef.current = true;
    const defaultModels = preferences.defaultModels?.chat || [];
    const validDefaults = defaultModels.filter((m) => availableModels.some((am) => am.id === m));
    if (validDefaults.length > 0) {
      setSelectedModels(validDefaults);
    }
  }, [availableModels, preferences.defaultModels, setSelectedModels]);

  // Enable client-side tool execution when:
  // 1. clientSideRAG is enabled (for client-side file_search)
  // 2. file_search is enabled (executes against vector stores)
  // 3. code_interpreter is enabled (it's a client-side only tool)
  // 4. js_code_interpreter is enabled (it's a client-side only tool)
  // 5. sql_query is enabled (it's a client-side only tool)
  // 6. chart_render is enabled (it's a client-side only tool)
  // 7. html_render is enabled (it's a client-side only tool)
  // 8. sub_agent is enabled (it's a client-side only tool)
  // 9. mcp is enabled (MCP tools are executed client-side)
  // 10. wikipedia is enabled (it's a client-side only tool)
  // 11. wikidata is enabled (it's a client-side only tool)
  const clientSideToolExecution =
    clientSideRAG ||
    enabledTools.includes("file_search") ||
    enabledTools.includes("code_interpreter") ||
    enabledTools.includes("js_code_interpreter") ||
    enabledTools.includes("sql_query") ||
    enabledTools.includes("chart_render") ||
    enabledTools.includes("html_render") ||
    enabledTools.includes("sub_agent") ||
    enabledTools.includes("mcp") ||
    enabledTools.includes("wikipedia") ||
    enabledTools.includes("wikidata");

  // Pass only active (non-disabled) models to useChat
  // Filter to only registered data files for SQL context
  const registeredDataFiles = useMemo(
    () =>
      dataFiles
        .filter((f) => f.registered)
        .map((f) => ({
          name: f.name,
          columns: f.columns,
        })),
    [dataFiles]
  );

  // Pending project selection for new conversations (before first message)
  const [pendingProject, setPendingProject] = useState<{
    id: string | null;
    name?: string;
  }>({ id: null });

  // Sync conversation state between persistence layer and stores
  // (must come before useChat so projectId is available)
  const { currentConversation, createConversation, forkConversation } =
    useConversationSync(conversationId);

  const {
    isStreaming,
    sendMessage,
    stopStreaming,
    clearMessages,
    regenerateResponse,
    editAndRerun,
  } = useChat({
    models: activeModels,
    settings: modelSettings,
    historyMode,
    conversationMode,
    modeConfig,
    perModelSettings,
    vectorStoreIds: vectorStoreIds.length > 0 ? vectorStoreIds : undefined,
    clientSideToolExecution,
    enabledTools,
    dataFiles: registeredDataFiles.length > 0 ? registeredDataFiles : undefined,
    captureRawSSEEvents,
    subAgentModel,
    projectId: currentConversation?.projectId ?? pendingProject.id ?? undefined,
  });

  const { moveToProject } = useConversationsContext();

  const handleProjectChange = useCallback(
    (projectId: string | null, projectName?: string) => {
      if (!currentConversation) return;
      moveToProject(currentConversation.id, projectId, projectName);
    },
    [currentConversation, moveToProject]
  );

  const handlePendingProjectChange = useCallback(
    (projectId: string | null, projectName?: string) => {
      setPendingProject({ id: projectId, name: projectName });
    },
    []
  );

  // Fork modal state
  const [forkModalOpen, setForkModalOpen] = useState(false);
  const [forkMessageId, setForkMessageId] = useState<string | undefined>(undefined);

  const handleSendMessage = useCallback(
    (content: string, files?: ChatFile[]) => {
      if (!currentConversation) {
        const newConv = createConversation(
          selectedModels,
          pendingProject.id ?? undefined,
          pendingProject.name
        );
        navigate(`/chat/${newConv.id}`, { replace: true });
        setPendingProject({ id: null });
      }
      sendMessage(content, files ?? []);
    },
    [currentConversation, createConversation, navigate, selectedModels, sendMessage, pendingProject]
  );

  // Handle regeneration of a single model response
  const handleRegenerate = useCallback(
    (userMessageId: string, model: string) => {
      regenerateResponse(userMessageId, model);
    },
    [regenerateResponse]
  );

  // Handle regeneration of all responses for a user message
  const handleRegenerateAll = useCallback(
    (messageId: string) => {
      const message = messages.find((m) => m.id === messageId);
      if (message && message.role === "user") {
        // Re-run with the same content (this deletes subsequent messages and re-queries all models)
        editAndRerun(messageId, message.content);
      }
    },
    [messages, editAndRerun]
  );

  // Handle forking conversation from a specific message - opens modal
  const handleForkFromMessage = useCallback(
    (messageId: string) => {
      if (!currentConversation) return;
      setForkMessageId(messageId);
      setForkModalOpen(true);
    },
    [currentConversation]
  );

  // Handle forking the entire current conversation - opens modal
  const handleForkConversation = useCallback(() => {
    if (!currentConversation) return;
    setForkMessageId(undefined);
    setForkModalOpen(true);
  }, [currentConversation]);

  // Handle the actual fork when modal confirms
  const handleForkConfirm = useCallback(
    (result: ForkConversationResult) => {
      if (!currentConversation) return;
      const forked = forkConversation(currentConversation.id, {
        upToMessageId: forkMessageId,
        newTitle: result.title,
        models: result.models,
        projectId: result.projectId,
        projectName: result.projectName,
      });
      navigate(`/chat/${forked.id}`);
    },
    [currentConversation, forkConversation, forkMessageId, navigate]
  );

  return (
    <>
      <ChatView
        availableModels={availableModels}
        conversation={currentConversation}
        isStreaming={isStreaming}
        isLoadingModels={isLoadingModels}
        onSendMessage={handleSendMessage}
        onStopStreaming={stopStreaming}
        onClearMessages={clearMessages}
        onRegenerate={handleRegenerate}
        onRegenerateAll={handleRegenerateAll}
        onForkFromMessage={handleForkFromMessage}
        onFork={handleForkConversation}
        onProjectChange={handleProjectChange}
        onPendingProjectChange={!currentConversation ? handlePendingProjectChange : undefined}
        pendingProjectName={pendingProject.name}
        onEditAndRerun={editAndRerun}
      />
      {currentConversation && (
        <ForkConversationModal
          open={forkModalOpen}
          onClose={() => setForkModalOpen(false)}
          conversation={currentConversation}
          upToMessageId={forkMessageId}
          onFork={handleForkConfirm}
        />
      )}
    </>
  );
}
