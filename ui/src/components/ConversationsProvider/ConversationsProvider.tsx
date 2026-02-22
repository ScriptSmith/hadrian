import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  type ReactNode,
} from "react";
import { useMutation, useQuery } from "@tanstack/react-query";

import {
  conversationCreate,
  conversationDelete,
  conversationListAccessibleForUser,
  conversationSetPin,
  conversationUpdate,
} from "@/api/generated/sdk.gen";
import type { ConversationWithProject, Message } from "@/api/generated/types.gen";
import { useAuth } from "@/auth";
import { useIndexedDB } from "@/hooks/useIndexedDB";

import type { ChatMessage, Conversation } from "@/components/chat-types";
import { usePreferences } from "@/preferences/PreferencesProvider";
import { generateSimpleTitle, generateTitleWithLLM } from "@/utils/generateTitle";

const STORAGE_KEY = "hadrian-conversations";
const BROADCAST_CHANNEL = "hadrian-conversations-sync";

const SYNC_DEBOUNCE_MS = 2000;
const MAX_RETRY_ATTEMPTS = 3;
const BASE_RETRY_DELAY_MS = 1000;

interface StoredConversation {
  id: string;
  title: string;
  messages: Array<{
    id: string;
    role: "user" | "assistant" | "system";
    content: string;
    model?: string;
    timestamp: string;
    files?: Array<{
      id: string;
      name: string;
      type: string;
      size: number;
      base64: string;
      preview?: string;
    }>;
  }>;
  models: string[];
  createdAt: string;
  updatedAt: string;
  projectId?: string;
  projectName?: string;
  /** Pin order: null/undefined = not pinned, 0-N = pinned with order (lower = higher in list) */
  pinOrder?: number | null;
  /** Usage from LLM-based title generation (if used) */
  titleGenerationUsage?: {
    inputTokens: number;
    outputTokens: number;
    totalTokens: number;
    cost?: number;
  };
  // Sync tracking
  syncedAt?: string;
  remoteId?: string;
}

// BroadcastChannel message types for cross-tab synchronization
type SyncMessage = { type: "sync"; conversations: StoredConversation[] } | { type: "request_sync" };

function serializeConversations(conversations: Conversation[]): StoredConversation[] {
  return conversations.map((c) => ({
    ...c,
    createdAt: c.createdAt.toISOString(),
    updatedAt: c.updatedAt.toISOString(),
    messages: c.messages.map((m) => ({
      ...m,
      timestamp: m.timestamp.toISOString(),
      // Files including base64 are stored in IndexedDB which has much larger limits than localStorage
    })),
  }));
}

function deserializeConversations(stored: StoredConversation[]): Conversation[] {
  return stored.map((c) => ({
    ...c,
    createdAt: new Date(c.createdAt),
    updatedAt: new Date(c.updatedAt),
    messages: c.messages.map((m) => ({
      ...m,
      timestamp: new Date(m.timestamp),
    })),
  }));
}

function generateTitleFromMessages(messages: ChatMessage[]): string {
  const firstUserMessage = messages.find((m) => m.role === "user");
  if (!firstUserMessage) return "New Chat";
  return generateSimpleTitle(firstUserMessage.content);
}

function apiToLocal(api: ConversationWithProject): StoredConversation {
  return {
    id: api.id,
    remoteId: api.id,
    title: api.title,
    messages: api.messages.map((m, index) => ({
      id: crypto.randomUUID(),
      role: m.role as "user" | "assistant" | "system",
      content: m.content,
      model: undefined,
      timestamp: new Date(new Date(api.created_at).getTime() + index).toISOString(),
    })),
    models: api.models ?? [],
    createdAt: api.created_at,
    updatedAt: api.updated_at,
    syncedAt: new Date().toISOString(),
    projectId: api.project_id ?? undefined,
    projectName: api.project_name ?? undefined,
    pinOrder: api.pin_order ?? undefined,
  };
}

function localToApiMessage(m: StoredConversation["messages"][0]): Message {
  return {
    role: m.role,
    content: m.content,
  };
}

// Compute a sync hash that includes actual content changes
function computeSyncHash(conversations: StoredConversation[]): string {
  return JSON.stringify(
    conversations.map((c) => ({
      id: c.id,
      title: c.title,
      // Include message content hash for detecting content changes
      msgHash: c.messages
        .map((m) => `${m.role}:${m.content.length}:${m.content.slice(0, 50)}`)
        .join("|"),
      models: c.models.join(","),
      updatedAt: c.updatedAt,
      remoteId: c.remoteId,
      syncedAt: c.syncedAt,
    }))
  );
}

// Helper for exponential backoff retry
async function withRetry<T>(
  fn: () => Promise<T>,
  maxAttempts: number = MAX_RETRY_ATTEMPTS,
  baseDelay: number = BASE_RETRY_DELAY_MS
): Promise<T> {
  let lastError: Error | undefined;
  for (let attempt = 0; attempt < maxAttempts; attempt++) {
    try {
      return await fn();
    } catch (error) {
      lastError = error instanceof Error ? error : new Error(String(error));
      if (attempt < maxAttempts - 1) {
        const delay = baseDelay * Math.pow(2, attempt);
        await new Promise((resolve) => setTimeout(resolve, delay));
      }
    }
  }
  throw lastError;
}

/** Options for forking a conversation */
export interface ForkConversationOptions {
  /** If set, fork only messages up to and including this message ID */
  upToMessageId?: string;
  /** Custom title for the forked conversation (defaults to "Original Title (fork)") */
  newTitle?: string;
  /** Models to include in the fork (defaults to all models from source) */
  models?: string[];
  /** Target project ID for the fork (null = personal, undefined = same as source) */
  projectId?: string | null;
  /** Project name for display (only used if projectId is set) */
  projectName?: string;
}

interface ConversationsContextValue {
  conversations: Conversation[];
  /** True while loading conversations from IndexedDB */
  isLoading: boolean;
  createConversation: (models: string[], projectId?: string, projectName?: string) => Conversation;
  updateConversation: (id: string, messages: ChatMessage[], models?: string[]) => void;
  deleteConversation: (id: string) => void;
  renameConversation: (id: string, title: string) => void;
  /** Regenerate title using LLM based on the first user message */
  regenerateTitle: (id: string) => Promise<void>;
  /** Toggle pin on a conversation. If pinned, unpins. If unpinned, pins at position 0 (top). */
  togglePin: (id: string) => void;
  /** Set pin order for a conversation. Pass null to unpin. */
  setPinOrder: (id: string, pinOrder: number | null) => void;
  /** Reorder pinned conversations. Takes array of conversation IDs in desired order. */
  reorderPinned: (orderedIds: string[]) => void;
  /** Move a conversation to a project (or back to personal if projectId is null) */
  moveToProject: (id: string, projectId: string | null, projectName?: string) => Promise<void>;
  /** Fork a conversation, creating a new conversation with cloned messages */
  forkConversation: (sourceId: string, options?: ForkConversationOptions) => Conversation;
  /** Add a remotely-fetched conversation into local state */
  addRemoteConversation: (conv: ConversationWithProject) => void;
}

const ConversationsContext = createContext<ConversationsContextValue | null>(null);

interface ConversationsProviderProps {
  children: ReactNode;
}

export function ConversationsProvider({ children }: ConversationsProviderProps) {
  const { user, isAuthenticated } = useAuth();
  const { preferences } = usePreferences();
  const {
    value: storedConversations,
    setValue: setStoredConversations,
    isLoading,
  } = useIndexedDB<StoredConversation[]>(STORAGE_KEY, []);
  const syncTimeoutRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const lastSyncHashRef = useRef<string>("");
  const pendingDeletesRef = useRef<Set<string>>(new Set());
  const broadcastChannelRef = useRef<BroadcastChannel | null>(null);
  const isSyncingRef = useRef<boolean>(false);
  const storedConversationsRef = useRef<StoredConversation[]>(storedConversations);

  // Keep ref in sync with state (for use in callbacks without causing re-renders)
  storedConversationsRef.current = storedConversations;

  // Only sync if we have a user_id
  const canSync = isAuthenticated && !!user?.id;
  const userId = user?.id;

  // Memoize conversations to avoid creating new Date objects on every render
  const conversations = useMemo(
    () => deserializeConversations(storedConversations),
    [storedConversations]
  );

  const setConversations = useCallback(
    (updater: (prev: Conversation[]) => Conversation[]) => {
      setStoredConversations((prev) => {
        const currentConvs = deserializeConversations(prev);
        const newConvs = updater(currentConvs);
        return serializeConversations(newConvs);
      });
    },
    [setStoredConversations]
  );

  // Fetch remote conversations (only when authenticated with user_id)
  // Uses the accessible endpoint to include both personal and project conversations
  const { data: remoteConversations } = useQuery({
    queryKey: ["conversations", "accessible", userId],
    queryFn: async () => {
      if (!userId) return [];
      try {
        const response = await conversationListAccessibleForUser({
          path: { user_id: userId },
          query: { limit: 100 },
        });
        return response.data?.data ?? [];
      } catch (error) {
        console.warn("Failed to fetch remote conversations:", error);
        return [];
      }
    },
    enabled: canSync,
    staleTime: 60000,
  });

  // Merge remote conversations into localStorage on initial load
  useEffect(() => {
    if (!remoteConversations?.length) return;

    setStoredConversations((prev) => {
      const localIds = new Set(prev.map((c) => c.id));
      const localRemoteIds = new Set(prev.map((c) => c.remoteId).filter(Boolean));

      let updated = false;
      const newLocal = [...prev];

      for (const remote of remoteConversations) {
        if (localIds.has(remote.id) || localRemoteIds.has(remote.id)) {
          continue;
        }
        newLocal.unshift(apiToLocal(remote));
        updated = true;
      }

      if (!updated) return prev;

      // Sort by updatedAt descending
      newLocal.sort((a, b) => new Date(b.updatedAt).getTime() - new Date(a.updatedAt).getTime());
      return newLocal;
    });
  }, [remoteConversations, setStoredConversations]);

  // Mutations for creating/updating/deleting conversations
  const createMutation = useMutation({
    mutationFn: async (conv: StoredConversation) => {
      if (!userId) throw new Error("No user ID");
      const response = await conversationCreate({
        body: {
          owner: { type: "user", user_id: userId },
          title: conv.title,
          models: conv.models,
          messages: conv.messages.map(localToApiMessage),
        },
      });
      return { localId: conv.id, remoteId: response.data?.id };
    },
  });

  const updateMutation = useMutation({
    mutationFn: async ({ remoteId, conv }: { remoteId: string; conv: StoredConversation }) => {
      await conversationUpdate({
        path: { id: remoteId },
        body: {
          title: conv.title,
          models: conv.models,
          messages: conv.messages.map(localToApiMessage),
        },
      });
    },
  });

  const deleteMutation = useMutation({
    mutationFn: async (remoteId: string) => {
      await conversationDelete({ path: { id: remoteId } });
    },
  });

  const pinMutation = useMutation({
    mutationFn: async ({ remoteId, pinOrder }: { remoteId: string; pinOrder: number | null }) => {
      await conversationSetPin({
        path: { id: remoteId },
        body: { pin_order: pinOrder },
      });
    },
  });

  // Background sync function - uses React state instead of direct localStorage
  const syncToApi = useCallback(async () => {
    if (!canSync || !userId) return;

    // Prevent concurrent syncs
    if (isSyncingRef.current) return;
    isSyncingRef.current = true;

    try {
      // Use React state instead of direct localStorage access
      const currentHash = computeSyncHash(storedConversations);

      // Skip if nothing changed
      if (currentHash === lastSyncHashRef.current) return;
      lastSyncHashRef.current = currentHash;

      // Collect updates to apply atomically at the end
      const updates: Array<{ id: string; remoteId?: string; syncedAt: string }> = [];

      for (const conv of storedConversations) {
        // Skip if already pending delete
        if (pendingDeletesRef.current.has(conv.id)) continue;

        try {
          if (!conv.remoteId) {
            // Create new conversation in API with retry
            const result = await withRetry(() => createMutation.mutateAsync(conv));
            if (result.remoteId) {
              updates.push({
                id: conv.id,
                remoteId: result.remoteId,
                syncedAt: new Date().toISOString(),
              });
            }
          } else if (conv.remoteId && (!conv.syncedAt || conv.updatedAt > conv.syncedAt)) {
            // Update existing conversation with retry
            await withRetry(() => updateMutation.mutateAsync({ remoteId: conv.remoteId!, conv }));
            updates.push({
              id: conv.id,
              syncedAt: new Date().toISOString(),
            });
          }
        } catch (error) {
          console.warn("Failed to sync conversation after retries:", conv.id, error);
        }
      }

      // Apply all updates atomically via React state
      if (updates.length > 0) {
        setStoredConversations((prev) => {
          const updated = [...prev];
          for (const update of updates) {
            const idx = updated.findIndex((c) => c.id === update.id);
            if (idx !== -1) {
              updated[idx] = {
                ...updated[idx],
                remoteId: update.remoteId ?? updated[idx].remoteId,
                syncedAt: update.syncedAt,
              };
            }
          }
          return updated;
        });

        // Broadcast to other tabs
        broadcastChannelRef.current?.postMessage({
          type: "sync",
          conversations: storedConversations,
        } satisfies SyncMessage);
      }
    } finally {
      isSyncingRef.current = false;
    }
  }, [
    canSync,
    userId,
    storedConversations,
    createMutation,
    updateMutation,
    setStoredConversations,
  ]);

  // Debounced sync trigger
  const triggerSync = useCallback(() => {
    if (syncTimeoutRef.current) {
      clearTimeout(syncTimeoutRef.current);
    }
    syncTimeoutRef.current = setTimeout(syncToApi, SYNC_DEBOUNCE_MS);
  }, [syncToApi]);

  // Trigger sync when storedConversations changes
  useEffect(() => {
    if (!canSync) return;
    triggerSync();
  }, [storedConversations, canSync, triggerSync]);

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      if (syncTimeoutRef.current) {
        clearTimeout(syncTimeoutRef.current);
      }
    };
  }, []);

  // BroadcastChannel for cross-tab synchronization
  useEffect(() => {
    if (typeof BroadcastChannel === "undefined") return;

    const channel = new BroadcastChannel(BROADCAST_CHANNEL);
    broadcastChannelRef.current = channel;

    channel.onmessage = (event: MessageEvent<SyncMessage>) => {
      const message = event.data;

      if (message.type === "sync") {
        // Another tab has synced - merge with local state using last-writer-wins
        setStoredConversations((prev) => {
          const incoming = message.conversations;
          const merged = [...prev];

          for (const remote of incoming) {
            const localIdx = merged.findIndex((c) => c.id === remote.id);
            if (localIdx === -1) {
              // New conversation from other tab
              merged.push(remote);
            } else {
              // Use last-writer-wins based on updatedAt
              const local = merged[localIdx];
              if (new Date(remote.updatedAt) > new Date(local.updatedAt)) {
                merged[localIdx] = remote;
              }
            }
          }

          // Sort by updatedAt descending
          merged.sort((a, b) => new Date(b.updatedAt).getTime() - new Date(a.updatedAt).getTime());
          return merged;
        });
      } else if (message.type === "request_sync") {
        // Another tab is requesting sync - send current state via ref to avoid stale closure
        channel.postMessage({
          type: "sync",
          conversations: storedConversationsRef.current,
        } satisfies SyncMessage);
      }
    };

    // Request sync from other tabs on mount
    channel.postMessage({ type: "request_sync" } satisfies SyncMessage);

    return () => {
      channel.close();
      broadcastChannelRef.current = null;
    };
  }, [setStoredConversations]);

  const createConversation = useCallback(
    (models: string[], projectId?: string, projectName?: string): Conversation => {
      const now = new Date();
      const newConversation: Conversation = {
        id: crypto.randomUUID(),
        title: "New Chat",
        messages: [],
        models,
        createdAt: now,
        updatedAt: now,
        projectId,
        projectName,
      };
      setConversations((prev) => [newConversation, ...prev]);
      return newConversation;
    },
    [setConversations]
  );

  // Track conversations that are pending LLM title generation to avoid duplicate calls
  const pendingTitleGenRef = useRef<Set<string>>(new Set());

  const updateConversation = useCallback(
    (id: string, messages: ChatMessage[], models?: string[]) => {
      let needsLLMTitle = false;
      let firstUserMessage: string | undefined;

      setConversations((prev) =>
        prev.map((c) => {
          if (c.id !== id) return c;

          // Check if we need to generate a title
          if (c.title === "New Chat") {
            const userMsg = messages.find((m) => m.role === "user");
            if (userMsg) {
              firstUserMessage = userMsg.content;
              // Only trigger LLM if we haven't already and there's an assistant response
              const hasAssistantResponse = messages.some((m) => m.role === "assistant");
              needsLLMTitle = hasAssistantResponse && !pendingTitleGenRef.current.has(id);
            }
          }

          const title = c.title === "New Chat" ? generateTitleFromMessages(messages) : c.title;

          return {
            ...c,
            title,
            messages,
            models: models ?? c.models,
            updatedAt: new Date(),
          };
        })
      );

      // Trigger async LLM title generation after state update (if model is configured)
      const titleModel = preferences.titleGenerationModel;
      if (needsLLMTitle && firstUserMessage && titleModel) {
        pendingTitleGenRef.current.add(id);
        generateTitleWithLLM(firstUserMessage, titleModel)
          .then((result) => {
            // Only update if the title is different and better
            setConversations((prev) =>
              prev.map((c) => {
                if (c.id !== id) return c;
                // Only update if still using auto-generated title (not manually renamed)
                const currentSimpleTitle = generateTitleFromMessages(c.messages);
                if (c.title === currentSimpleTitle && result.title !== currentSimpleTitle) {
                  return {
                    ...c,
                    title: result.title,
                    updatedAt: new Date(),
                    titleGenerationUsage: result.usage,
                  };
                }
                return c;
              })
            );
          })
          .finally(() => {
            pendingTitleGenRef.current.delete(id);
          });
      }
    },
    [setConversations, preferences.titleGenerationModel]
  );

  const deleteConversation = useCallback(
    (id: string) => {
      // Find the conversation to get remoteId before deleting
      const conv = storedConversations.find((c) => c.id === id);
      if (conv?.remoteId && canSync) {
        pendingDeletesRef.current.add(id);
        deleteMutation.mutate(conv.remoteId, {
          onSettled: () => {
            pendingDeletesRef.current.delete(id);
          },
        });
      }
      setConversations((prev) => prev.filter((c) => c.id !== id));
    },
    [setConversations, storedConversations, canSync, deleteMutation]
  );

  const renameConversation = useCallback(
    (id: string, title: string) => {
      setConversations((prev) =>
        prev.map((c) => (c.id === id ? { ...c, title, updatedAt: new Date() } : c))
      );
    },
    [setConversations]
  );

  const regenerateTitle = useCallback(
    async (id: string): Promise<void> => {
      const conv = conversations.find((c) => c.id === id);
      if (!conv) return;

      // Find the first user message
      const firstUserMessage = conv.messages.find((m) => m.role === "user");
      if (!firstUserMessage) return;

      const titleModel = preferences.titleGenerationModel;
      if (!titleModel) {
        // No LLM configured, use simple title generation
        const simpleTitle = generateSimpleTitle(firstUserMessage.content);
        setConversations((prev) =>
          prev.map((c) => (c.id === id ? { ...c, title: simpleTitle, updatedAt: new Date() } : c))
        );
        return;
      }

      // Generate title with LLM
      const result = await generateTitleWithLLM(firstUserMessage.content, titleModel);
      setConversations((prev) =>
        prev.map((c) =>
          c.id === id
            ? {
                ...c,
                title: result.title,
                updatedAt: new Date(),
                // Accumulate title generation usage if regenerating
                titleGenerationUsage: result.usage
                  ? {
                      inputTokens:
                        (c.titleGenerationUsage?.inputTokens ?? 0) + result.usage.inputTokens,
                      outputTokens:
                        (c.titleGenerationUsage?.outputTokens ?? 0) + result.usage.outputTokens,
                      totalTokens:
                        (c.titleGenerationUsage?.totalTokens ?? 0) + result.usage.totalTokens,
                      cost: (c.titleGenerationUsage?.cost ?? 0) + (result.usage.cost ?? 0),
                    }
                  : c.titleGenerationUsage,
              }
            : c
        )
      );
    },
    [conversations, setConversations, preferences.titleGenerationModel]
  );

  const setPinOrder = useCallback(
    (id: string, pinOrder: number | null) => {
      // Find the conversation to get remoteId
      const conv = storedConversations.find((c) => c.id === id);

      // Sync to API if we have a remoteId
      if (conv?.remoteId && canSync) {
        pinMutation.mutate({ remoteId: conv.remoteId, pinOrder });
      }

      // Update local state immediately
      setConversations((prev) =>
        prev.map((c) => (c.id === id ? { ...c, pinOrder, updatedAt: new Date() } : c))
      );
    },
    [setConversations, storedConversations, canSync, pinMutation]
  );

  const togglePin = useCallback(
    (id: string) => {
      const conv = storedConversations.find((c) => c.id === id);
      if (!conv) return;

      // If currently pinned, unpin. Otherwise, pin at position 0.
      const newPinOrder = conv.pinOrder != null ? null : 0;

      // If pinning at 0, shift other pins down
      if (newPinOrder === 0) {
        // Update all currently pinned conversations to shift their order
        setStoredConversations((prev) =>
          prev.map((c) => {
            if (c.id === id) {
              return { ...c, pinOrder: 0, updatedAt: new Date().toISOString() };
            }
            if (c.pinOrder != null) {
              return { ...c, pinOrder: c.pinOrder + 1 };
            }
            return c;
          })
        );

        // Sync the pin to API
        if (conv.remoteId && canSync) {
          pinMutation.mutate({ remoteId: conv.remoteId, pinOrder: 0 });
        }
      } else {
        // Unpinning - just set to null
        setPinOrder(id, null);
      }
    },
    [storedConversations, setStoredConversations, canSync, pinMutation, setPinOrder]
  );

  const reorderPinned = useCallback(
    (orderedIds: string[]) => {
      // Update local state with new pin orders
      setStoredConversations((prev) => {
        const updated = prev.map((c) => {
          const newOrder = orderedIds.indexOf(c.id);
          if (newOrder !== -1) {
            return { ...c, pinOrder: newOrder };
          }
          return c;
        });
        return updated;
      });

      // Sync each reordered pin to API
      if (canSync) {
        orderedIds.forEach((id, index) => {
          const conv = storedConversations.find((c) => c.id === id);
          if (conv?.remoteId) {
            pinMutation.mutate({ remoteId: conv.remoteId, pinOrder: index });
          }
        });
      }
    },
    [storedConversations, setStoredConversations, canSync, pinMutation]
  );

  const moveToProject = useCallback(
    async (id: string, projectId: string | null, projectName?: string): Promise<void> => {
      const conv = storedConversations.find((c) => c.id === id);
      if (!conv) {
        throw new Error("Conversation not found");
      }

      if (!canSync || !userId) {
        throw new Error("Must be authenticated to move conversations");
      }

      // If not synced yet, sync first
      let remoteId = conv.remoteId;
      if (!remoteId) {
        const result = await createMutation.mutateAsync(conv);
        if (!result.remoteId) {
          throw new Error("Failed to sync conversation");
        }
        remoteId = result.remoteId;

        // Update local state with remoteId
        setStoredConversations((prev) =>
          prev.map((c) =>
            c.id === id ? { ...c, remoteId, syncedAt: new Date().toISOString() } : c
          )
        );
      }

      // Call the API to update the owner
      const owner = projectId
        ? { type: "project" as const, project_id: projectId }
        : { type: "user" as const, user_id: userId };

      await conversationUpdate({
        path: { id: remoteId },
        body: { owner },
      });

      // Update local state
      setStoredConversations((prev) =>
        prev.map((c) => {
          if (c.id !== id) return c;
          return {
            ...c,
            projectId: projectId ?? undefined,
            projectName: projectId ? projectName : undefined,
            updatedAt: new Date().toISOString(),
            syncedAt: new Date().toISOString(),
          };
        })
      );
    },
    [storedConversations, setStoredConversations, canSync, userId, createMutation]
  );

  const forkConversation = useCallback(
    (sourceId: string, options?: ForkConversationOptions): Conversation => {
      const source = conversations.find((c) => c.id === sourceId);
      if (!source) {
        throw new Error("Source conversation not found");
      }

      // Get messages up to the specified point (or all)
      let messagesToClone = source.messages;
      if (options?.upToMessageId) {
        const idx = source.messages.findIndex((m) => m.id === options.upToMessageId);
        if (idx !== -1) {
          messagesToClone = source.messages.slice(0, idx + 1);
        }
      }

      // Determine which models to include
      const modelsToInclude = options?.models ?? source.models;
      const modelsSet = new Set(modelsToInclude);

      // Deep clone messages with new IDs, stripping feedback/debug info
      // Filter out assistant messages from excluded models
      const clonedMessages: ChatMessage[] = messagesToClone
        .filter((m) => {
          // Keep all user and system messages
          if (m.role !== "assistant") return true;
          // For assistant messages, only keep if model is in the included set
          return m.model ? modelsSet.has(m.model) : true;
        })
        .map((m) => ({
          ...m,
          id: crypto.randomUUID(),
          timestamp: new Date(m.timestamp), // Clone the date object
          feedback: undefined, // Don't copy user feedback
          debugMessageId: undefined, // Don't copy debug reference
          // Keep: content, model, instanceId, files, usage, historyMode,
          // modeMetadata, error, citations, artifacts, toolExecutionRounds
        }));

      // Determine project settings
      // If projectId is explicitly set (including null), use that value
      // If undefined, keep same as source
      const projectId = options?.projectId !== undefined ? options.projectId : source.projectId;
      const projectName =
        options?.projectId !== undefined ? options.projectName : source.projectName;

      const now = new Date();
      const forked: Conversation = {
        id: crypto.randomUUID(),
        title: options?.newTitle ?? `${source.title} (fork)`,
        messages: clonedMessages,
        models: modelsToInclude,
        createdAt: now,
        updatedAt: now,
        projectId: projectId ?? undefined,
        projectName: projectName,
        // Don't copy pinOrder - forks aren't pinned
      };

      setConversations((prev) => [forked, ...prev]);
      return forked;
    },
    [conversations, setConversations]
  );

  const addRemoteConversation = useCallback(
    (conv: ConversationWithProject) => {
      setStoredConversations((prev) => {
        if (prev.some((c) => c.id === conv.id || c.remoteId === conv.id)) return prev;
        const local = apiToLocal(conv);
        return [local, ...prev].sort(
          (a, b) => new Date(b.updatedAt).getTime() - new Date(a.updatedAt).getTime()
        );
      });
    },
    [setStoredConversations]
  );

  // Memoize context value to prevent unnecessary re-renders of consumers
  const contextValue = useMemo(
    () => ({
      conversations,
      isLoading,
      createConversation,
      updateConversation,
      deleteConversation,
      renameConversation,
      regenerateTitle,
      togglePin,
      setPinOrder,
      reorderPinned,
      moveToProject,
      forkConversation,
      addRemoteConversation,
    }),
    [
      conversations,
      isLoading,
      createConversation,
      updateConversation,
      deleteConversation,
      renameConversation,
      regenerateTitle,
      togglePin,
      setPinOrder,
      reorderPinned,
      moveToProject,
      forkConversation,
      addRemoteConversation,
    ]
  );

  return (
    <ConversationsContext.Provider value={contextValue}>{children}</ConversationsContext.Provider>
  );
}

export function useConversationsContext(): ConversationsContextValue {
  const context = useContext(ConversationsContext);
  if (!context) {
    throw new Error("useConversationsContext must be used within a ConversationsProvider");
  }
  return context;
}
