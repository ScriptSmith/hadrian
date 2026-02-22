import { useState, useCallback, useEffect, useRef } from "react";

export interface UseResizableOptions {
  /** Initial width in pixels */
  initialWidth: number;
  /** Minimum width in pixels */
  minWidth: number;
  /** Maximum width in pixels */
  maxWidth: number;
  /** Default width to reset to on double-click (defaults to initialWidth if not specified) */
  defaultWidth?: number;
  /** Called when width changes during drag */
  onResize?: (width: number) => void;
  /** Called when drag ends with final width */
  onResizeEnd?: (width: number) => void;
  /** Direction of resize: 'left' means handle is on left edge, 'right' means handle is on right edge */
  direction?: "left" | "right";
}

export interface UseResizableReturn {
  /** Current width */
  width: number;
  /** Whether currently dragging */
  isDragging: boolean;
  /** Props to spread on the resize handle element */
  handleProps: {
    onMouseDown: (e: React.MouseEvent) => void;
    onTouchStart: (e: React.TouchEvent) => void;
    onDoubleClick: () => void;
    style: React.CSSProperties;
  };
  /** Reset width to initial value */
  resetWidth: () => void;
  /** Set width programmatically */
  setWidth: (width: number) => void;
}

export function useResizable({
  initialWidth,
  minWidth,
  maxWidth,
  defaultWidth,
  onResize,
  onResizeEnd,
  direction = "right",
}: UseResizableOptions): UseResizableReturn {
  const resetToWidth = defaultWidth ?? initialWidth;
  const [width, setWidthState] = useState(initialWidth);
  const [isDragging, setIsDragging] = useState(false);
  const startXRef = useRef(0);
  const startWidthRef = useRef(0);

  const clampWidth = useCallback(
    (w: number) => Math.max(minWidth, Math.min(maxWidth, w)),
    [minWidth, maxWidth]
  );

  const setWidth = useCallback(
    (w: number) => {
      const clamped = clampWidth(w);
      setWidthState(clamped);
    },
    [clampWidth]
  );

  const resetWidth = useCallback(() => {
    setWidthState(resetToWidth);
    onResizeEnd?.(resetToWidth);
  }, [resetToWidth, onResizeEnd]);

  const handleMouseDown = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      startXRef.current = e.clientX;
      startWidthRef.current = width;
      setIsDragging(true);
    },
    [width]
  );

  const handleTouchStart = useCallback(
    (e: React.TouchEvent) => {
      const touch = e.touches[0];
      startXRef.current = touch.clientX;
      startWidthRef.current = width;
      setIsDragging(true);
    },
    [width]
  );

  useEffect(() => {
    if (!isDragging) return;

    const handleMouseMove = (e: MouseEvent) => {
      const delta = e.clientX - startXRef.current;
      const newWidth =
        direction === "right" ? startWidthRef.current + delta : startWidthRef.current - delta;
      const clamped = clampWidth(newWidth);
      setWidthState(clamped);
      onResize?.(clamped);
    };

    const handleTouchMove = (e: TouchEvent) => {
      const touch = e.touches[0];
      const delta = touch.clientX - startXRef.current;
      const newWidth =
        direction === "right" ? startWidthRef.current + delta : startWidthRef.current - delta;
      const clamped = clampWidth(newWidth);
      setWidthState(clamped);
      onResize?.(clamped);
    };

    const handleEnd = () => {
      setIsDragging(false);
      onResizeEnd?.(width);
    };

    document.addEventListener("mousemove", handleMouseMove);
    document.addEventListener("mouseup", handleEnd);
    document.addEventListener("touchmove", handleTouchMove);
    document.addEventListener("touchend", handleEnd);

    // Prevent text selection while dragging
    document.body.style.userSelect = "none";
    document.body.style.cursor = "col-resize";

    return () => {
      document.removeEventListener("mousemove", handleMouseMove);
      document.removeEventListener("mouseup", handleEnd);
      document.removeEventListener("touchmove", handleTouchMove);
      document.removeEventListener("touchend", handleEnd);
      document.body.style.userSelect = "";
      document.body.style.cursor = "";
    };
  }, [isDragging, width, clampWidth, direction, onResize, onResizeEnd]);

  // Sync with external width changes (e.g., from preferences)
  useEffect(() => {
    if (!isDragging && initialWidth !== width) {
      setWidthState(initialWidth);
    }
    // Only sync when initialWidth changes externally, not during drag
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [initialWidth]);

  return {
    width,
    isDragging,
    handleProps: {
      onMouseDown: handleMouseDown,
      onTouchStart: handleTouchStart,
      onDoubleClick: resetWidth,
      style: { cursor: "col-resize" },
    },
    resetWidth,
    setWidth,
  };
}
