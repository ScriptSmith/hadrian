import {
  GitFork,
  MessageSquare,
  MoreHorizontal,
  Pencil,
  Plus,
  Search,
  Sparkles,
  Trash2,
  X,
} from "lucide-react";
import { memo, useCallback, useState } from "react";

import { Button } from "@/components/Button/Button";
import {
  Dropdown,
  DropdownContent,
  DropdownItem,
  DropdownTrigger,
} from "@/components/Dropdown/Dropdown";
import { Input } from "@/components/Input/Input";
import { cn } from "@/utils/cn";

import type { Conversation } from "@/components/chat-types";

interface ConversationListProps {
  conversations: Conversation[];
  currentConversationId: string | null;
  onSelect: (id: string) => void;
  onNew: () => void;
  onDelete: (id: string) => void;
  onRename: (id: string, title: string) => void;
  onRegenerateTitle: (id: string) => void;
  onFork: (id: string) => void;
}

function groupConversations(conversations: Conversation[]) {
  const now = new Date();
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  const yesterday = new Date(today.getTime() - 24 * 60 * 60 * 1000);
  const weekAgo = new Date(today.getTime() - 7 * 24 * 60 * 60 * 1000);
  const monthAgo = new Date(today.getTime() - 30 * 24 * 60 * 60 * 1000);

  const groups: { label: string; conversations: Conversation[] }[] = [
    { label: "Today", conversations: [] },
    { label: "Yesterday", conversations: [] },
    { label: "Last 7 days", conversations: [] },
    { label: "Last 30 days", conversations: [] },
    { label: "Older", conversations: [] },
  ];

  for (const conv of conversations) {
    const date = conv.updatedAt;
    if (date >= today) {
      groups[0].conversations.push(conv);
    } else if (date >= yesterday) {
      groups[1].conversations.push(conv);
    } else if (date >= weekAgo) {
      groups[2].conversations.push(conv);
    } else if (date >= monthAgo) {
      groups[3].conversations.push(conv);
    } else {
      groups[4].conversations.push(conv);
    }
  }

  return groups.filter((g) => g.conversations.length > 0);
}

interface ConversationItemProps {
  conversation: Conversation;
  isSelected: boolean;
  isEditing: boolean;
  editTitle: string;
  onSelect: (id: string) => void;
  onStartEdit: (conv: Conversation) => void;
  onSaveEdit: () => void;
  onCancelEdit: () => void;
  onEditTitleChange: (title: string) => void;
  onDelete: (id: string) => void;
  onRegenerateTitle: (id: string) => void;
  onFork: (id: string) => void;
}

/**
 * Individual conversation item - memoized to prevent re-renders during search typing
 * and other list-level state changes.
 */
const ConversationItem = memo(
  function ConversationItem({
    conversation: conv,
    isSelected,
    isEditing,
    editTitle,
    onSelect,
    onStartEdit,
    onSaveEdit,
    onCancelEdit,
    onEditTitleChange,
    onDelete,
    onRegenerateTitle,
    onFork,
  }: ConversationItemProps) {
    if (isEditing) {
      return (
        <li>
          <div className="flex items-center gap-1 rounded-md bg-accent p-1">
            <Input
              value={editTitle}
              onChange={(e) => onEditTitleChange(e.target.value)}
              className="h-7 text-sm"
              autoFocus
              onKeyDown={(e) => {
                if (e.key === "Enter") onSaveEdit();
                if (e.key === "Escape") onCancelEdit();
              }}
            />
            <Button
              variant="ghost"
              size="icon"
              className="h-7 w-7 shrink-0"
              onClick={onSaveEdit}
              aria-label="Save"
            >
              <Pencil className="h-3 w-3" />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              className="h-7 w-7 shrink-0"
              onClick={onCancelEdit}
              aria-label="Cancel"
            >
              <X className="h-3 w-3" />
            </Button>
          </div>
        </li>
      );
    }

    return (
      <li>
        <div
          className={cn(
            "group flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-sm transition-colors",
            "hover:bg-accent hover:text-accent-foreground",
            isSelected && "bg-accent text-accent-foreground"
          )}
        >
          <button
            type="button"
            className="flex min-w-0 flex-1 items-center gap-2 text-left"
            onClick={() => onSelect(conv.id)}
          >
            <MessageSquare className="h-4 w-4 shrink-0 text-muted-foreground" />
            <div className="min-w-0 flex-1">
              <div className="truncate">{conv.title}</div>
              <div className="text-xs text-muted-foreground">
                {conv.messages.length} messages
                {conv.models.length > 0 && (
                  <span>
                    {" "}
                    &middot; {conv.models.length} model
                    {conv.models.length !== 1 ? "s" : ""}
                  </span>
                )}
              </div>
            </div>
          </button>
          <Dropdown>
            <DropdownTrigger asChild showChevron={false}>
              <button
                type="button"
                aria-label="Conversation actions"
                className={cn(
                  "inline-flex items-center justify-center h-6 w-6 shrink-0 p-0 opacity-0 transition-opacity",
                  "group-hover:opacity-100",
                  isSelected && "opacity-100"
                )}
              >
                <MoreHorizontal className="h-4 w-4" />
              </button>
            </DropdownTrigger>
            <DropdownContent align="end">
              <DropdownItem onClick={() => onStartEdit(conv)}>
                <Pencil className="mr-2 h-4 w-4" />
                Rename
              </DropdownItem>
              <DropdownItem onClick={() => onRegenerateTitle(conv.id)}>
                <Sparkles className="mr-2 h-4 w-4" />
                Regenerate title
              </DropdownItem>
              <DropdownItem onClick={() => onFork(conv.id)}>
                <GitFork className="mr-2 h-4 w-4" />
                Fork
              </DropdownItem>
              <DropdownItem className="text-destructive" onClick={() => onDelete(conv.id)}>
                <Trash2 className="mr-2 h-4 w-4" />
                Delete
              </DropdownItem>
            </DropdownContent>
          </Dropdown>
        </div>
      </li>
    );
  },
  (prevProps, nextProps) => {
    // Custom comparator for efficient re-render detection
    return (
      prevProps.conversation.id === nextProps.conversation.id &&
      prevProps.conversation.title === nextProps.conversation.title &&
      prevProps.conversation.messages.length === nextProps.conversation.messages.length &&
      prevProps.conversation.models.length === nextProps.conversation.models.length &&
      prevProps.isSelected === nextProps.isSelected &&
      prevProps.isEditing === nextProps.isEditing &&
      // Only compare editTitle if editing this item
      (!prevProps.isEditing || prevProps.editTitle === nextProps.editTitle)
    );
  }
);

export function ConversationList({
  conversations,
  currentConversationId,
  onSelect,
  onNew,
  onDelete,
  onRename,
  onRegenerateTitle,
  onFork,
}: ConversationListProps) {
  const [searchQuery, setSearchQuery] = useState("");
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editTitle, setEditTitle] = useState("");

  const filteredConversations = searchQuery
    ? conversations.filter(
        (c) =>
          c.title.toLowerCase().includes(searchQuery.toLowerCase()) ||
          c.messages.some((m) => m.content.toLowerCase().includes(searchQuery.toLowerCase()))
      )
    : conversations;

  const groups = groupConversations(filteredConversations);

  // Memoized handlers to prevent ConversationItem re-renders
  const handleStartEdit = useCallback((conv: Conversation) => {
    setEditingId(conv.id);
    setEditTitle(conv.title);
  }, []);

  const handleSaveEdit = useCallback(() => {
    setEditingId((currentId) => {
      if (currentId) {
        setEditTitle((currentTitle) => {
          if (currentTitle.trim()) {
            onRename(currentId, currentTitle.trim());
          }
          return "";
        });
      }
      return null;
    });
  }, [onRename]);

  const handleCancelEdit = useCallback(() => {
    setEditingId(null);
    setEditTitle("");
  }, []);

  const handleEditTitleChange = useCallback((title: string) => {
    setEditTitle(title);
  }, []);

  return (
    <div className="flex h-full flex-col">
      {/* Header */}
      <div className="flex items-center justify-between border-b px-3 py-2">
        <span className="font-semibold text-sm">Conversations</span>
        <Button variant="ghost" size="icon" onClick={onNew} title="New conversation">
          <Plus className="h-4 w-4" />
        </Button>
      </div>

      {/* Search */}
      <div className="px-2 py-2">
        <div className="relative">
          <Search className="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
          <Input
            placeholder="Search conversations..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="pl-8 text-sm"
            aria-label="Search conversations"
          />
          {searchQuery && (
            <Button
              variant="ghost"
              size="icon"
              className="absolute right-1 top-1 h-6 w-6"
              onClick={() => setSearchQuery("")}
              aria-label="Clear search"
            >
              <X className="h-3 w-3" />
            </Button>
          )}
        </div>
      </div>

      {/* Conversation List */}
      <div className="flex-1 overflow-y-auto px-2 pb-2">
        {conversations.length === 0 ? (
          <div className="py-8 text-center text-sm text-muted-foreground">
            <MessageSquare className="mx-auto mb-2 h-8 w-8 opacity-50" />
            <p>No conversations yet</p>
            <p className="mt-1 text-xs">Start a new chat to begin</p>
          </div>
        ) : filteredConversations.length === 0 ? (
          <div className="py-8 text-center text-sm text-muted-foreground">
            <Search className="mx-auto mb-2 h-8 w-8 opacity-50" />
            <p>No conversations found</p>
          </div>
        ) : (
          groups.map((group) => (
            <div key={group.label} className="mb-4">
              <div className="mb-1 px-2 text-xs font-medium text-muted-foreground">
                {group.label}
              </div>
              <ul className="space-y-0.5">
                {group.conversations.map((conv) => (
                  <ConversationItem
                    key={conv.id}
                    conversation={conv}
                    isSelected={currentConversationId === conv.id}
                    isEditing={editingId === conv.id}
                    editTitle={editTitle}
                    onSelect={onSelect}
                    onStartEdit={handleStartEdit}
                    onSaveEdit={handleSaveEdit}
                    onCancelEdit={handleCancelEdit}
                    onEditTitleChange={handleEditTitleChange}
                    onDelete={onDelete}
                    onRegenerateTitle={onRegenerateTitle}
                    onFork={onFork}
                  />
                ))}
              </ul>
            </div>
          ))
        )}
      </div>
    </div>
  );
}
