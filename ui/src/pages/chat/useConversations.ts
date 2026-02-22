import { useEffect, useState, useCallback } from "react";

import {
  useConversationsContext,
  type ForkConversationOptions,
} from "@/components/ConversationsProvider/ConversationsProvider";
import type { ChatMessage, Conversation } from "./types";

interface UseConversationsReturn {
  conversations: Conversation[];
  currentConversation: Conversation | null;
  createConversation: (models: string[]) => Conversation;
  selectConversation: (id: string) => void;
  updateConversation: (id: string, messages: ChatMessage[], models?: string[]) => void;
  deleteConversation: (id: string) => void;
  renameConversation: (id: string, title: string) => void;
  regenerateTitle: (id: string) => Promise<void>;
  togglePin: (id: string) => void;
  reorderPinned: (orderedIds: string[]) => void;
  moveToProject: (id: string, projectId: string | null, projectName?: string) => Promise<void>;
  forkConversation: (id: string, options?: ForkConversationOptions) => Conversation;
  clearCurrentConversation: () => void;
}

export function useConversations(initialConversationId?: string): UseConversationsReturn {
  const {
    conversations,
    createConversation: contextCreate,
    updateConversation,
    deleteConversation: contextDelete,
    renameConversation,
    regenerateTitle,
    togglePin,
    reorderPinned,
    moveToProject,
    forkConversation: contextFork,
  } = useConversationsContext();

  const [currentId, setCurrentId] = useState<string | null>(initialConversationId ?? null);

  // Load conversation when URL changes
  useEffect(() => {
    if (initialConversationId && initialConversationId !== currentId) {
      setCurrentId(initialConversationId);
    }
  }, [initialConversationId, currentId]);

  // Clear current ID if the conversation was deleted
  useEffect(() => {
    if (currentId && !conversations.some((c) => c.id === currentId)) {
      setCurrentId(null);
    }
  }, [conversations, currentId]);

  const currentConversation = conversations.find((c) => c.id === currentId) ?? null;

  const createConversation = useCallback(
    (models: string[]): Conversation => {
      const newConversation = contextCreate(models);
      setCurrentId(newConversation.id);
      return newConversation;
    },
    [contextCreate]
  );

  const selectConversation = useCallback((id: string) => {
    setCurrentId(id);
  }, []);

  const deleteConversation = useCallback(
    (id: string) => {
      contextDelete(id);
      if (currentId === id) {
        setCurrentId(null);
      }
    },
    [contextDelete, currentId]
  );

  const clearCurrentConversation = useCallback(() => {
    setCurrentId(null);
  }, []);

  const forkConversation = useCallback(
    (id: string, options?: ForkConversationOptions): Conversation => {
      const forked = contextFork(id, options);
      setCurrentId(forked.id);
      return forked;
    },
    [contextFork]
  );

  return {
    conversations,
    currentConversation,
    createConversation,
    selectConversation,
    updateConversation,
    deleteConversation,
    renameConversation,
    regenerateTitle,
    togglePin,
    reorderPinned,
    moveToProject,
    forkConversation,
    clearCurrentConversation,
  };
}
