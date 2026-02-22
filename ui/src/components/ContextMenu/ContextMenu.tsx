import {
  useState,
  useRef,
  useEffect,
  useCallback,
  createContext,
  useContext,
  useId,
  type ReactNode,
  type HTMLAttributes,
  type ButtonHTMLAttributes,
  type KeyboardEvent as ReactKeyboardEvent,
} from "react";
import { createPortal } from "react-dom";
import { Check } from "lucide-react";
import { cn } from "@/utils/cn";

interface ContextMenuContextValue {
  open: boolean;
  onClose: () => void;
  highlightedIndex: number;
  setHighlightedIndex: (index: number) => void;
  menuId: string;
  registerItem: () => number;
  itemCount: number;
}

const ContextMenuContext = createContext<ContextMenuContextValue | null>(null);

function useContextMenuContext() {
  const context = useContext(ContextMenuContext);
  if (!context) {
    throw new Error("ContextMenu components must be used within a ContextMenu");
  }
  return context;
}

export interface ContextMenuPosition {
  x: number;
  y: number;
}

interface ContextMenuProps {
  children: ReactNode;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  position: ContextMenuPosition | null;
}

export function ContextMenu({ children, open, onOpenChange, position }: ContextMenuProps) {
  const [highlightedIndex, setHighlightedIndex] = useState(-1);
  const [itemCount, setItemCount] = useState(0);
  const menuId = useId();
  const itemCounterRef = useRef(0);

  const onClose = useCallback(() => {
    onOpenChange(false);
  }, [onOpenChange]);

  const registerItem = useCallback(() => {
    const index = itemCounterRef.current;
    itemCounterRef.current += 1;
    setItemCount(itemCounterRef.current);
    return index;
  }, []);

  // Reset state when opening
  useEffect(() => {
    if (open) {
      itemCounterRef.current = 0;
      setHighlightedIndex(-1); // Start with nothing highlighted
      setItemCount(0);
    }
  }, [open]);

  // Handle keyboard navigation (click outside handled in ContextMenuContent)
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      switch (e.key) {
        case "Escape":
          e.preventDefault();
          onClose();
          break;
        case "ArrowDown":
          e.preventDefault();
          // If nothing highlighted, start at first item; otherwise wrap around
          setHighlightedIndex((prev) => (prev < 0 ? 0 : prev < itemCount - 1 ? prev + 1 : 0));
          break;
        case "ArrowUp":
          e.preventDefault();
          // If nothing highlighted, start at last item; otherwise wrap around
          setHighlightedIndex((prev) =>
            prev < 0 ? itemCount - 1 : prev > 0 ? prev - 1 : itemCount - 1
          );
          break;
        case "Home":
          e.preventDefault();
          setHighlightedIndex(0);
          break;
        case "End":
          e.preventDefault();
          setHighlightedIndex(itemCount - 1);
          break;
        case "Tab":
          onClose();
          break;
      }
    };

    if (open) {
      document.addEventListener("keydown", handleKeyDown);
    }

    return () => {
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [open, onClose, itemCount]);

  if (!open || !position) return null;

  return (
    <ContextMenuContext.Provider
      value={{
        open,
        onClose,
        highlightedIndex,
        setHighlightedIndex,
        menuId,
        registerItem,
        itemCount,
      }}
    >
      <ContextMenuContent position={position}>{children}</ContextMenuContent>
    </ContextMenuContext.Provider>
  );
}

interface ContextMenuContentProps extends HTMLAttributes<HTMLDivElement> {
  position: ContextMenuPosition;
}

function ContextMenuContent({ className, children, position, ...props }: ContextMenuContentProps) {
  const { menuId, onClose } = useContextMenuContext();
  const contentRef = useRef<HTMLDivElement>(null);
  const [finalPosition, setFinalPosition] = useState<ContextMenuPosition>(position);

  // Adjust position to keep menu in viewport after mount
  useEffect(() => {
    // Use requestAnimationFrame to ensure DOM is ready
    const frame = requestAnimationFrame(() => {
      if (contentRef.current) {
        const rect = contentRef.current.getBoundingClientRect();
        const viewportWidth = window.innerWidth;
        const viewportHeight = window.innerHeight;

        let x = position.x;
        let y = position.y;

        // Adjust horizontal position if menu would overflow right edge
        if (x + rect.width > viewportWidth - 8) {
          x = viewportWidth - rect.width - 8;
        }
        // Keep at least 8px from left edge
        if (x < 8) {
          x = 8;
        }

        // Adjust vertical position if menu would overflow bottom edge
        if (y + rect.height > viewportHeight - 8) {
          y = viewportHeight - rect.height - 8;
        }
        // Keep at least 8px from top edge
        if (y < 8) {
          y = 8;
        }

        setFinalPosition({ x, y });
      }
    });

    return () => cancelAnimationFrame(frame);
  }, [position]);

  // Handle click outside and scroll to close
  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (contentRef.current && !contentRef.current.contains(e.target as Node)) {
        onClose();
      }
    };

    // Small delay to avoid closing immediately from the triggering click
    const timeoutId = setTimeout(() => {
      document.addEventListener("mousedown", handleClickOutside);
      document.addEventListener("scroll", onClose, true);
    }, 0);

    return () => {
      clearTimeout(timeoutId);
      document.removeEventListener("mousedown", handleClickOutside);
      document.removeEventListener("scroll", onClose, true);
    };
  }, [onClose]);

  return createPortal(
    <div
      ref={contentRef}
      id={menuId}
      role="menu"
      aria-orientation="vertical"
      tabIndex={-1}
      className={cn(
        "fixed z-50 min-w-[8rem] overflow-hidden rounded-lg border bg-popover/95 p-1.5 text-popover-foreground shadow-xl backdrop-blur-sm",
        "animate-in fade-in-0 zoom-in-95",
        "ring-1 ring-black/5 dark:ring-white/10",
        className
      )}
      style={{ top: finalPosition.y, left: finalPosition.x }}
      {...props}
    >
      {children}
    </div>,
    document.body
  );
}

interface ContextMenuItemProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  selected?: boolean;
}

export function ContextMenuItem({ className, children, selected, ...props }: ContextMenuItemProps) {
  const { onClose, highlightedIndex, registerItem, setHighlightedIndex } = useContextMenuContext();
  const itemRef = useRef<HTMLButtonElement>(null);
  const [itemIndex, setItemIndex] = useState<number>(-1);

  // Register this item and get its index
  useEffect(() => {
    const index = registerItem();
    setItemIndex(index);
  }, [registerItem]);

  // Focus management when highlighted (only for valid indices)
  useEffect(() => {
    if (itemIndex >= 0 && highlightedIndex === itemIndex && itemRef.current) {
      itemRef.current.focus();
    }
  }, [highlightedIndex, itemIndex]);

  const isHighlighted = itemIndex >= 0 && highlightedIndex === itemIndex;

  const handleKeyDown = (e: ReactKeyboardEvent<HTMLButtonElement>) => {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      props.onClick?.(e as unknown as React.MouseEvent<HTMLButtonElement>);
      onClose();
    }
  };

  return (
    <button
      ref={itemRef}
      type="button"
      role="menuitem"
      tabIndex={isHighlighted ? 0 : -1}
      className={cn(
        "relative flex w-full cursor-pointer select-none items-center rounded-md px-2.5 py-2 text-sm outline-none",
        "transition-colors duration-100",
        "hover:bg-accent hover:text-accent-foreground",
        "focus:bg-accent focus:text-accent-foreground",
        "disabled:pointer-events-none disabled:opacity-50",
        isHighlighted && "bg-accent text-accent-foreground",
        className
      )}
      onClick={(e) => {
        props.onClick?.(e);
        onClose();
      }}
      onKeyDown={handleKeyDown}
      onMouseEnter={() => setHighlightedIndex(itemIndex)}
      {...props}
    >
      {selected && <Check className="mr-2 h-4 w-4 text-primary" />}
      {children}
    </button>
  );
}

export function ContextMenuSeparator({ className, ...props }: HTMLAttributes<HTMLDivElement>) {
  return <div className={cn("-mx-1.5 my-1.5 h-px bg-border/50", className)} {...props} />;
}
