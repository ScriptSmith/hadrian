import {
  Bot,
  Copy,
  Check,
  User,
  GitFork,
  Pencil,
  X,
  RotateCcw,
  Download,
  ExternalLink,
} from "lucide-react";
import {
  useState,
  memo,
  useCallback,
  useRef,
  useEffect,
  type MouseEvent,
  type KeyboardEvent,
} from "react";

import { Avatar, AvatarFallback } from "@/components/Avatar/Avatar";
import { Button } from "@/components/Button/Button";
import { Markdown } from "@/components/Markdown/Markdown";
import { QuoteSelectionPopover } from "@/components/QuoteSelectionPopover";
import { Textarea } from "@/components/Textarea/Textarea";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/Tooltip/Tooltip";
import { useChatUIStore, useIsEditing } from "@/stores/chatUIStore";
import { useIsStreaming } from "@/stores/streamingStore";
import { cn } from "@/utils/cn";
import { formatCost, formatTokens } from "@/utils/formatters";

import type { ChatMessage as ChatMessageType } from "@/components/chat-types";

/**
 * ChatMessage - Memoized User/Assistant Message Component
 *
 * ## Performance Design
 *
 * This component renders a single chat message (user or assistant). It uses
 * React.memo with a custom comparator to prevent re-renders when parent
 * state changes but message content hasn't.
 *
 * ## Re-render Behavior
 *
 * ChatMessage only re-renders when:
 * - `message.id` changes (different message)
 * - `message.content` changes (edited message)
 * - `message.role` changes (role correction)
 * - `isStreaming` prop changes
 * - `message.files` array length changes
 * - `message.usage.totalTokens` changes
 * - `message.feedback.rating` changes
 *
 * It does NOT re-render when:
 * - Timestamp object reference changes (but value is same)
 * - Parent component re-renders for other reasons
 * - Sibling messages update
 *
 * ## When This Component Is Used
 *
 * - **User messages**: Always rendered via this component
 * - **Assistant messages**: Only for single-model responses or as a fallback
 *   (multi-model responses use MultiModelResponse -> ModelResponseCard)
 *
 * ## Memo Comparator Location
 *
 * See the export at the bottom of this file for the custom areEqual function.
 */

interface ChatMessageProps {
  message: ChatMessageType;
  isStreaming?: boolean;
  /** Callback to fork the conversation up to and including this message */
  onFork?: (messageId: string) => void;
  /** Callback to save edited message and re-run (for user messages) */
  onSaveEdit?: (messageId: string, newContent: string) => void;
  /** Callback to regenerate all responses to this user message */
  onRegenerate?: (messageId: string) => void;
}

function ChatMessageComponent({
  message,
  isStreaming = false,
  onFork,
  onSaveEdit,
  onRegenerate,
}: ChatMessageProps) {
  const [copied, setCopied] = useState(false);
  const isUser = message.role === "user";
  const isAnyStreaming = useIsStreaming();

  // Inline editing state
  const isEditing = useIsEditing(message.id);
  const [editContent, setEditContent] = useState(message.content);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const { startEditing, stopEditing } = useChatUIStore();

  // Reset edit content when message content changes or editing starts
  useEffect(() => {
    if (isEditing) {
      setEditContent(message.content);
      // Focus textarea when editing starts
      setTimeout(() => textareaRef.current?.focus(), 0);
    }
  }, [isEditing, message.content]);

  const handleStartEdit = useCallback(() => {
    startEditing(message.id);
  }, [startEditing, message.id]);

  const handleRegenerate = useCallback(() => {
    onRegenerate?.(message.id);
  }, [onRegenerate, message.id]);

  const handleCancelEdit = useCallback(() => {
    setEditContent(message.content);
    stopEditing();
  }, [stopEditing, message.content]);

  const handleSaveEdit = useCallback(() => {
    if (editContent.trim() && editContent !== message.content) {
      onSaveEdit?.(message.id, editContent.trim());
    }
    stopEditing();
  }, [editContent, message.content, message.id, onSaveEdit, stopEditing]);

  const handleEditKeyDown = useCallback(
    (e: KeyboardEvent<HTMLTextAreaElement>) => {
      if (e.key === "Escape") {
        e.preventDefault();
        handleCancelEdit();
      } else if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
        e.preventDefault();
        handleSaveEdit();
      }
    },
    [handleCancelEdit, handleSaveEdit]
  );

  const handleCopy = async () => {
    await navigator.clipboard.writeText(message.content);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  // Quote selection state
  const [quotePopover, setQuotePopover] = useState<{
    isOpen: boolean;
    position: { x: number; y: number };
    selectedText: string;
  }>({ isOpen: false, position: { x: 0, y: 0 }, selectedText: "" });
  const { setQuotedText } = useChatUIStore();

  const handleContentMouseUp = useCallback(
    (e: MouseEvent) => {
      // Don't show quote popover during streaming
      if (isStreaming) return;

      const selection = window.getSelection();
      const selectedText = selection?.toString().trim() || "";

      if (selectedText.length > 0) {
        setQuotePopover({
          isOpen: true,
          position: { x: e.clientX, y: e.clientY },
          selectedText,
        });
      }
    },
    [isStreaming]
  );

  const handleQuote = useCallback(
    (text: string) => {
      setQuotedText({
        messageId: message.id,
        text,
      });
    },
    [setQuotedText, message.id]
  );

  const handleCloseQuotePopover = useCallback(() => {
    setQuotePopover((prev) => ({ ...prev, isOpen: false }));
  }, []);

  return (
    <article
      className={cn("group flex gap-3 py-4", isUser ? "flex-row-reverse" : "flex-row")}
      aria-label={`${isUser ? "Your" : "Assistant"} message`}
    >
      <Avatar className="h-9 w-9 shrink-0 shadow-sm">
        <AvatarFallback
          className={cn(
            "transition-colors",
            isUser
              ? "bg-gradient-to-br from-primary to-primary/80 text-primary-foreground"
              : "bg-gradient-to-br from-secondary to-secondary/80"
          )}
        >
          {isUser ? <User className="h-4 w-4" /> : <Bot className="h-4 w-4" />}
        </AvatarFallback>
      </Avatar>

      <div
        className={cn("flex max-w-[85%] flex-col gap-1.5", isUser ? "items-end" : "items-start")}
      >
        {/* eslint-disable-next-line jsx-a11y/no-static-element-interactions -- onMouseUp for text selection quoting, not an interactive action */}
        <div
          className="rounded-2xl px-4 py-3 shadow-sm transition-shadow hover:shadow-md bg-card border"
          onMouseUp={handleContentMouseUp}
        >
          {message.files && message.files.length > 0 && (
            <div className="mb-3 flex flex-wrap gap-2">
              {message.files.map((file) => {
                // Use preview or base64 (both are full data URLs)
                const imageSrc = file.preview || file.base64;
                const isImage = file.type.startsWith("image/") && imageSrc;
                const hasContent = !!file.base64;

                const handleClick = () => {
                  if (!hasContent) return;

                  if (isImage) {
                    // Open image in new tab
                    window.open(imageSrc, "_blank");
                  } else {
                    // Download file
                    const link = document.createElement("a");
                    link.href = file.base64;
                    link.download = file.name;
                    document.body.appendChild(link);
                    link.click();
                    document.body.removeChild(link);
                  }
                };

                return (
                  <button
                    type="button"
                    key={file.id}
                    onClick={handleClick}
                    disabled={!hasContent}
                    className={cn(
                      "flex flex-col gap-1 rounded-lg overflow-hidden bg-muted text-left transition-all",
                      hasContent
                        ? "cursor-pointer hover:bg-muted/80 hover:ring-2 hover:ring-primary/20"
                        : "cursor-not-allowed opacity-60"
                    )}
                    title={
                      hasContent
                        ? isImage
                          ? "Click to open"
                          : "Click to download"
                        : "File content not available"
                    }
                  >
                    {isImage ? (
                      <div className="relative group">
                        <img
                          src={imageSrc}
                          alt={`Preview of ${file.name}`}
                          className="max-w-[200px] max-h-[200px] object-contain rounded-t-lg"
                        />
                        <div className="absolute inset-0 flex items-center justify-center bg-black/0 group-hover:bg-black/30 transition-colors rounded-t-lg">
                          <ExternalLink className="h-6 w-6 text-white opacity-0 group-hover:opacity-100 transition-opacity drop-shadow-lg" />
                        </div>
                      </div>
                    ) : null}
                    <div className="px-2 py-1.5 text-xs flex items-center gap-1.5">
                      {!isImage && hasContent && (
                        <Download className="h-3 w-3 text-muted-foreground" />
                      )}
                      <span className="max-w-[150px] truncate font-medium">{file.name}</span>
                    </div>
                  </button>
                );
              })}
            </div>
          )}

          <div
            className="break-words text-sm leading-relaxed"
            aria-live={isStreaming ? "polite" : undefined}
            aria-busy={isStreaming}
          >
            {isUser ? (
              isEditing ? (
                <div className="flex flex-col gap-2 min-w-[300px] sm:min-w-[400px]">
                  <Textarea
                    ref={textareaRef}
                    value={editContent}
                    onChange={(e) => setEditContent(e.target.value)}
                    onKeyDown={handleEditKeyDown}
                    className="min-h-[100px] w-full resize-y"
                    placeholder="Edit your message..."
                  />
                  <div className="flex gap-2 justify-end">
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={handleCancelEdit}
                      className="h-7 px-2"
                    >
                      <X className="h-3 w-3 mr-1" />
                      Cancel
                    </Button>
                    <Button
                      variant="primary"
                      size="sm"
                      onClick={handleSaveEdit}
                      disabled={!editContent.trim() || editContent === message.content}
                      className="h-7 px-2"
                    >
                      Save & Resend
                    </Button>
                  </div>
                  <span className="text-xs text-muted-foreground">
                    Ctrl+Enter to save · Escape to cancel
                  </span>
                </div>
              ) : (
                <Markdown content={message.content} />
              )
            ) : (
              <>
                <Markdown content={message.content} />
                {isStreaming && (
                  <>
                    <span
                      className="inline-block h-4 w-0.5 animate-blink rounded-full bg-primary"
                      aria-hidden="true"
                    />
                    <span className="sr-only">Generating response...</span>
                  </>
                )}
              </>
            )}
          </div>
        </div>

        {/* Hide action bar when editing */}
        {!isEditing && (
          <div
            className={cn(
              "flex items-center gap-2 transition-opacity",
              // Mobile: always visible, Desktop: show on hover
              "opacity-100 sm:opacity-0 sm:group-hover:opacity-100",
              isUser ? "flex-row-reverse" : "flex-row"
            )}
          >
            {/* Regenerate button for user messages - disabled during streaming */}
            {isUser && onRegenerate && !isAnyStreaming && (
              <Tooltip>
                <TooltipTrigger asChild>
                  <Button
                    variant="ghost"
                    size="sm"
                    className="h-7 w-7 p-0"
                    onClick={handleRegenerate}
                    aria-label="Regenerate responses"
                  >
                    <RotateCcw className="h-3 w-3" />
                  </Button>
                </TooltipTrigger>
                <TooltipContent side="top">Regenerate</TooltipContent>
              </Tooltip>
            )}
            {/* Edit button for user messages - disabled during streaming */}
            {isUser && onSaveEdit && !isAnyStreaming && (
              <Tooltip>
                <TooltipTrigger asChild>
                  <Button
                    variant="ghost"
                    size="sm"
                    className="h-7 w-7 p-0"
                    onClick={handleStartEdit}
                    aria-label="Edit message"
                  >
                    <Pencil className="h-3 w-3" />
                  </Button>
                </TooltipTrigger>
                <TooltipContent side="top">Edit & resend</TooltipContent>
              </Tooltip>
            )}
            <Button
              variant="ghost"
              size="sm"
              className={cn("h-7 w-7 p-0 transition-all", copied && "text-success")}
              onClick={handleCopy}
              aria-label={copied ? "Copied to clipboard" : "Copy message"}
            >
              {copied ? (
                <Check className="h-3 w-3 animate-bounce-in" />
              ) : (
                <Copy className="h-3 w-3" />
              )}
            </Button>
            {onFork && (
              <Tooltip>
                <TooltipTrigger asChild>
                  <Button
                    variant="ghost"
                    size="sm"
                    className="h-7 w-7 p-0"
                    onClick={() => onFork(message.id)}
                    aria-label="Fork conversation from here"
                  >
                    <GitFork className="h-3 w-3" />
                  </Button>
                </TooltipTrigger>
                <TooltipContent side="top">Fork from here</TooltipContent>
              </Tooltip>
            )}
            <span className="text-xs text-muted-foreground">
              {message.timestamp.toLocaleTimeString([], {
                hour: "2-digit",
                minute: "2-digit",
              })}
            </span>
            {!isUser && message.usage && (
              <Tooltip>
                <TooltipTrigger asChild>
                  <button
                    type="button"
                    className="text-xs text-muted-foreground cursor-help hover:text-foreground transition-colors"
                    aria-label="View token usage details"
                  >
                    {formatTokens(message.usage.totalTokens)} tokens
                    {message.usage.cost !== undefined && ` · ${formatCost(message.usage.cost)}`}
                  </button>
                </TooltipTrigger>
                <TooltipContent side="top" className="text-xs">
                  <div className="space-y-1">
                    <div>Input: {formatTokens(message.usage.inputTokens)} tokens</div>
                    <div>Output: {formatTokens(message.usage.outputTokens)} tokens</div>
                    {message.usage.cachedTokens !== undefined && message.usage.cachedTokens > 0 && (
                      <div>Cached: {formatTokens(message.usage.cachedTokens)} tokens</div>
                    )}
                    {message.usage.reasoningTokens !== undefined &&
                      message.usage.reasoningTokens > 0 && (
                        <div>Reasoning: {formatTokens(message.usage.reasoningTokens)} tokens</div>
                      )}
                    {message.usage.cost !== undefined && (
                      <div className="pt-1 border-t border-border/50">
                        Cost: {formatCost(message.usage.cost)}
                      </div>
                    )}
                  </div>
                </TooltipContent>
              </Tooltip>
            )}
          </div>
        )}
      </div>

      {/* Quote selection popover */}
      <QuoteSelectionPopover
        isOpen={quotePopover.isOpen}
        position={quotePopover.position}
        selectedText={quotePopover.selectedText}
        onQuote={handleQuote}
        onClose={handleCloseQuotePopover}
      />
    </article>
  );
}

/**
 * Memoized ChatMessage Export
 *
 * Custom comparator checks only properties that affect rendering:
 * - id, content, role: Core message data
 * - isStreaming: Changes rendering mode
 * - files.length: User message attachments
 * - usage.totalTokens: Assistant message stats
 * - feedback.rating: User feedback indicator
 * - onFork: Callback identity (for fork button visibility)
 * - onSaveEdit: Callback identity (for edit button visibility)
 * - onRegenerate: Callback identity (for regenerate button visibility)
 *
 * Intentionally IGNORES:
 * - timestamp: Displayed but doesn't affect layout
 * - Other usage fields: totalTokens is sufficient proxy
 * - feedback.selectedAsBest: Handled by parent (MultiModelResponse)
 */
export const ChatMessage = memo(ChatMessageComponent, (prevProps, nextProps) => {
  const prev = prevProps.message;
  const next = nextProps.message;

  return (
    prev.id === next.id &&
    prev.content === next.content &&
    prev.role === next.role &&
    prevProps.isStreaming === nextProps.isStreaming &&
    // For user messages, also check files
    prev.files?.length === next.files?.length &&
    // For assistant messages, check usage and feedback
    prev.usage?.totalTokens === next.usage?.totalTokens &&
    prev.feedback?.rating === next.feedback?.rating &&
    // Check callback identities (determines if buttons are shown)
    prevProps.onFork === nextProps.onFork &&
    prevProps.onSaveEdit === nextProps.onSaveEdit &&
    prevProps.onRegenerate === nextProps.onRegenerate
  );
});
