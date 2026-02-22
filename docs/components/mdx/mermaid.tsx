"use client";

import { use, useEffect, useId, useState } from "react";
import { useTheme } from "next-themes";

// Use typeof window check to determine if we're on client
// This avoids the hydration mismatch while not triggering lint warnings
export function Mermaid({ chart }: { chart: string }) {
  // On server, window is undefined. On client after hydration, it's defined.
  // We use a state to force a re-render after hydration.
  const [isClient, setIsClient] = useState(false);

  useEffect(() => {
    // This only runs on client after hydration
    // eslint-disable-next-line react-hooks/set-state-in-effect -- Intentional for hydration
    setIsClient(true);
  }, []);

  // During SSR and initial hydration, return a placeholder
  if (!isClient) {
    return <div className="mermaid-placeholder" style={{ minHeight: "200px" }} />;
  }

  return <MermaidContent chart={chart} />;
}

type MermaidRenderResult = {
  svg: string;
  bindFunctions?: (el: HTMLElement) => void;
};

// Cache for mermaid render results
const cache = new Map<string, Promise<MermaidRenderResult>>();

// Cache for mermaid module
let mermaidModulePromise: Promise<typeof import("mermaid")> | null = null;

function getCachedRender(
  key: string,
  render: () => Promise<MermaidRenderResult>
): Promise<MermaidRenderResult> {
  const cached = cache.get(key);
  if (cached) return cached;

  const promise = render();
  cache.set(key, promise);
  return promise;
}

function MermaidContent({ chart }: { chart: string }) {
  // Normalize chart content FIRST
  const normalizedChart = chart.replaceAll("\\n", "\n");
  const id = useId();
  const { resolvedTheme } = useTheme();

  const [mermaid, setMermaid] = useState<typeof import("mermaid") | null>(null);

  // Load mermaid module once
  useEffect(() => {
    let mounted = true;

    if (!mermaidModulePromise) {
      mermaidModulePromise = import("mermaid");
    }

    mermaidModulePromise.then((mod) => {
      if (mounted) setMermaid(mod);
    });

    return () => {
      mounted = false;
    };
  }, []);

  // Initialize mermaid once when module loads
  useEffect(() => {
    if (!mermaid) return;

    mermaid.default.initialize({
      startOnLoad: false,
      securityLevel: "loose",
      fontFamily: "inherit",
      themeCSS: "margin: 1.5rem auto 0;",
      theme: resolvedTheme === "dark" ? "dark" : "default",
    });
  }, [mermaid, resolvedTheme]);

  if (!mermaid) return <div>Loading...</div>;

  // Include ID in cache key to prevent collisions
  const cacheKey = `${id}-${normalizedChart}-${resolvedTheme}`;

  const { svg, bindFunctions } = use(
    getCachedRender(cacheKey, () => {
      return mermaid.default.render(id, normalizedChart);
    })
  );

  return (
    <div
      ref={(container) => {
        if (container) bindFunctions?.(container);
      }}
      dangerouslySetInnerHTML={{ __html: svg }}
    />
  );
}
