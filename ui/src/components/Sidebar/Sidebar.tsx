import { NavLink, useLocation, useNavigate } from "react-router-dom";
import { useState, useCallback, useMemo, type MouseEvent } from "react";
import {
  DndContext,
  closestCenter,
  KeyboardSensor,
  PointerSensor,
  useSensor,
  useSensors,
  type DragEndEvent,
} from "@dnd-kit/core";
import {
  arrayMove,
  SortableContext,
  sortableKeyboardCoordinates,
  useSortable,
  verticalListSortingStrategy,
} from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import {
  MessageSquare,
  FolderOpen,
  X,
  ChevronLeft,
  ChevronDown,
  ChevronRight,
  Plus,
  Search,
  MoreHorizontal,
  Pencil,
  Trash2,
  Pin,
  PinOff,
  Sparkles,
  GripVertical,
  ArrowRightLeft,
  GitFork,
} from "lucide-react";
import { cn } from "@/utils/cn";
import { Button } from "@/components/Button/Button";
import { Input } from "@/components/Input/Input";
import {
  Dropdown,
  DropdownContent,
  DropdownItem,
  DropdownTrigger,
} from "@/components/Dropdown/Dropdown";
import {
  ContextMenu,
  ContextMenuItem,
  ContextMenuSeparator,
  type ContextMenuPosition,
} from "@/components/ContextMenu";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/Tooltip/Tooltip";
import { useToast } from "@/components/Toast/Toast";
import { useConfig } from "@/config/ConfigProvider";
import { useConversations } from "@/pages/chat/useConversations";
import { MoveToProjectModal } from "@/components/MoveToProjectModal";
import {
  ForkConversationModal,
  type ForkConversationResult,
} from "@/components/ForkConversationModal/ForkConversationModal";
import type { Conversation } from "@/pages/chat/types";

interface SidebarProps {
  open: boolean;
  onClose: () => void;
  collapsed?: boolean;
  onCollapsedChange?: (collapsed: boolean) => void;
  /** Sidebar width in pixels (only used when not collapsed) */
  width?: number;
  /** Called when sidebar is being resized */
  onWidthChange?: (width: number) => void;
  /** Whether currently resizing */
  isResizing?: boolean;
  /** Props for the resize handle */
  resizeHandleProps?: {
    onMouseDown: (e: React.MouseEvent) => void;
    onTouchStart: (e: React.TouchEvent) => void;
    onDoubleClick: () => void;
    style: React.CSSProperties;
  };
}

/** Format a date as a relative time string (e.g., "2 hours ago", "Yesterday") */
function formatRelativeTime(date: Date): string {
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffMins = Math.floor(diffMs / (1000 * 60));
  const diffHours = Math.floor(diffMs / (1000 * 60 * 60));
  const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24));

  if (diffMins < 1) return "Just now";
  if (diffMins < 60) return `${diffMins}m ago`;
  if (diffHours < 24) return `${diffHours}h ago`;
  if (diffDays === 1) return "Yesterday";
  if (diffDays < 7) return `${diffDays}d ago`;
  return date.toLocaleDateString();
}

/** Strip markdown formatting from text */
function stripMarkdown(text: string): string {
  return (
    text
      // Remove code blocks (``` ... ```)
      .replace(/```[\s\S]*?```/g, "[code]")
      // Remove inline code (`...`)
      .replace(/`([^`]+)`/g, "$1")
      // Remove images ![alt](url)
      .replace(/!\[([^\]]*)\]\([^)]+\)/g, "$1")
      // Remove links [text](url) -> text
      .replace(/\[([^\]]+)\]\([^)]+\)/g, "$1")
      // Remove bold **text** or __text__
      .replace(/\*\*([^*]+)\*\*/g, "$1")
      .replace(/__([^_]+)__/g, "$1")
      // Remove italic *text* or _text_
      .replace(/\*([^*]+)\*/g, "$1")
      .replace(/_([^_]+)_/g, "$1")
      // Remove strikethrough ~~text~~
      .replace(/~~([^~]+)~~/g, "$1")
      // Remove headers (# ## ### etc.)
      .replace(/^#{1,6}\s+/gm, "")
      // Remove blockquotes
      .replace(/^>\s+/gm, "")
      // Remove horizontal rules
      .replace(/^[-*_]{3,}\s*$/gm, "")
      // Remove list markers
      .replace(/^[\s]*[-*+]\s+/gm, "")
      .replace(/^[\s]*\d+\.\s+/gm, "")
      // Collapse multiple spaces/newlines
      .replace(/\s+/g, " ")
      .trim()
  );
}

/** Get a preview of the last message in a conversation */
function getLastMessagePreview(conv: Conversation): string | null {
  if (conv.messages.length === 0) return null;
  const lastMessage = conv.messages[conv.messages.length - 1];
  const content = lastMessage.content.trim();
  if (!content) return null;
  // Strip markdown and truncate
  const plainText = stripMarkdown(content);
  if (!plainText) return null;
  if (plainText.length > 100) {
    return plainText.slice(0, 97) + "...";
  }
  return plainText;
}

interface ConversationPreviewProps {
  conv: Conversation;
  children: React.ReactNode;
}

/** Wraps a conversation item with a hover tooltip showing preview and timestamp */
function ConversationPreview({ conv, children }: ConversationPreviewProps) {
  const preview = useMemo(() => getLastMessagePreview(conv), [conv]);
  const timeAgo = useMemo(() => formatRelativeTime(conv.updatedAt), [conv.updatedAt]);
  const lastRole = conv.messages.length > 0 ? conv.messages[conv.messages.length - 1].role : null;

  return (
    <Tooltip delayDuration={400}>
      <TooltipTrigger asChild>{children}</TooltipTrigger>
      <TooltipContent side="right" sideOffset={8} className="max-w-xs">
        <div className="space-y-1">
          {preview && (
            <p className="text-xs text-popover-foreground/90 leading-relaxed">
              <span className="text-muted-foreground">
                {lastRole === "user" ? "You: " : lastRole === "assistant" ? "AI: " : ""}
              </span>
              {preview}
            </p>
          )}
          <p className="text-[11px] text-muted-foreground">{timeAgo}</p>
        </div>
      </TooltipContent>
    </Tooltip>
  );
}

interface SortablePinnedItemProps {
  conv: Conversation;
  isActive: boolean;
  isEditing: boolean;
  editTitle: string;
  onEditTitleChange: (value: string) => void;
  onSaveEdit: () => void;
  onCancelEdit: () => void;
  onSelect: () => void;
  onContextMenu: (e: MouseEvent) => void;
  onTogglePin: () => void;
  onStartEdit: () => void;
  onRegenerateTitle: () => void;
  onOpenMoveModal: () => void;
  onFork: () => void;
  onDelete: () => void;
}

/** Sortable pinned conversation item with drag handle */
function SortablePinnedItem({
  conv,
  isActive,
  isEditing,
  editTitle,
  onEditTitleChange,
  onSaveEdit,
  onCancelEdit,
  onSelect,
  onContextMenu,
  onTogglePin,
  onStartEdit,
  onRegenerateTitle,
  onOpenMoveModal,
  onFork,
  onDelete,
}: SortablePinnedItemProps) {
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({
    id: conv.id,
  });

  const style = {
    transform: CSS.Transform.toString(transform),
    transition,
  };

  if (isEditing) {
    return (
      <li ref={setNodeRef} style={style}>
        <div className="flex items-center gap-1 rounded-lg bg-accent p-1">
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
    <li ref={setNodeRef} style={style} className={cn(isDragging && "opacity-50 z-50")}>
      <ConversationPreview conv={conv}>
        <div
          className={cn(
            "group flex w-full items-center gap-1 rounded-lg px-1 py-1.5 text-sm",
            "hover:bg-accent/50",
            isActive ? "bg-accent text-accent-foreground font-medium" : "text-foreground/80"
          )}
          onContextMenu={onContextMenu}
        >
          {/* Drag handle - stop propagation to prevent navigation when dragging */}
          {/* eslint-disable-next-line jsx-a11y/click-events-have-key-events, jsx-a11y/no-static-element-interactions -- DnD kit provides keyboard handlers via {...attributes} and {...listeners} */}
          <div
            className={cn(
              "flex h-6 w-5 shrink-0 cursor-grab items-center justify-center rounded text-muted-foreground",
              "opacity-0 group-hover:opacity-100 hover:bg-muted active:cursor-grabbing",
              isDragging && "opacity-100"
            )}
            onClick={(e) => e.stopPropagation()}
            onPointerDown={(e) => e.stopPropagation()}
            {...attributes}
            {...listeners}
          >
            <GripVertical className="h-3.5 w-3.5" />
          </div>

          {/* Title */}
          <button type="button" className="min-w-0 flex-1 truncate text-left" onClick={onSelect}>
            {conv.title}
          </button>

          {/* Actions dropdown */}
          <Dropdown>
            <DropdownTrigger asChild showChevron={false}>
              <button
                type="button"
                aria-label="Conversation actions"
                className={cn(
                  "inline-flex items-center justify-center h-6 w-6 shrink-0 rounded-md p-0 opacity-0 transition-opacity hover:bg-muted",
                  "group-hover:opacity-100",
                  isActive && "opacity-100"
                )}
              >
                <MoreHorizontal className="h-4 w-4" />
              </button>
            </DropdownTrigger>
            <DropdownContent align="end">
              <DropdownItem onClick={onTogglePin}>
                <PinOff className="mr-2 h-4 w-4" />
                Unpin
              </DropdownItem>
              <DropdownItem onClick={onStartEdit}>
                <Pencil className="mr-2 h-4 w-4" />
                Rename
              </DropdownItem>
              <DropdownItem onClick={onRegenerateTitle}>
                <Sparkles className="mr-2 h-4 w-4" />
                Regenerate title
              </DropdownItem>
              <DropdownItem onClick={onFork}>
                <GitFork className="mr-2 h-4 w-4" />
                Fork
              </DropdownItem>
              <DropdownItem onClick={onOpenMoveModal}>
                <ArrowRightLeft className="mr-2 h-4 w-4" />
                Move to Project...
              </DropdownItem>
              <DropdownItem className="text-destructive" onClick={onDelete}>
                <Trash2 className="mr-2 h-4 w-4" />
                Delete
              </DropdownItem>
            </DropdownContent>
          </Dropdown>
        </div>
      </ConversationPreview>
    </li>
  );
}

function groupByTime(conversations: Conversation[]): ConversationGroup[] {
  const now = new Date();
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  const yesterday = new Date(today.getTime() - 24 * 60 * 60 * 1000);
  const weekAgo = new Date(today.getTime() - 7 * 24 * 60 * 60 * 1000);

  const groups: ConversationGroup[] = [
    { label: "Today", conversations: [] },
    { label: "Yesterday", conversations: [] },
    { label: "Previous 7 days", conversations: [] },
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
    } else {
      groups[3].conversations.push(conv);
    }
  }

  return groups.filter((g) => g.conversations.length > 0);
}

interface ConversationGroup {
  label: string;
  conversations: Conversation[];
  isProject?: boolean;
}

interface SectionedConversations {
  pinnedConversations: Conversation[];
  myConversations: ConversationGroup[];
  sharedByProject: Map<string, { projectName: string; groups: ConversationGroup[] }>;
}

function groupConversations(conversations: Conversation[]): SectionedConversations {
  // Separate pinned from unpinned
  const pinned = conversations
    .filter((c) => c.pinOrder != null)
    .sort((a, b) => (a.pinOrder ?? 0) - (b.pinOrder ?? 0));
  const unpinned = conversations.filter((c) => c.pinOrder == null);

  const myConversations = unpinned.filter((c) => !c.projectId);
  const sharedConversations = unpinned.filter((c) => c.projectId);

  // Group shared by project
  const projectMap = new Map<string, Conversation[]>();
  for (const conv of sharedConversations) {
    const projectId = conv.projectId!;
    if (!projectMap.has(projectId)) {
      projectMap.set(projectId, []);
    }
    projectMap.get(projectId)!.push(conv);
  }

  const sharedByProject = new Map<string, { projectName: string; groups: ConversationGroup[] }>();
  for (const [projectId, convs] of projectMap) {
    const projectName = convs[0]?.projectName || projectId;
    sharedByProject.set(projectId, {
      projectName,
      groups: groupByTime(convs),
    });
  }

  return {
    pinnedConversations: pinned,
    myConversations: groupByTime(myConversations),
    sharedByProject,
  };
}

export function Sidebar({
  open,
  onClose,
  collapsed = false,
  onCollapsedChange,
  width = 256,
  isResizing = false,
  resizeHandleProps,
}: SidebarProps) {
  const location = useLocation();
  const navigate = useNavigate();
  const { config } = useConfig();

  const [pinnedExpanded, setPinnedExpanded] = useState(true);
  const [myExpanded, setMyExpanded] = useState(true);
  const [sharedExpanded, setSharedExpanded] = useState(true);
  const [expandedProjects, setExpandedProjects] = useState<Set<string>>(new Set());
  const [searchQuery, setSearchQuery] = useState("");
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editTitle, setEditTitle] = useState("");

  // Context menu state
  const [contextMenuConversation, setContextMenuConversation] = useState<Conversation | null>(null);
  const [contextMenuPosition, setContextMenuPosition] = useState<ContextMenuPosition | null>(null);

  // Move to project modal state
  const [moveModalConversation, setMoveModalConversation] = useState<Conversation | null>(null);
  const [forkModalConversation, setForkModalConversation] = useState<Conversation | null>(null);

  const { toast } = useToast();

  const toggleProject = (projectId: string) => {
    setExpandedProjects((prev) => {
      const next = new Set(prev);
      if (next.has(projectId)) {
        next.delete(projectId);
      } else {
        next.add(projectId);
      }
      return next;
    });
  };

  const {
    conversations,
    createConversation,
    deleteConversation,
    renameConversation,
    regenerateTitle,
    togglePin,
    reorderPinned,
    moveToProject,
    forkConversation,
  } = useConversations();

  // Get active conversation ID from URL instead of hook state
  // This ensures the sidebar always reflects the current URL
  const activeConversationId = location.pathname.startsWith("/chat/")
    ? location.pathname.split("/")[2]
    : null;

  const filteredConversations = searchQuery
    ? conversations.filter((c) => c.title.toLowerCase().includes(searchQuery.toLowerCase()))
    : conversations;

  const { pinnedConversations, myConversations, sharedByProject } =
    groupConversations(filteredConversations);
  const hasPinned = pinnedConversations.length > 0;

  // Drag and drop sensors for pinned conversations
  const sensors = useSensors(
    useSensor(PointerSensor, {
      activationConstraint: {
        distance: 8, // Require 8px of movement before starting drag
      },
    }),
    useSensor(KeyboardSensor, {
      coordinateGetter: sortableKeyboardCoordinates,
    })
  );

  // Handle drag end for pinned conversations reordering
  const handleDragEnd = (event: DragEndEvent) => {
    const { active, over } = event;
    if (!over || active.id === over.id) return;

    const pinnedIds = pinnedConversations.map((c) => c.id);
    const oldIndex = pinnedIds.indexOf(active.id as string);
    const newIndex = pinnedIds.indexOf(over.id as string);

    if (oldIndex !== -1 && newIndex !== -1) {
      const newOrder = arrayMove(pinnedIds, oldIndex, newIndex);
      reorderPinned(newOrder);
    }
  };
  const hasShared = sharedByProject.size > 0;

  const handleNewChat = () => {
    const newConv = createConversation([]);
    navigate(`/chat/${newConv.id}`);
    if (window.innerWidth < 1024) onClose();
  };

  const handleSelectConversation = (id: string) => {
    navigate(`/chat/${id}`);
    if (window.innerWidth < 1024) onClose();
  };

  const handleDeleteConversation = (conv: Conversation) => {
    deleteConversation(conv.id);
    if (activeConversationId === conv.id) {
      navigate("/chat");
    }
  };

  const handleStartEdit = (conv: Conversation) => {
    setEditingId(conv.id);
    setEditTitle(conv.title);
  };

  const handleSaveEdit = () => {
    if (editingId && editTitle.trim()) {
      renameConversation(editingId, editTitle.trim());
    }
    setEditingId(null);
    setEditTitle("");
  };

  const handleCancelEdit = () => {
    setEditingId(null);
    setEditTitle("");
  };

  const handleContextMenu = useCallback((e: MouseEvent, conv: Conversation) => {
    e.preventDefault();
    e.stopPropagation();
    setContextMenuConversation(conv);
    setContextMenuPosition({ x: e.clientX, y: e.clientY });
  }, []);

  const handleContextMenuClose = useCallback(() => {
    setContextMenuConversation(null);
    setContextMenuPosition(null);
  }, []);

  const handleOpenMoveModal = useCallback(
    (conv: Conversation) => {
      setMoveModalConversation(conv);
      handleContextMenuClose();
    },
    [handleContextMenuClose]
  );

  const handleOpenForkModal = useCallback(
    (conv: Conversation) => {
      setForkModalConversation(conv);
      handleContextMenuClose();
    },
    [handleContextMenuClose]
  );

  const handleForkConversation = useCallback(
    (result: ForkConversationResult) => {
      if (!forkModalConversation) return;
      const forked = forkConversation(forkModalConversation.id, {
        newTitle: result.title,
        models: result.models,
        projectId: result.projectId,
        projectName: result.projectName,
      });
      navigate(`/chat/${forked.id}`);
      setForkModalConversation(null);
      if (window.innerWidth < 1024) onClose();
    },
    [forkModalConversation, forkConversation, navigate, onClose]
  );

  const handleMoveToProject = useCallback(
    async (projectId: string | null, projectName?: string) => {
      if (!moveModalConversation) return;
      await moveToProject(moveModalConversation.id, projectId, projectName);
      toast({
        title: projectId ? "Moved to project" : "Moved to My Conversations",
        type: "success",
      });
    },
    [moveModalConversation, moveToProject, toast]
  );

  const isChatRoute = location.pathname.startsWith("/chat") || location.pathname === "/";

  return (
    <>
      {/* Backdrop for mobile */}
      {open && (
        <div
          className="fixed inset-0 z-40 bg-black/60 backdrop-blur-sm lg:hidden animate-in fade-in-0"
          onClick={onClose}
          aria-hidden="true"
        />
      )}

      {/* Sidebar */}
      <aside
        aria-label="Chat navigation"
        className={cn(
          "fixed inset-y-0 left-0 z-50 flex flex-col bg-card",
          "lg:relative lg:z-auto",
          open ? "translate-x-0" : "-translate-x-full lg:translate-x-0",
          collapsed && "w-16",
          !isResizing && "transition-all duration-300"
        )}
        style={collapsed ? undefined : { width: `${width}px` }}
      >
        {/* Resize handle - only shown on desktop when not collapsed */}
        {!collapsed && resizeHandleProps && (
          <div
            {...resizeHandleProps}
            className={cn(
              "absolute inset-y-0 right-0 z-10 hidden w-1 lg:block",
              "hover:bg-primary/20 active:bg-primary/30",
              "transition-colors duration-150",
              isResizing && "bg-primary/30"
            )}
            title="Drag to resize, double-click to reset"
          />
        )}

        {/* Header with New Chat + close button */}
        <div className="flex h-14 shrink-0 items-center gap-2 px-3">
          {!collapsed ? (
            <>
              <Button
                variant="primary"
                className="flex-1 justify-center gap-2"
                onClick={handleNewChat}
              >
                <Plus className="h-4 w-4" />
                New Chat
              </Button>
              <Button
                variant="ghost"
                size="icon"
                onClick={onClose}
                aria-label="Close sidebar"
                className="h-8 w-8 lg:hidden text-muted-foreground hover:text-foreground"
              >
                <X className="h-4 w-4" />
              </Button>
            </>
          ) : (
            <div className="flex w-full flex-col items-center gap-2">
              <Tooltip>
                <TooltipTrigger asChild>
                  <Button
                    variant="primary"
                    size="icon"
                    className="h-10 w-10"
                    onClick={handleNewChat}
                    aria-label="New Chat"
                  >
                    <Plus className="h-5 w-5" />
                  </Button>
                </TooltipTrigger>
                <TooltipContent side="right">New Chat</TooltipContent>
              </Tooltip>
            </div>
          )}
        </div>

        {!collapsed && (
          <>
            {/* Search */}
            <div className="shrink-0 px-3 pb-3">
              <div className="relative">
                <Search
                  className="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground"
                  aria-hidden="true"
                />
                <Input
                  placeholder="Search..."
                  aria-label="Search conversations"
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  className="h-9 pl-8 text-sm bg-secondary/50 border-transparent hover:bg-secondary focus-visible:bg-background"
                />
                {searchQuery && (
                  <Button
                    variant="ghost"
                    size="icon"
                    className="absolute right-1 top-1 h-7 w-7"
                    onClick={() => setSearchQuery("")}
                    aria-label="Clear search"
                  >
                    <X className="h-3 w-3" />
                  </Button>
                )}
              </div>
            </div>
          </>
        )}

        {collapsed ? (
          /* Collapsed state - New Chat button + expand */
          <nav className="flex-1 p-2">
            <ul className="space-y-1">
              <li>
                <NavLink
                  to="/chat"
                  className={({ isActive }) =>
                    cn(
                      "flex h-10 w-10 items-center justify-center rounded-lg transition-colors mx-auto",
                      "hover:bg-accent hover:text-accent-foreground",
                      isActive ? "bg-accent text-accent-foreground" : "text-muted-foreground"
                    )
                  }
                  aria-label="Chat"
                  title="Chat"
                >
                  <MessageSquare className="h-5 w-5" />
                </NavLink>
              </li>
              {onCollapsedChange && (
                <li className="pt-2 border-t mt-2">
                  <button
                    onClick={() => onCollapsedChange(false)}
                    className="flex h-10 w-10 items-center justify-center rounded-lg transition-colors mx-auto text-muted-foreground hover:bg-accent hover:text-accent-foreground"
                    aria-label="Expand sidebar"
                    title="Expand sidebar"
                  >
                    <ChevronRight className="h-5 w-5" />
                  </button>
                </li>
              )}
            </ul>
          </nav>
        ) : (
          /* Expanded state - flex column layout */
          <div className="flex flex-1 flex-col overflow-hidden">
            {/* Conversations Section - scrollable */}
            <div className="flex-1 overflow-y-auto scrollbar-thin px-2">
              {/* Conversations */}
              {conversations.length === 0 && !searchQuery ? (
                <div className="py-8 text-center">
                  <div className="mx-auto mb-3 flex h-12 w-12 items-center justify-center rounded-xl bg-muted">
                    <MessageSquare className="h-6 w-6 text-muted-foreground" />
                  </div>
                  <p className="text-sm font-medium text-foreground">No conversations</p>
                  <p className="mt-1 text-xs text-muted-foreground">Start a new chat to begin</p>
                </div>
              ) : filteredConversations.length === 0 ? (
                <div className="py-8 text-center">
                  <div className="mx-auto mb-3 flex h-12 w-12 items-center justify-center rounded-xl bg-muted">
                    <Search className="h-6 w-6 text-muted-foreground" />
                  </div>
                  <p className="text-sm font-medium text-foreground">No results</p>
                  <p className="mt-1 text-xs text-muted-foreground">Try a different search</p>
                </div>
              ) : (
                <>
                  {/* Pinned Section */}
                  {hasPinned && (
                    <div className="mb-2">
                      <button
                        className={cn(
                          "flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-xs font-medium uppercase tracking-wider transition-colors",
                          "hover:bg-accent/50",
                          "text-muted-foreground"
                        )}
                        onClick={() => setPinnedExpanded(!pinnedExpanded)}
                        aria-expanded={pinnedExpanded}
                        aria-controls="pinned-conversations-list"
                      >
                        <Pin className="h-3 w-3" aria-hidden="true" />
                        <span className="flex-1 text-left">Pinned</span>
                        {pinnedExpanded ? (
                          <ChevronDown className="h-3.5 w-3.5" aria-hidden="true" />
                        ) : (
                          <ChevronRight className="h-3.5 w-3.5" aria-hidden="true" />
                        )}
                      </button>
                      {pinnedExpanded && (
                        <DndContext
                          sensors={sensors}
                          collisionDetection={closestCenter}
                          onDragEnd={handleDragEnd}
                        >
                          <SortableContext
                            items={pinnedConversations.map((c) => c.id)}
                            strategy={verticalListSortingStrategy}
                          >
                            <ul id="pinned-conversations-list" className="mt-1 space-y-0.5">
                              {pinnedConversations.map((conv) => (
                                <SortablePinnedItem
                                  key={conv.id}
                                  conv={conv}
                                  isActive={activeConversationId === conv.id && isChatRoute}
                                  isEditing={editingId === conv.id}
                                  editTitle={editTitle}
                                  onEditTitleChange={setEditTitle}
                                  onSaveEdit={handleSaveEdit}
                                  onCancelEdit={handleCancelEdit}
                                  onSelect={() => handleSelectConversation(conv.id)}
                                  onContextMenu={(e) => handleContextMenu(e, conv)}
                                  onTogglePin={() => togglePin(conv.id)}
                                  onStartEdit={() => handleStartEdit(conv)}
                                  onRegenerateTitle={() => regenerateTitle(conv.id)}
                                  onOpenMoveModal={() => handleOpenMoveModal(conv)}
                                  onFork={() => handleOpenForkModal(conv)}
                                  onDelete={() => handleDeleteConversation(conv)}
                                />
                              ))}
                            </ul>
                          </SortableContext>
                        </DndContext>
                      )}
                    </div>
                  )}

                  {/* My Conversations Section */}
                  {myConversations.length > 0 && (
                    <div>
                      <button
                        className={cn(
                          "flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-xs font-medium uppercase tracking-wider transition-colors",
                          "hover:bg-accent/50",
                          "text-muted-foreground"
                        )}
                        onClick={() => setMyExpanded(!myExpanded)}
                        aria-expanded={myExpanded}
                        aria-controls="my-conversations-list"
                      >
                        <span className="flex-1 text-left">Conversations</span>
                        {myExpanded ? (
                          <ChevronDown className="h-3.5 w-3.5" aria-hidden="true" />
                        ) : (
                          <ChevronRight className="h-3.5 w-3.5" aria-hidden="true" />
                        )}
                      </button>
                      {myExpanded && (
                        <div id="my-conversations-list" className="mt-1 space-y-3">
                          {myConversations.map((group) => (
                            <div key={group.label}>
                              <div className="mb-1 px-2 text-[11px] font-medium text-muted-foreground">
                                {group.label}
                              </div>
                              <ul className="space-y-0.5">
                                {group.conversations.map((conv) => (
                                  <li key={conv.id}>
                                    {editingId === conv.id ? (
                                      <div className="flex items-center gap-1 rounded-lg bg-accent p-1">
                                        <Input
                                          value={editTitle}
                                          onChange={(e) => setEditTitle(e.target.value)}
                                          className="h-7 text-sm"
                                          autoFocus
                                          onKeyDown={(e) => {
                                            if (e.key === "Enter") handleSaveEdit();
                                            if (e.key === "Escape") handleCancelEdit();
                                          }}
                                        />
                                        <Button
                                          variant="ghost"
                                          size="icon"
                                          className="h-7 w-7 shrink-0"
                                          onClick={handleSaveEdit}
                                          aria-label="Save"
                                        >
                                          <Pencil className="h-3 w-3" />
                                        </Button>
                                        <Button
                                          variant="ghost"
                                          size="icon"
                                          className="h-7 w-7 shrink-0"
                                          onClick={handleCancelEdit}
                                          aria-label="Cancel"
                                        >
                                          <X className="h-3 w-3" />
                                        </Button>
                                      </div>
                                    ) : (
                                      <ConversationPreview conv={conv}>
                                        <div
                                          className={cn(
                                            "group flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-sm",
                                            "hover:bg-accent/50",
                                            activeConversationId === conv.id && isChatRoute
                                              ? "bg-accent text-accent-foreground font-medium"
                                              : "text-foreground/80"
                                          )}
                                          onContextMenu={(e) => handleContextMenu(e, conv)}
                                        >
                                          <button
                                            type="button"
                                            className="min-w-0 flex-1 truncate text-left"
                                            onClick={() => handleSelectConversation(conv.id)}
                                          >
                                            {conv.title}
                                          </button>
                                          <Dropdown>
                                            <DropdownTrigger asChild showChevron={false}>
                                              <button
                                                type="button"
                                                aria-label="Conversation actions"
                                                className={cn(
                                                  "inline-flex items-center justify-center h-6 w-6 shrink-0 rounded-md p-0 opacity-0 transition-opacity hover:bg-muted",
                                                  "group-hover:opacity-100",
                                                  activeConversationId === conv.id && "opacity-100"
                                                )}
                                              >
                                                <MoreHorizontal className="h-4 w-4" />
                                              </button>
                                            </DropdownTrigger>
                                            <DropdownContent align="end">
                                              <DropdownItem onClick={() => togglePin(conv.id)}>
                                                {conv.pinOrder != null ? (
                                                  <>
                                                    <PinOff className="mr-2 h-4 w-4" />
                                                    Unpin
                                                  </>
                                                ) : (
                                                  <>
                                                    <Pin className="mr-2 h-4 w-4" />
                                                    Pin
                                                  </>
                                                )}
                                              </DropdownItem>
                                              <DropdownItem onClick={() => handleStartEdit(conv)}>
                                                <Pencil className="mr-2 h-4 w-4" />
                                                Rename
                                              </DropdownItem>
                                              <DropdownItem
                                                onClick={() => regenerateTitle(conv.id)}
                                              >
                                                <Sparkles className="mr-2 h-4 w-4" />
                                                Regenerate title
                                              </DropdownItem>
                                              <DropdownItem
                                                onClick={() => handleOpenForkModal(conv)}
                                              >
                                                <GitFork className="mr-2 h-4 w-4" />
                                                Fork
                                              </DropdownItem>
                                              <DropdownItem
                                                onClick={() => handleOpenMoveModal(conv)}
                                              >
                                                <ArrowRightLeft className="mr-2 h-4 w-4" />
                                                Move to Project...
                                              </DropdownItem>
                                              <DropdownItem
                                                className="text-destructive"
                                                onClick={() => handleDeleteConversation(conv)}
                                              >
                                                <Trash2 className="mr-2 h-4 w-4" />
                                                Delete
                                              </DropdownItem>
                                            </DropdownContent>
                                          </Dropdown>
                                        </div>
                                      </ConversationPreview>
                                    )}
                                  </li>
                                ))}
                              </ul>
                            </div>
                          ))}
                        </div>
                      )}
                    </div>
                  )}

                  {/* Shared Conversations Section */}
                  {hasShared && (
                    <div className="mt-4">
                      <button
                        className={cn(
                          "flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-xs font-medium uppercase tracking-wider transition-colors",
                          "hover:bg-accent/50",
                          "text-muted-foreground"
                        )}
                        onClick={() => setSharedExpanded(!sharedExpanded)}
                        aria-expanded={sharedExpanded}
                        aria-controls="shared-conversations-list"
                      >
                        <span className="flex-1 text-left">Shared</span>
                        {sharedExpanded ? (
                          <ChevronDown className="h-3.5 w-3.5" aria-hidden="true" />
                        ) : (
                          <ChevronRight className="h-3.5 w-3.5" aria-hidden="true" />
                        )}
                      </button>
                      {sharedExpanded && (
                        <div id="shared-conversations-list" className="mt-1 space-y-1">
                          {Array.from(sharedByProject.entries()).map(
                            ([projectId, { projectName, groups }]) => (
                              <div key={projectId}>
                                <button
                                  className={cn(
                                    "flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-sm transition-colors",
                                    "hover:bg-accent/50",
                                    "text-foreground/80"
                                  )}
                                  onClick={() => toggleProject(projectId)}
                                  aria-expanded={expandedProjects.has(projectId)}
                                  aria-controls={`project-${projectId}-conversations`}
                                >
                                  <FolderOpen
                                    className="h-4 w-4 text-muted-foreground"
                                    aria-hidden="true"
                                  />
                                  <span className="flex-1 text-left truncate">{projectName}</span>
                                  {expandedProjects.has(projectId) ? (
                                    <ChevronDown
                                      className="h-3.5 w-3.5 text-muted-foreground"
                                      aria-hidden="true"
                                    />
                                  ) : (
                                    <ChevronRight
                                      className="h-3.5 w-3.5 text-muted-foreground"
                                      aria-hidden="true"
                                    />
                                  )}
                                </button>
                                {expandedProjects.has(projectId) && (
                                  <div
                                    id={`project-${projectId}-conversations`}
                                    className="mt-1 space-y-3 pl-2"
                                  >
                                    {groups.map((group) => (
                                      <div key={group.label}>
                                        <div className="mb-1 px-2 text-[11px] font-medium text-muted-foreground">
                                          {group.label}
                                        </div>
                                        <ul className="space-y-0.5">
                                          {group.conversations.map((conv) => (
                                            <li key={conv.id}>
                                              <ConversationPreview conv={conv}>
                                                <button
                                                  className={cn(
                                                    "group flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-left text-sm",
                                                    "hover:bg-accent/50",
                                                    activeConversationId === conv.id && isChatRoute
                                                      ? "bg-accent text-accent-foreground font-medium"
                                                      : "text-muted-foreground"
                                                  )}
                                                  onClick={() => handleSelectConversation(conv.id)}
                                                  onContextMenu={(e) => handleContextMenu(e, conv)}
                                                >
                                                  <span className="min-w-0 flex-1 truncate">
                                                    {conv.title}
                                                  </span>
                                                </button>
                                              </ConversationPreview>
                                            </li>
                                          ))}
                                        </ul>
                                      </div>
                                    ))}
                                  </div>
                                )}
                              </div>
                            )
                          )}
                        </div>
                      )}
                    </div>
                  )}
                </>
              )}
            </div>

            {/* Collapse button (desktop only) */}
            {onCollapsedChange && (
              <div className="hidden shrink-0 border-t px-2 py-2 lg:block">
                <button
                  onClick={() => onCollapsedChange(true)}
                  className="flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-xs text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
                  aria-label="Collapse sidebar"
                >
                  <ChevronLeft className="h-3.5 w-3.5" />
                  <span>Collapse</span>
                </button>
              </div>
            )}

            {/* Footer */}
            {(config?.branding.footer_text ||
              config?.branding.footer_links.length ||
              config?.branding.version) && (
              <div className="shrink-0 border-t bg-card px-3 py-2 space-y-1.5">
                {config?.branding.footer_text && (
                  <p className="text-[11px] text-muted-foreground">{config.branding.footer_text}</p>
                )}
                {config?.branding.footer_links.length > 0 && (
                  <div className="flex flex-wrap gap-x-3 gap-y-1">
                    {config.branding.footer_links.map((link, index) => (
                      <a
                        key={index}
                        href={link.url}
                        target={link.url.startsWith("http") ? "_blank" : undefined}
                        rel={link.url.startsWith("http") ? "noopener noreferrer" : undefined}
                        className="text-[11px] text-muted-foreground hover:text-foreground transition-colors"
                      >
                        {link.label}
                      </a>
                    ))}
                  </div>
                )}
                {config?.branding.version && (
                  <p className="text-[10px] text-muted-foreground/50">v{config.branding.version}</p>
                )}
              </div>
            )}
          </div>
        )}
      </aside>

      {/* Context Menu for Conversations */}
      <ContextMenu
        open={contextMenuConversation !== null}
        onOpenChange={(open) => {
          if (!open) handleContextMenuClose();
        }}
        position={contextMenuPosition}
      >
        {contextMenuConversation && (
          <>
            <ContextMenuItem
              onClick={() => {
                togglePin(contextMenuConversation.id);
              }}
            >
              {contextMenuConversation.pinOrder != null ? (
                <>
                  <PinOff className="mr-2 h-4 w-4" />
                  Unpin
                </>
              ) : (
                <>
                  <Pin className="mr-2 h-4 w-4" />
                  Pin
                </>
              )}
            </ContextMenuItem>
            <ContextMenuItem
              onClick={() => {
                handleStartEdit(contextMenuConversation);
              }}
            >
              <Pencil className="mr-2 h-4 w-4" />
              Rename
            </ContextMenuItem>
            <ContextMenuItem
              onClick={() => {
                regenerateTitle(contextMenuConversation.id);
              }}
            >
              <Sparkles className="mr-2 h-4 w-4" />
              Regenerate title
            </ContextMenuItem>
            <ContextMenuItem
              onClick={() => {
                handleOpenForkModal(contextMenuConversation);
              }}
            >
              <GitFork className="mr-2 h-4 w-4" />
              Fork
            </ContextMenuItem>
            <ContextMenuItem
              onClick={() => {
                handleOpenMoveModal(contextMenuConversation);
              }}
            >
              <ArrowRightLeft className="mr-2 h-4 w-4" />
              Move to Project...
            </ContextMenuItem>
            <ContextMenuSeparator />
            <ContextMenuItem
              className="text-destructive"
              onClick={() => {
                handleDeleteConversation(contextMenuConversation);
              }}
            >
              <Trash2 className="mr-2 h-4 w-4" />
              Delete
            </ContextMenuItem>
          </>
        )}
      </ContextMenu>

      {moveModalConversation && (
        <MoveToProjectModal
          open={moveModalConversation !== null}
          onClose={() => setMoveModalConversation(null)}
          conversation={{
            id: moveModalConversation.id,
            title: moveModalConversation.title,
            projectId: moveModalConversation.projectId,
          }}
          onMove={handleMoveToProject}
        />
      )}

      {forkModalConversation && (
        <ForkConversationModal
          open={forkModalConversation !== null}
          onClose={() => setForkModalConversation(null)}
          conversation={forkModalConversation}
          onFork={handleForkConversation}
        />
      )}
    </>
  );
}
