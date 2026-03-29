import { useEffect, useRef, useCallback } from "react";
import { useNavigate } from "react-router-dom";
import { useQuery } from "@tanstack/react-query";

import { conversationGet } from "@/api/generated/sdk.gen";
import type { ConversationWithProject } from "@/api/generated/types.gen";
import {
  useConversationsContext,
  type ForkConversationOptions,
} from "@/components/ConversationsProvider/ConversationsProvider";
import { useConversationStore } from "@/stores/conversationStore";
import { useIsStreaming } from "@/stores/streamingStore";
import { useChatUIStore } from "@/stores/chatUIStore";

/**
 * Hook that synchronizes the current conversation between the persistence layer
 * (ConversationsProvider) and the in-memory state (conversationStore).
 *
 * This hook handles:
 * 1. Loading conversation data into stores when conversationId changes
 * 2. Saving conversation data back to persistence when messages change
 * 3. Debouncing saves to avoid excessive writes
 * 4. Fetching conversations from the API when not found locally (direct URL navigation)
 * 5. Updating the URL to use the server-assigned remoteId for shareable links
 *
 * @param conversationId - The ID of the current conversation from URL params
 */
export function useConversationSync(conversationId: string | undefined) {
  const {
    conversations,
    isLoading,
    createConversation,
    updateConversation,
    forkConversation,
    addRemoteConversation,
  } = useConversationsContext();

  const navigate = useNavigate();
  const isStreaming = useIsStreaming();

  // Get store state and actions
  const messages = useConversationStore((state) => state.messages);
  const selectedModels = useConversationStore((state) => state.selectedModels);
  const setMessages = useConversationStore((state) => state.setMessages);
  const setSelectedModels = useConversationStore((state) => state.setSelectedModels);
  const clearMessages = useConversationStore((state) => state.clearMessages);

  const { setDisabledModels, clearSelectedBestResponses } = useChatUIStore();

  // Find the current conversation from the provider (check both local id and server remoteId)
  const currentConversation =
    conversations.find((c) => c.id === conversationId || c.remoteId === conversationId) ?? null;

  // Fetch from API when navigating directly to a conversation URL not in local state.
  // This enables shareable URLs — the conversationId in the URL is the server-assigned
  // remoteId which can be fetched from the API.
  const { data: remoteConversation } = useQuery({
    queryKey: ["conversation", conversationId],
    queryFn: async () => {
      const response = await conversationGet({ path: { id: conversationId! } });
      return (response.data ?? null) as ConversationWithProject | null;
    },
    enabled: !!conversationId && !currentConversation && !isLoading,
    retry: false,
    staleTime: Infinity,
  });

  // Merge the fetched conversation into local state
  useEffect(() => {
    if (remoteConversation) {
      addRemoteConversation(remoteConversation);
    }
  }, [remoteConversation, addRemoteConversation]);

  // Once a remoteId is assigned (after sync), update the URL so it's shareable.
  // Only redirect when the URL still uses the local id (not yet the remoteId).
  useEffect(() => {
    if (
      currentConversation?.remoteId &&
      conversationId &&
      conversationId === currentConversation.id &&
      conversationId !== currentConversation.remoteId
    ) {
      navigate(`/chat/${currentConversation.remoteId}`, { replace: true });
    }
  }, [currentConversation?.remoteId, currentConversation?.id, conversationId, navigate]);

  // Track which conversation we've loaded to avoid re-loading during updates
  const loadedConversationIdRef = useRef<string | null>(null);
  // Track conversations created with an immediate message (to skip loading empty state)
  const pendingNewConversationRef = useRef<string | null>(null);
  // Debounce timer for saves
  const saveTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  // Skip the first save after loading a conversation (loading messages into the store
  // triggers the save effect, but nothing actually changed — saving would bump updatedAt
  // and cause the conversation to jump to the top of the sidebar)
  const skipNextSaveRef = useRef(false);
  // Ref for currentConversationId so the save effect doesn't re-trigger on conversation
  // switch (which would consume the skipNextSaveRef before the real message-load render)
  const currentConversationIdRef = useRef(currentConversation?.id);

  // Load conversation when it changes
  useEffect(() => {
    const newId = currentConversation?.id ?? null;

    if (newId !== loadedConversationIdRef.current) {
      loadedConversationIdRef.current = newId;

      // Skip loading if this is a conversation we just created with an immediate message
      // (the message is already in state, and the conversation is empty)
      if (newId === pendingNewConversationRef.current) {
        pendingNewConversationRef.current = null;
        skipNextSaveRef.current = false;
        return;
      }

      if (currentConversation) {
        skipNextSaveRef.current = true;
        setMessages(currentConversation.messages);
        if (currentConversation.models.length > 0) {
          setSelectedModels(currentConversation.models);
        }
        setDisabledModels([]);
        clearSelectedBestResponses();
      } else {
        skipNextSaveRef.current = false;
        clearMessages();
        clearSelectedBestResponses();
      }
    }
  }, [currentConversation?.id]); // eslint-disable-line react-hooks/exhaustive-deps

  // Save conversation when messages change (debounced).
  // currentConversationId is accessed via ref so that switching conversations doesn't
  // trigger this effect — otherwise it fires before the message-load render and
  // consumes skipNextSaveRef too early, causing a spurious save that bumps updatedAt.
  currentConversationIdRef.current = currentConversation?.id;
  useEffect(() => {
    const convId = currentConversationIdRef.current;
    if (isStreaming || !convId || messages.length === 0) return;
    if (loadedConversationIdRef.current !== convId) return;

    // After loading a conversation, the messages store changes which triggers this effect.
    // Skip that first save to avoid bumping updatedAt (which reorders the sidebar).
    if (skipNextSaveRef.current) {
      skipNextSaveRef.current = false;
      return;
    }

    if (saveTimeoutRef.current) {
      clearTimeout(saveTimeoutRef.current);
    }

    saveTimeoutRef.current = setTimeout(() => {
      updateConversation(currentConversationIdRef.current!, messages, selectedModels);
    }, 100);

    return () => {
      if (saveTimeoutRef.current) {
        clearTimeout(saveTimeoutRef.current);
      }
    };
  }, [messages, isStreaming, selectedModels, updateConversation]);

  /**
   * Fork a conversation, optionally up to a specific message.
   * Returns the new forked conversation.
   */
  const handleForkConversation = useCallback(
    (sourceId: string, options?: ForkConversationOptions) => {
      return forkConversation(sourceId, options);
    },
    [forkConversation]
  );

  return {
    currentConversation,
    /**
     * Create a new conversation and mark it as pending so the load effect
     * doesn't clear the messages that are about to be added.
     */
    createConversation: (models: string[], projectId?: string, projectName?: string) => {
      const newConv = createConversation(models, projectId, projectName);
      pendingNewConversationRef.current = newConv.id;
      return newConv;
    },
    /**
     * Fork the current or another conversation.
     * Returns the new forked conversation.
     */
    forkConversation: handleForkConversation,
  };
}
