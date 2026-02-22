import { useEffect, useRef, useState, useCallback } from "react";
import { createPortal } from "react-dom";
import { MessageSquareQuote } from "lucide-react";
import { cn } from "@/utils/cn";

export interface QuoteSelectionPopoverProps {
  /** Whether the popover is visible */
  isOpen: boolean;
  /** Position coordinates for the popover */
  position: { x: number; y: number };
  /** The selected text to quote */
  selectedText: string;
  /** Callback when quote button is clicked */
  onQuote: (text: string) => void;
  /** Callback when popover should close */
  onClose: () => void;
}

/**
 * A floating popover that appears when text is selected in chat messages.
 * Shows a "Quote" button that inserts the selected text into the chat input.
 */
export function QuoteSelectionPopover({
  isOpen,
  position,
  selectedText,
  onQuote,
  onClose,
}: QuoteSelectionPopoverProps) {
  const popoverRef = useRef<HTMLDivElement>(null);
  const [adjustedPosition, setAdjustedPosition] = useState<{ top: number; left: number } | null>(
    null
  );

  // Adjust position to keep popover in viewport
  const updatePosition = useCallback(() => {
    if (!popoverRef.current) return;

    const rect = popoverRef.current.getBoundingClientRect();
    const padding = 8;

    // Position above the selection by default
    let top = position.y - rect.height - padding;
    let left = position.x - rect.width / 2;

    // If too close to top, show below selection
    if (top < padding) {
      top = position.y + padding;
    }

    // Keep within horizontal bounds
    left = Math.max(padding, Math.min(left, window.innerWidth - rect.width - padding));

    // Keep within vertical bounds
    top = Math.max(padding, Math.min(top, window.innerHeight - rect.height - padding));

    setAdjustedPosition({ top, left });
  }, [position]);

  // Update position when popover opens or position changes
  useEffect(() => {
    if (isOpen) {
      // Wait for render to get dimensions
      requestAnimationFrame(updatePosition);
    } else {
      setAdjustedPosition(null);
    }
  }, [isOpen, updatePosition]);

  // Close on click outside
  useEffect(() => {
    if (!isOpen) return;

    const handleClickOutside = (e: MouseEvent) => {
      if (popoverRef.current && !popoverRef.current.contains(e.target as Node)) {
        onClose();
      }
    };

    // Close on escape
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        onClose();
      }
    };

    // Delay adding listeners to avoid immediate close from the mouseup that triggered selection
    const timeoutId = setTimeout(() => {
      document.addEventListener("mousedown", handleClickOutside);
      document.addEventListener("keydown", handleKeyDown);
    }, 0);

    return () => {
      clearTimeout(timeoutId);
      document.removeEventListener("mousedown", handleClickOutside);
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [isOpen, onClose]);

  // Close when selection changes (user clicks elsewhere)
  useEffect(() => {
    if (!isOpen) return;

    const handleSelectionChange = () => {
      const selection = window.getSelection();
      const currentText = selection?.toString().trim() || "";
      // If selection is cleared or changed significantly, close the popover
      if (!currentText || currentText !== selectedText) {
        onClose();
      }
    };

    document.addEventListener("selectionchange", handleSelectionChange);
    return () => document.removeEventListener("selectionchange", handleSelectionChange);
  }, [isOpen, selectedText, onClose]);

  const handleQuoteClick = (e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    onQuote(selectedText);
    onClose();
    // Clear the selection after quoting
    window.getSelection()?.removeAllRanges();
  };

  if (!isOpen) return null;

  return createPortal(
    <div
      ref={popoverRef}
      role="dialog"
      aria-label="Quote selection"
      className="fixed z-50 flex items-center gap-1 rounded-md border bg-popover p-1 shadow-md"
      style={{
        top: adjustedPosition?.top ?? -9999,
        left: adjustedPosition?.left ?? -9999,
        opacity: adjustedPosition ? 1 : 0,
        pointerEvents: adjustedPosition ? "auto" : "none",
      }}
    >
      <button
        type="button"
        onClick={handleQuoteClick}
        className={cn(
          "flex items-center gap-1.5 rounded px-2 py-1 text-sm font-medium",
          "text-popover-foreground hover:bg-accent hover:text-accent-foreground",
          "transition-colors focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-1"
        )}
      >
        <MessageSquareQuote className="h-4 w-4" />
        <span>Quote</span>
      </button>
    </div>,
    document.body
  );
}
