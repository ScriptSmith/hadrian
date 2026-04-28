import { useRef, useEffect } from "react";
import { Streamdown, type MermaidOptions } from "streamdown";
import { createCodePlugin } from "@streamdown/code";
import { math } from "@streamdown/math";
import { mermaid } from "@streamdown/mermaid";

import { cn } from "@/utils/cn";
import { usePreferences } from "@/preferences/PreferencesProvider";
import { loadKatexCss } from "@/utils/katexCss";
import { linkSafety } from "./linkSafety";

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

  // Lazy-load the KaTeX stylesheet on first mount so it doesn't bloat the
  // initial bundle on pages that never render markdown.
  useEffect(() => {
    void loadKatexCss();
  }, []);

  // Streamdown renders <pre> elements that we can't control directly.
  // Post-render fixup: set tabIndex="0" on all <pre> children so keyboard
  // users can scroll them (fixes axe-core scrollable-region-focusable).
  //
  // Use a MutationObserver instead of re-querying on every token: streaming
  // content changes hundreds of times per response, and `querySelectorAll`
  // walks the entire markdown subtree each call. The observer only fires
  // when the DOM actually changes, and we only need to attribute newly
  // mounted <pre> nodes.
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    const tagPre = (node: Element) => {
      if (node.tagName === "PRE" && !node.hasAttribute("tabindex")) {
        node.setAttribute("tabindex", "0");
      }
      for (const pre of node.querySelectorAll("pre")) {
        if (!pre.hasAttribute("tabindex")) {
          pre.setAttribute("tabindex", "0");
        }
      }
    };
    tagPre(container);

    const observer = new MutationObserver((records) => {
      for (const record of records) {
        for (const node of record.addedNodes) {
          if (node.nodeType === Node.ELEMENT_NODE) {
            tagPre(node as Element);
          }
        }
      }
    });
    observer.observe(container, { childList: true, subtree: true });
    return () => observer.disconnect();
  }, []);

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
        "max-w-none",
        "[&_pre]:overflow-x-auto",
        className
      )}
    >
      <Streamdown
        plugins={{ code, math, mermaid }}
        mermaid={mermaidOptions}
        linkSafety={linkSafety}
      >
        {content}
      </Streamdown>
    </div>
  );
}
