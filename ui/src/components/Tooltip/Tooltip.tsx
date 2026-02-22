import {
  useState,
  useRef,
  useEffect,
  useCallback,
  createContext,
  useContext,
  cloneElement,
  isValidElement,
  type ReactNode,
  type HTMLAttributes,
  type ReactElement,
} from "react";
import { createPortal } from "react-dom";
import { cn } from "@/utils/cn";

interface TooltipContextValue {
  open: boolean;
  setOpen: (open: boolean) => void;
  triggerRef: React.RefObject<HTMLElement | null>;
  delayDuration: number;
}

const TooltipContext = createContext<TooltipContextValue | null>(null);

function useTooltipContext() {
  const context = useContext(TooltipContext);
  if (!context) {
    throw new Error("Tooltip components must be used within a Tooltip");
  }
  return context;
}

interface TooltipProviderProps {
  children: ReactNode;
  delayDuration?: number;
}

export function TooltipProvider({ children }: TooltipProviderProps) {
  return <>{children}</>;
}

interface TooltipProps {
  children: ReactNode;
  delayDuration?: number;
}

export function Tooltip({ children, delayDuration = 200 }: TooltipProps) {
  const [open, setOpen] = useState(false);
  const triggerRef = useRef<HTMLElement>(null);

  return (
    <TooltipContext.Provider
      value={{
        open,
        setOpen,
        triggerRef,
        delayDuration,
      }}
    >
      {children}
    </TooltipContext.Provider>
  );
}

interface TooltipTriggerProps {
  children: ReactNode;
  asChild?: boolean;
}

export function TooltipTrigger({ children, asChild }: TooltipTriggerProps) {
  const ctx = useTooltipContext();
  const timeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const handleMouseEnter = useCallback(
    (e: React.MouseEvent) => {
      // Capture the trigger element on mouse enter
      (ctx.triggerRef as React.MutableRefObject<HTMLElement | null>).current =
        e.currentTarget as HTMLElement;
      timeoutRef.current = setTimeout(() => {
        ctx.setOpen(true);
      }, ctx.delayDuration);
    },
    [ctx]
  );

  const handleMouseLeave = useCallback(() => {
    if (timeoutRef.current) {
      clearTimeout(timeoutRef.current);
    }
    ctx.setOpen(false);
  }, [ctx]);

  useEffect(() => {
    return () => {
      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current);
      }
    };
  }, []);

  if (asChild && isValidElement(children)) {
    return cloneElement(children as ReactElement<HTMLAttributes<HTMLElement>>, {
      onMouseEnter: handleMouseEnter,
      onMouseLeave: handleMouseLeave,
      onFocus: (e: React.FocusEvent) => {
        (ctx.triggerRef as React.MutableRefObject<HTMLElement | null>).current =
          e.currentTarget as HTMLElement;
        ctx.setOpen(true);
      },
      onBlur: () => ctx.setOpen(false),
    });
  }

  return (
    <span
      ref={ctx.triggerRef as React.RefObject<HTMLSpanElement>}
      onMouseEnter={handleMouseEnter}
      onMouseLeave={handleMouseLeave}
      onFocus={() => ctx.setOpen(true)}
      onBlur={() => ctx.setOpen(false)}
    >
      {children}
    </span>
  );
}

interface TooltipContentProps extends HTMLAttributes<HTMLDivElement> {
  side?: "top" | "right" | "bottom" | "left";
  sideOffset?: number;
}

export function TooltipContent({
  className,
  children,
  side = "top",
  sideOffset = 4,
  ...props
}: TooltipContentProps) {
  const { open, triggerRef } = useTooltipContext();
  const contentRef = useRef<HTMLDivElement>(null);
  const [position, setPosition] = useState<{ top: number; left: number } | null>(null);
  const [isPositioned, setIsPositioned] = useState(false);

  const updatePosition = useCallback(() => {
    if (triggerRef.current && contentRef.current) {
      const triggerRect = triggerRef.current.getBoundingClientRect();
      const contentRect = contentRef.current.getBoundingClientRect();

      let top = 0;
      let left = 0;

      switch (side) {
        case "top":
          top = triggerRect.top - contentRect.height - sideOffset;
          left = triggerRect.left + (triggerRect.width - contentRect.width) / 2;
          break;
        case "bottom":
          top = triggerRect.bottom + sideOffset;
          left = triggerRect.left + (triggerRect.width - contentRect.width) / 2;
          break;
        case "left":
          top = triggerRect.top + (triggerRect.height - contentRect.height) / 2;
          left = triggerRect.left - contentRect.width - sideOffset;
          break;
        case "right":
          top = triggerRect.top + (triggerRect.height - contentRect.height) / 2;
          left = triggerRect.right + sideOffset;
          break;
      }

      // Ensure tooltip stays within viewport
      left = Math.max(8, Math.min(left, window.innerWidth - contentRect.width - 8));
      top = Math.max(8, Math.min(top, window.innerHeight - contentRect.height - 8));

      setPosition({ top, left });
      // Use RAF to ensure position is applied before showing
      requestAnimationFrame(() => {
        setIsPositioned(true);
      });
    }
  }, [side, sideOffset, triggerRef]);

  useEffect(() => {
    if (open) {
      // Need to wait for the content to be rendered to get its dimensions
      requestAnimationFrame(updatePosition);
    } else {
      // Reset when closing so next open starts with clean state
      setPosition(null);
      setIsPositioned(false);
    }
  }, [open, updatePosition]);

  if (!open) return null;

  return createPortal(
    <div
      ref={contentRef}
      role="tooltip"
      className={cn(
        "fixed z-50 overflow-hidden rounded-md border bg-popover px-3 py-1.5 text-sm text-popover-foreground shadow-md",
        className
      )}
      style={{
        top: position?.top ?? -9999,
        left: position?.left ?? -9999,
        opacity: isPositioned ? 1 : 0,
        pointerEvents: "none",
      }}
      {...props}
    >
      {children}
    </div>,
    document.body
  );
}
