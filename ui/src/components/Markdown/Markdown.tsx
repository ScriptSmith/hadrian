import { useRef, useEffect } from "react";
import { Streamdown, type MermaidOptions } from "streamdown";
import { createCodePlugin } from "@streamdown/code";

import { cn } from "@/utils/cn";
import { usePreferences } from "@/preferences/PreferencesProvider";

const lightCode = createCodePlugin({
  themes: ["github-light-high-contrast", "github-light-high-contrast"],
});
const darkCode = createCodePlugin({ themes: ["github-dark", "github-dark"] });

interface MarkdownProps {
  content: string;
  className?: string;
}

export function Markdown({ content, className }: MarkdownProps) {
  const { resolvedTheme } = usePreferences();
  const containerRef = useRef<HTMLDivElement>(null);

  // Streamdown renders <pre> elements that we can't control directly.
  // Post-render fixup: set tabIndex="0" on all <pre> children so keyboard
  // users can scroll them (fixes axe-core scrollable-region-focusable).
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    for (const pre of container.querySelectorAll("pre")) {
      if (!pre.hasAttribute("tabindex")) {
        pre.setAttribute("tabindex", "0");
      }
    }
  }, [content]);

  const mermaidOptions: MermaidOptions = {
    config: {
      theme: resolvedTheme === "dark" ? "dark" : "default",
    },
  };

  const code = resolvedTheme === "dark" ? darkCode : lightCode;

  return (
    <div
      ref={containerRef}
      className={cn(
        "markdown-content prose prose-sm dark:prose-invert",
        "max-w-[calc(100vw-8rem)] sm:max-w-[500px] md:max-w-[600px] lg:max-w-[700px]",
        "[&_pre]:overflow-x-auto",
        className
      )}
    >
      <Streamdown plugins={{ code }} mermaid={mermaidOptions}>
        {content}
      </Streamdown>
    </div>
  );
}
