# Modifying the Chat UI

The chat UI is designed for high-performance multi-model streaming (50-100+ tokens/sec across multiple concurrent streams). Follow these patterns when modifying chat components.

## State Architecture (Six Zustand Stores)

The chat state is split across six stores to isolate high-frequency updates from UI re-renders:

| Store | Purpose | Persistence |
|-------|---------|-------------|
| **streamingStore** | High-frequency token updates (50-100+/sec) | Ephemeral (lost on reload) |
| **conversationStore** | Committed messages after streaming completes | IndexedDB via `useIndexedDB` hook |
| **chatUIStore** | View mode, expanded state, scroll position | Session-only |
| **mcpStore** | MCP server connections and tool discovery | localStorage via Zustand persist |
| **websocketStore** | WebSocket real-time events | Ephemeral |
| **debugStore** | Debug capture | Ephemeral |

This separation ensures token streaming doesn't trigger re-renders of the message list.

## Surgical Selectors (Critical)

Always use the provided selector hooks, never subscribe to entire stores:

```typescript
// GOOD - Only re-renders when this model's content changes
const content = useStreamContent("claude-opus");

// BAD - Re-renders on ANY store change
const { streams } = useStreamingStore();
```

Key selectors:
- `useStreamContent(model)` - Single model's streaming content
- `useMessages()` - Committed message array
- `useViewMode()` - Grid/stacked layout preference
- `useIsStreaming()` - Global streaming boolean

## Memoization Requirements

1. **Custom memo comparators**: `ChatMessage` and `MultiModelResponse` use custom `arePropsEqual` functions. When modifying these, ensure comparators check all props that affect rendering.

2. **Stable callbacks**: Parent components MUST use `useCallback` for callbacks passed to memoized children. The memo comparators check callback identity.

3. **Memo checklist** (what causes unnecessary re-renders):
   - New function reference on every render → use `useCallback`
   - New object/array reference → use `useMemo` or extract to module scope
   - Missing custom comparator → add `arePropsEqual` function

## Component Responsibilities

| Component | Re-renders On | Does NOT Re-render On |
|-----------|--------------|----------------------|
| `ChatMessageList` | `messages` array change | Token streaming |
| `MultiModelResponse` | Response content/status change | Sibling message changes |
| `ModelResponseCard` | Own model's content | Other models' content |
| `StreamingMarkdown` | `content` prop change | Parent re-renders (memo) |

## Streaming Architecture

1. User sends message → `streamingStore.initStreaming(models)`
2. Parallel SSE streams opened per model
3. `appendContent(model, delta)` called per token → only that model's card re-renders
4. `completeStream(model, usage)` when stream ends
5. `conversationStore.addAssistantMessages()` commits all responses → message list re-renders once
6. `streamingStore.clearStreams()` cleans up

## Virtualization

`ChatMessageList` uses `@tanstack/react-virtual`:
- Only visible message groups render in DOM
- Streaming responses render OUTSIDE virtualization (at bottom)
- This prevents constant height re-measurement during streaming

## Adding New Chat Features

1. **New state?** → Decide which store it belongs to (streaming/conversation/UI/websocket/debug)
2. **New component?** → Consider if it needs memoization
3. **New callbacks?** → Wrap in `useCallback` with tight dependencies
4. **New selectors?** → Create surgical selector hooks in the store file

## Model Instances

The chat supports **model instances** — multiple copies of the same model with different settings (e.g., compare GPT-4 with temperature 0.3 vs 0.9):

- `ModelInstance` type in `chat-types.ts` with `id`, `modelId`, `label`, `parameters`
- Streams and messages are keyed by **instance ID**, not model ID
- Chat modes use instance IDs for role assignment (synthesizer, router, coordinator, etc.)
- Instance labels appear in UI to distinguish copies of the same model

When working with multi-model features:
- Use `selectedInstances: ModelInstance[]` instead of `selectedModels: string[]`
- Look up parameters via `instance.parameters`, not global model settings
- Use `instance.id` for all tracking and state management

## Key Files

- `ui/src/stores/streamingStore.ts` - Token streaming state
- `ui/src/stores/conversationStore.ts` - Persistent messages
- `ui/src/stores/chatUIStore.ts` - UI preferences
- `ui/src/stores/mcpStore.ts` - MCP server connections (persisted)
- `ui/src/stores/websocketStore.ts` - WebSocket events
- `ui/src/stores/debugStore.ts` - Debug capture
- `ui/src/components/ChatMessageList/ChatMessageList.tsx` - Virtualized list
- `ui/src/components/MultiModelResponse/MultiModelResponse.tsx` - Model response cards
- `ui/src/hooks/useAutoScroll.ts` - Smart auto-scroll behavior
- `ui/src/hooks/useIndexedDB.ts` - IndexedDB persistence for conversations
