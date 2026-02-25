"use client";

import { useEffect, useState } from "react";

interface StoryEmbedProps {
  /** Story ID in format "category-component--story", e.g. "ui-button--primary" */
  storyId: string;
  /** Height of the iframe */
  height?: number | string;
  /** Optional title for accessibility */
  title?: string;
}

/**
 * Embeds a Storybook story in an iframe.
 * Stories are served from /storybook/ (symlinked to ui/storybook-static).
 */
export function StoryEmbed({ storyId, height = 200, title }: StoryEmbedProps) {
  const [theme, setTheme] = useState<"light" | "dark">("light");

  // Sync with Fumadocs theme
  useEffect(() => {
    const root = document.documentElement;
    const updateTheme = () => {
      setTheme(root.classList.contains("dark") ? "dark" : "light");
    };

    updateTheme();

    const observer = new MutationObserver(updateTheme);
    observer.observe(root, { attributes: true, attributeFilter: ["class"] });

    return () => observer.disconnect();
  }, []);

  const basePath = process.env.DOCS_BASE_PATH || "";
  const src = `${basePath}/storybook/iframe.html?id=${storyId}&viewMode=story&globals=theme:${theme}`;

  return (
    <iframe
      src={src}
      title={title || `Storybook: ${storyId}`}
      style={{
        width: "100%",
        height: typeof height === "number" ? `${height}px` : height,
        border: "1px solid var(--fd-border)",
        borderRadius: "8px",
        background: theme === "dark" ? "#09090b" : "#fafafa",
      }}
      loading="lazy"
    />
  );
}
