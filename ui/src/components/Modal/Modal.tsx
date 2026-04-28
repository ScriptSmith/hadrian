import {
  useEffect,
  useCallback,
  useRef,
  useId,
  createContext,
  useContext,
  type ReactNode,
  type HTMLAttributes,
} from "react";
import { createPortal } from "react-dom";
import { X } from "lucide-react";
import { cn } from "@/utils/cn";
import { Button } from "@/components/Button/Button";

interface ModalContextValue {
  titleId: string;
  descriptionId: string;
}

const ModalContext = createContext<ModalContextValue | null>(null);

function useModalContext() {
  return useContext(ModalContext);
}

// Shared stack of open modal contents. Only the top entry is interactive —
// stacked dialogs (e.g. a confirm-modal opened over a form-modal) used to
// share Escape/Tab handlers and could let focus tab into the dialog
// underneath. Tracking the stack lets us route keyboard events to the
// topmost dialog only and apply `inert` to everything beneath it.
const modalStack: HTMLElement[] = [];

function refreshInertState() {
  const top = modalStack[modalStack.length - 1] ?? null;

  // Background app: inert when any modal is open, otherwise interactive.
  const root = document.getElementById("root");
  if (root) {
    if (modalStack.length > 0) {
      root.setAttribute("inert", "");
      root.setAttribute("aria-hidden", "true");
    } else {
      root.removeAttribute("inert");
      root.removeAttribute("aria-hidden");
    }
  }

  // Stacked modals: every dialog except the top one is inert.
  for (const node of modalStack) {
    if (node === top) {
      node.removeAttribute("inert");
      node.removeAttribute("aria-hidden");
    } else {
      node.setAttribute("inert", "");
      node.setAttribute("aria-hidden", "true");
    }
  }
}

export interface ModalProps {
  open: boolean;
  onClose: () => void;
  children: ReactNode;
  className?: string;
}

export function Modal({ open, onClose, children, className }: ModalProps) {
  const contentRef = useRef<HTMLDivElement>(null);
  const previousActiveElement = useRef<HTMLElement | null>(null);
  const titleId = useId();
  const descriptionId = useId();

  const isTopModal = useCallback(() => {
    const node = contentRef.current;
    return node !== null && modalStack[modalStack.length - 1] === node;
  }, []);

  const handleEscape = useCallback(
    (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      // Stacked modals share a window-level keydown listener; only the
      // topmost dialog should react, otherwise Escape closes everything.
      if (!isTopModal()) return;
      onClose();
    },
    [onClose, isTopModal]
  );

  // Focus trap - keep focus within modal
  const handleTabKey = useCallback(
    (e: KeyboardEvent) => {
      if (e.key !== "Tab" || !contentRef.current) return;
      if (!isTopModal()) return;

      const focusableElements = contentRef.current.querySelectorAll<HTMLElement>(
        'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
      );
      const firstElement = focusableElements[0];
      const lastElement = focusableElements[focusableElements.length - 1];

      if (e.shiftKey && document.activeElement === firstElement) {
        e.preventDefault();
        lastElement?.focus();
      } else if (!e.shiftKey && document.activeElement === lastElement) {
        e.preventDefault();
        firstElement?.focus();
      }
    },
    [isTopModal]
  );

  // Handle initial focus when modal opens (only runs when `open` changes)
  useEffect(() => {
    if (!open) return;
    // Store currently focused element
    previousActiveElement.current = document.activeElement as HTMLElement;
    document.body.style.overflow = "hidden";

    const node = contentRef.current;
    if (node) {
      modalStack.push(node);
      refreshInertState();
    }

    // Focus the first input if available, otherwise the modal content
    requestAnimationFrame(() => {
      const firstInput = node?.querySelector<HTMLElement>("input, select, textarea");
      if (firstInput) {
        firstInput.focus();
      } else {
        const firstFocusable = node?.querySelector<HTMLElement>(
          'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
        );
        if (firstFocusable) {
          firstFocusable.focus();
        } else {
          node?.focus();
        }
      }
    });

    return () => {
      if (node) {
        const idx = modalStack.lastIndexOf(node);
        if (idx !== -1) modalStack.splice(idx, 1);
      }
      // Only release the body scroll lock when the last modal closes, so a
      // background page doesn't briefly start scrolling between stacked
      // dialogs.
      if (modalStack.length === 0) {
        document.body.style.overflow = "";
      }
      refreshInertState();
      // Restore focus to previously focused element
      if (previousActiveElement.current) {
        previousActiveElement.current.focus();
      }
    };
  }, [open]);

  // Set up keyboard event listeners (separate from focus logic)
  useEffect(() => {
    if (open) {
      document.addEventListener("keydown", handleEscape);
      document.addEventListener("keydown", handleTabKey);
    }
    return () => {
      document.removeEventListener("keydown", handleEscape);
      document.removeEventListener("keydown", handleTabKey);
    };
  }, [open, handleEscape, handleTabKey]);

  if (!open) return null;

  return createPortal(
    <ModalContext.Provider value={{ titleId, descriptionId }}>
      <div className="fixed inset-0 z-50 flex items-center justify-center">
        {/* Backdrop */}
        <div
          className="fixed inset-0 bg-black/50 backdrop-blur-sm"
          onClick={onClose}
          aria-hidden="true"
        />
        {/* Content */}
        {/* eslint-disable-next-line jsx-a11y/click-events-have-key-events, jsx-a11y/no-noninteractive-element-interactions -- dialog stopPropagation to prevent backdrop close; Escape handled separately */}
        <div
          ref={contentRef}
          role="dialog"
          aria-modal="true"
          aria-labelledby={titleId}
          aria-describedby={descriptionId}
          tabIndex={-1}
          className={cn(
            "relative z-50 w-full max-w-lg rounded-lg border bg-background p-6 shadow-lg",
            "animate-in fade-in-0 zoom-in-95",
            "focus:outline-none",
            className
          )}
          onClick={(e) => e.stopPropagation()}
        >
          {children}
        </div>
      </div>
    </ModalContext.Provider>,
    document.body
  );
}

export function ModalHeader({ className, children, ...props }: HTMLAttributes<HTMLDivElement>) {
  const context = useModalContext();
  // If children is a string, render it as a title with proper id
  if (typeof children === "string") {
    return (
      <div className={cn("mb-4 pr-8", className)} {...props}>
        <h2 id={context?.titleId} className="text-lg font-semibold">
          {children}
        </h2>
      </div>
    );
  }
  // Stack title and description vertically, with padding-right for close button
  return (
    <div className={cn("space-y-1.5 pr-8", className)} {...props}>
      {children}
    </div>
  );
}

export function ModalTitle({ className, ...props }: HTMLAttributes<HTMLHeadingElement>) {
  const context = useModalContext();
  // eslint-disable-next-line jsx-a11y/heading-has-content -- content provided via children in props spread
  return <h2 id={context?.titleId} className={cn("text-lg font-semibold", className)} {...props} />;
}

export function ModalDescription({ className, ...props }: HTMLAttributes<HTMLParagraphElement>) {
  const context = useModalContext();
  return (
    <p
      id={context?.descriptionId}
      className={cn("text-sm text-muted-foreground", className)}
      {...props}
    />
  );
}

export function ModalClose({ onClose }: { onClose: () => void }) {
  return (
    <Button variant="ghost" size="icon" onClick={onClose} className="absolute right-4 top-4">
      <X className="h-4 w-4" />
      <span className="sr-only">Close</span>
    </Button>
  );
}

export function ModalContent({ className, ...props }: HTMLAttributes<HTMLDivElement>) {
  // -mx-1 px-1 gives focus rings room to render without being clipped by modal border
  return <div className={cn("py-4 -mx-1 px-1", className)} {...props} />;
}

export function ModalFooter({ className, ...props }: HTMLAttributes<HTMLDivElement>) {
  return <div className={cn("mt-4 flex justify-end gap-2", className)} {...props} />;
}
