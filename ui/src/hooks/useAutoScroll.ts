import { useRef, useEffect, useCallback, useState, type RefObject } from "react";
import { useThrottledCallback } from "./useThrottledCallback";

/**
 * useAutoScroll - Smart Auto-Scroll for Streaming Chat Interfaces
 *
 * ## Problem Solved
 *
 * Chat interfaces need to auto-scroll as new content appears, but also respect
 * when the user scrolls up to read previous messages. This hook handles:
 *
 * 1. **User intent detection**: Distinguishes user scrolling from programmatic scrolling
 * 2. **Streaming optimization**: Uses 'instant' scroll during streaming (no animation lag)
 * 3. **Throttled handlers**: Prevents excessive state updates during scroll events
 *
 * ## Usage Pattern
 *
 * ```tsx
 * function ChatMessageList() {
 *   const { containerRef, userHasScrolledUp, handleScroll, scrollToBottom } = useAutoScroll();
 *
 *   // Auto-scroll effect
 *   useAutoScrollEffect(scrollToBottom, userHasScrolledUp, isStreaming, [messages.length]);
 *
 *   return (
 *     <div ref={containerRef} onScroll={handleScroll}>
 *       {messages.map(...)}
 *     </div>
 *   );
 * }
 * ```
 *
 * ## Performance Characteristics
 *
 * - Scroll handler is throttled (default 100ms) to reduce state update frequency
 * - Uses requestAnimationFrame for smooth programmatic scrolls
 * - Programmatic scroll detection prevents false "user scrolled up" triggers
 */

interface UseAutoScrollOptions {
  /** Threshold in pixels from bottom to consider "at bottom" */
  threshold?: number;
  /** Throttle delay for scroll handler in ms */
  throttleMs?: number;
  /** Whether content is currently streaming (disables ResizeObserver scroll checks) */
  isStreaming?: boolean;
}

interface UseAutoScrollReturn {
  /** Ref to attach to the scroll container */
  containerRef: RefObject<HTMLDivElement | null>;
  /** Whether the user has scrolled up from the bottom */
  userHasScrolledUp: boolean;
  /** Scroll handler to attach to the container */
  handleScroll: () => void;
  /** Scroll to bottom with appropriate behavior based on streaming state */
  scrollToBottom: (isStreaming: boolean) => void;
}

/**
 * Hook for managing auto-scroll behavior in chat-like interfaces.
 *
 * Features:
 * - Detects when user scrolls up to pause auto-scroll
 * - Detects when user scrolls back to bottom to resume auto-scroll
 * - Uses 'instant' scroll during streaming for smooth token updates
 * - Uses 'smooth' scroll for non-streaming updates (new messages)
 * - Ignores programmatic scrolls when determining user intent
 */
export function useAutoScroll(options: UseAutoScrollOptions = {}): UseAutoScrollReturn {
  const { threshold = 100, throttleMs = 100, isStreaming = false } = options;

  const containerRef = useRef<HTMLDivElement | null>(null);
  const [userHasScrolledUp, setUserHasScrolledUp] = useState(false);
  const isScrollingProgrammaticallyRef = useRef(false);
  // Track streaming state in a ref so ResizeObserver callback has access to current value
  const isStreamingRef = useRef(isStreaming);
  isStreamingRef.current = isStreaming;

  // Helper to check if currently at bottom
  const checkIfAtBottom = useCallback(() => {
    const container = containerRef.current;
    if (!container) return true; // Assume at bottom if no container

    const distanceFromBottom =
      container.scrollHeight - container.scrollTop - container.clientHeight;
    return distanceFromBottom < threshold;
  }, [threshold]);

  const handleScrollRaw = useCallback(() => {
    // Ignore scroll events triggered by programmatic scrolling
    if (isScrollingProgrammaticallyRef.current) return;

    setUserHasScrolledUp(!checkIfAtBottom());
  }, [checkIfAtBottom]);

  // Throttle scroll handler to reduce frequency of state updates
  const handleScroll = useThrottledCallback(handleScrollRaw, throttleMs);

  const scrollToBottom = useCallback((isStreaming: boolean) => {
    const container = containerRef.current;
    if (!container) return;

    // Use requestAnimationFrame to ensure measurements are complete
    requestAnimationFrame(() => {
      isScrollingProgrammaticallyRef.current = true;

      container.scrollTo({
        top: container.scrollHeight,
        // Use instant during streaming for smooth token updates
        // Use smooth for non-streaming (e.g., after sending a message)
        behavior: isStreaming ? "instant" : "smooth",
      });

      // We're scrolling to bottom, so explicitly mark as not scrolled up
      // This fixes the issue where the button wouldn't disappear after clicking
      setUserHasScrolledUp(false);

      // Reset the flag after scroll completes
      // For instant scrolls, we can reset immediately
      // For smooth scrolls, wait for animation to complete
      const resetDelay = isStreaming ? 0 : 300;
      setTimeout(() => {
        isScrollingProgrammaticallyRef.current = false;
      }, resetDelay);
    });
  }, []);

  // Check scroll position on mount and when container size changes
  // This handles the case where user navigates to a long conversation
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    // Check initial position after a short delay to allow content to render
    const checkInitialPosition = () => {
      if (!isScrollingProgrammaticallyRef.current) {
        setUserHasScrolledUp(!checkIfAtBottom());
      }
    };

    // Use requestAnimationFrame to ensure layout is complete
    requestAnimationFrame(checkInitialPosition);

    // Also check when container resizes (content loaded)
    // Skip during streaming - content height changes constantly during streaming,
    // which would fight with user's scroll intent (resetting userHasScrolledUp
    // to false when user is near bottom, causing unwanted scroll-to-bottom)
    const resizeObserver = new ResizeObserver(() => {
      if (!isScrollingProgrammaticallyRef.current && !isStreamingRef.current) {
        setUserHasScrolledUp(!checkIfAtBottom());
      }
    });
    resizeObserver.observe(container);

    return () => resizeObserver.disconnect();
  }, [checkIfAtBottom]);

  return {
    containerRef,
    userHasScrolledUp,
    handleScroll,
    scrollToBottom,
  };
}

/**
 * Effect hook that triggers auto-scroll when dependencies change.
 * Should be used alongside useAutoScroll.
 *
 * ## Why This Exists
 *
 * Separating the effect from the hook allows flexible configuration of
 * when to trigger scrolls. Common triggers include:
 * - `messages.length` - New message added
 * - `modelResponses` - Streaming content updated
 *
 * ## Behavior
 *
 * - If `userHasScrolledUp` is true, does nothing (respects user intent)
 * - If `isStreaming`, uses instant scroll (no animation lag)
 * - Otherwise, uses smooth scroll (pleasant UX)
 *
 * @param scrollToBottom - The scrollToBottom function from useAutoScroll
 * @param userHasScrolledUp - Whether user has scrolled up
 * @param isStreaming - Whether content is currently streaming
 * @param deps - Dependencies that should trigger scroll (e.g., message count)
 */
export function useAutoScrollEffect(
  scrollToBottom: (isStreaming: boolean) => void,
  userHasScrolledUp: boolean,
  isStreaming: boolean,
  deps: unknown[]
): void {
  useEffect(() => {
    if (userHasScrolledUp) return;
    scrollToBottom(isStreaming);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [scrollToBottom, userHasScrolledUp, isStreaming, ...deps]);
}
