// Zustand stores for chat UI state management
// These stores separate concerns to minimize re-renders during streaming

export {
  useStreamingStore,
  useStreamContent,
  useStreamState,
  useIsStreaming,
  useAllStreams,
  type StreamingResponse,
  type StreamingStore,
} from "./streamingStore";

export {
  useConversationStore,
  useMessages,
  useMessage,
  useSelectedModels,
  useCurrentConversationMeta,
  useConversations,
  type ConversationStore,
} from "./conversationStore";

export {
  useChatUIStore,
  useViewMode,
  useExpandedModel,
  useUserHasScrolledUp,
  useSystemPrompt,
  useDisabledModels,
  useSelectedBestResponses,
  useModelSettings,
  useActionConfig,
  type ViewMode,
  type ChatUIStore,
} from "./chatUIStore";

export {
  useWebSocketStore,
  useWebSocketStatus,
  useIsWebSocketConnected,
  useIsWebSocketConnecting,
  useWebSocketError,
  useWebSocketTopics,
  getWebSocketClient,
  type WebSocketStore,
} from "./websocketStore";
