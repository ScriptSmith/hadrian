import { useRef, useMemo, useEffect } from "react";

/**
 * Returns a throttled version of the callback that executes at most once
 * every `delay` milliseconds. Uses trailing edge execution (executes at the
 * end of the throttle window if called during the window).
 *
 * @param callback - The function to throttle
 * @param delay - Minimum milliseconds between executions
 * @returns A throttled version of the callback
 */
export function useThrottledCallback<T extends (...args: never[]) => void>(
  callback: T,
  delay: number
): T {
  const lastExecutedRef = useRef<number>(0);
  const timeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const callbackRef = useRef<T>(callback);

  // Keep callback ref up to date
  useEffect(() => {
    callbackRef.current = callback;
  }, [callback]);

  // Clean up timeout on unmount
  useEffect(() => {
    return () => {
      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current);
      }
    };
  }, []);

  return useMemo(() => {
    const throttled = (...args: Parameters<T>) => {
      const now = Date.now();
      const timeSinceLastExecution = now - lastExecutedRef.current;

      if (timeSinceLastExecution >= delay) {
        // Execute immediately if enough time has passed
        lastExecutedRef.current = now;
        callbackRef.current(...args);
      } else if (!timeoutRef.current) {
        // Schedule trailing execution only if not already scheduled
        const remainingTime = delay - timeSinceLastExecution;
        timeoutRef.current = setTimeout(() => {
          lastExecutedRef.current = Date.now();
          timeoutRef.current = null;
          callbackRef.current(...args);
        }, remainingTime);
      }
    };
    return throttled as T;
  }, [delay]);
}
