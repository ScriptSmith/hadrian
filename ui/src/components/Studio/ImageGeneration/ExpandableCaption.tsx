import { useState, useRef, useEffect, useCallback } from "react";
import { cn } from "@/utils/cn";

interface ExpandableCaptionProps {
  text: string;
  /** Maximum number of visible lines before clamping (default: 2) */
  maxLines?: number;
  className?: string;
}

export function ExpandableCaption({ text, maxLines = 2, className }: ExpandableCaptionProps) {
  const [expanded, setExpanded] = useState(false);
  const [isClamped, setIsClamped] = useState(false);
  const textRef = useRef<HTMLParagraphElement>(null);

  const checkClamped = useCallback(() => {
    const el = textRef.current;
    if (el) {
      setIsClamped(el.scrollHeight > el.clientHeight + 1);
    }
  }, []);

  useEffect(() => {
    checkClamped();
    // Re-check on resize
    const observer = new ResizeObserver(checkClamped);
    if (textRef.current) observer.observe(textRef.current);
    return () => observer.disconnect();
  }, [text, checkClamped]);

  const lineClampClass =
    maxLines === 2 ? "line-clamp-2" : maxLines === 3 ? "line-clamp-3" : "line-clamp-2";

  return (
    <div className={cn("text-xs text-muted-foreground", className)}>
      <p ref={textRef} className={cn(!expanded && lineClampClass)}>
        {text}
      </p>
      {(isClamped || expanded) && (
        <button
          type="button"
          onClick={() => setExpanded(!expanded)}
          className="mt-0.5 text-xs font-medium text-primary hover:underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
        >
          {expanded ? "Show less" : "Show more"}
        </button>
      )}
    </div>
  );
}
