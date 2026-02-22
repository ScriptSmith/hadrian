import { Streamdown, type MermaidOptions } from "streamdown";
import { memo } from "react";

import { cn } from "@/utils/cn";
import { usePreferences } from "@/preferences/PreferencesProvider";

/**
 * StreamingMarkdown - Optimized Markdown Rendering for Token Streaming
 *
 * ## Why This Exists
 *
 * Standard markdown renderers re-parse the entire document on every change.
 * During streaming (50-100+ tokens/second), this causes:
 * - Visual stuttering as the parser catches up
 * - Unnecessary CPU usage from repeated parsing
 * - Flickering of syntax-highlighted code blocks
 *
 * ## How Streamdown Solves This
 *
 * The `streamdown` library provides incremental markdown parsing:
 * - Tracks which parts of the document have changed
 * - Only re-renders affected blocks
 * - Handles incomplete markdown gracefully (e.g., unterminated code blocks)
 *
 * ## Mode Switching
 *
 * - `mode="streaming"` + `isAnimating={true}`: Optimized for frequent updates
 *   - Disables interactive features (copy buttons) until stable
 *   - Uses incremental parsing
 *
 * - `mode="static"`: Full rendering with all features
 *   - Used after streaming completes
 *   - Enables code block copy buttons, etc.
 *
 * ## Performance Note
 *
 * This component is wrapped in React.memo. Combined with Streamdown's internal
 * optimizations, only the minimum necessary DOM updates occur on each token.
 */

interface StreamingMarkdownProps {
  content: string;
  isStreaming: boolean;
  className?: string;
}

/**
 * StreamingMarkdown is optimized for rendering markdown during active streaming.
 *
 * It differs from the standard Markdown component by:
 * 1. Using `mode="streaming"` and `isAnimating={true}` when streaming is active
 * 2. Enabling `parseIncompleteMarkdown` to handle unterminated markdown blocks
 *
 * This allows Streamdown to optimize its rendering for high-frequency updates
 * during token streaming, such as disabling copy buttons until streaming completes.
 */
function StreamingMarkdownComponent({ content, isStreaming, className }: StreamingMarkdownProps) {
  const { resolvedTheme } = usePreferences();

  const mermaidOptions: MermaidOptions = {
    config: {
      theme: resolvedTheme === "dark" ? "dark" : "default",
    },
  };

  return (
    <div
      className={cn(
        "markdown-content prose prose-sm dark:prose-invert",
        "max-w-[calc(100vw-8rem)] sm:max-w-[500px] md:max-w-[600px] lg:max-w-[700px]",
        "[&_pre]:overflow-x-auto",
        className
      )}
    >
      <Streamdown
        mermaid={mermaidOptions}
        mode={isStreaming ? "streaming" : "static"}
        isAnimating={isStreaming}
        parseIncompleteMarkdown={isStreaming}
      >
        {content}
      </Streamdown>
      {isStreaming && (
        <span className="inline-block h-4 w-0.5 animate-blink rounded-full bg-primary" />
      )}
    </div>
  );
}

export const StreamingMarkdown = memo(StreamingMarkdownComponent);
