import { memo, useState, useEffect, useCallback } from "react";
import { createHighlighter, type Highlighter } from "shiki";
import { Copy, Check } from "lucide-react";

import { Button } from "@/components/Button/Button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/Tooltip/Tooltip";
import { usePreferences } from "@/preferences/PreferencesProvider";
import { cn } from "@/utils/cn";

const THEMES = ["github-light-high-contrast", "github-dark-high-contrast"] as const;
const PRELOADED_LANGS = [
  "python",
  "javascript",
  "typescript",
  "sql",
  "json",
  "html",
  "css",
  "bash",
] as const;

let highlighterPromise: Promise<Highlighter> | null = null;

function getHighlighter(): Promise<Highlighter> {
  if (!highlighterPromise) {
    highlighterPromise = createHighlighter({
      themes: [...THEMES],
      langs: [...PRELOADED_LANGS],
    });
  }
  return highlighterPromise;
}

// Bounded LRU-ish cache so toggling themes back and forth on the same blocks
// doesn't trigger a re-highlight every time. Keyed on (theme, lang, code).
const HIGHLIGHT_CACHE_LIMIT = 256;
const highlightCache = new Map<string, string>();

function cacheKey(theme: string, lang: string, code: string): string {
  return `${theme}|${lang}|${code}`;
}

function readHighlightCache(key: string): string | undefined {
  const cached = highlightCache.get(key);
  if (cached !== undefined) {
    // Move to most-recent slot
    highlightCache.delete(key);
    highlightCache.set(key, cached);
  }
  return cached;
}

function writeHighlightCache(key: string, value: string): void {
  if (highlightCache.size >= HIGHLIGHT_CACHE_LIMIT) {
    const oldest = highlightCache.keys().next().value;
    if (oldest !== undefined) highlightCache.delete(oldest);
  }
  highlightCache.set(key, value);
}

export interface HighlightedCodeProps {
  code: string;
  language?: string;
  className?: string;
  /** Show copy button (default: true) */
  showCopy?: boolean;
  /** Show language badge (default: false) */
  showLanguage?: boolean;
  /** Max height with overflow scroll */
  maxHeight?: string;
  /** Compact mode — smaller font and padding for inline previews */
  compact?: boolean;
}

function HighlightedCodeComponent({
  code,
  language,
  className,
  showCopy = true,
  showLanguage = false,
  maxHeight,
  compact = false,
}: HighlightedCodeProps) {
  const { resolvedTheme } = usePreferences();
  const [html, setHtml] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  const theme =
    resolvedTheme === "dark" ? "github-dark-high-contrast" : "github-light-high-contrast";

  useEffect(() => {
    let cancelled = false;

    const lang = (language?.toLowerCase() ?? "text") || "text";
    const key = cacheKey(theme, lang, code);
    const cached = readHighlightCache(key);
    if (cached !== undefined) {
      setHtml(cached);
      return;
    }

    getHighlighter().then((highlighter) => {
      if (cancelled) return;

      const loadedLangs = highlighter.getLoadedLanguages();

      // Use plain text for unknown languages
      const effectiveLang = loadedLangs.includes(lang) ? lang : "text";

      const result = highlighter.codeToHtml(code, {
        lang: effectiveLang,
        theme,
      });
      writeHighlightCache(key, result);
      setHtml(result);
    });

    return () => {
      cancelled = true;
    };
  }, [code, language, theme]);

  const handleCopy = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(code);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (error) {
      console.error("Failed to copy to clipboard:", error);
    }
  }, [code]);

  return (
    <div className={cn("relative group", className)}>
      {showCopy && (
        <div
          className={cn(
            "absolute opacity-0 group-hover:opacity-100 transition-opacity z-10",
            compact ? "right-1 top-1" : "right-2 top-2"
          )}
        >
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="secondary"
                size="sm"
                className={compact ? "h-5 w-5 p-0" : "h-7 w-7 p-0"}
                onClick={handleCopy}
                aria-label={copied ? "Copied" : "Copy code"}
              >
                {copied ? (
                  <Check className={compact ? "h-3 w-3" : "h-3.5 w-3.5"} />
                ) : (
                  <Copy className={compact ? "h-3 w-3" : "h-3.5 w-3.5"} />
                )}
              </Button>
            </TooltipTrigger>
            <TooltipContent>{copied ? "Copied!" : "Copy code"}</TooltipContent>
          </Tooltip>
        </div>
      )}

      {showLanguage && language && (
        <div className="absolute left-2 top-2 z-10">
          <span className="text-[10px] font-mono text-muted-foreground bg-muted/80 px-1.5 py-0.5 rounded">
            {language}
          </span>
        </div>
      )}

      {/* eslint-disable jsx-a11y/no-noninteractive-tabindex -- scrollable region needs keyboard access (axe: scrollable-region-focusable) */}
      {html ? (
        <div
          tabIndex={0}
          className={cn(
            "overflow-x-auto [&_pre]:!m-0 [&_pre]:overflow-x-auto [&_code]:!font-mono focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-primary",
            compact
              ? "[&_pre]:!px-2.5 [&_pre]:!py-1.5 [&_pre]:!text-[11px] [&_pre]:!leading-relaxed"
              : "[&_pre]:!p-4 [&_pre]:!text-sm",
            showLanguage && language && !compact && "[&_pre]:!pt-8",
            maxHeight && "overflow-y-auto"
          )}
          style={maxHeight ? { maxHeight } : undefined}
          dangerouslySetInnerHTML={{ __html: html }}
        />
      ) : (
        <pre
          tabIndex={0}
          className={cn(
            "overflow-x-auto bg-muted/50 text-foreground font-mono focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-primary",
            compact ? "px-2.5 py-1.5 text-[11px] leading-relaxed" : "p-4 text-sm",
            showLanguage && language && !compact && "pt-8",
            maxHeight && "overflow-y-auto"
          )}
          style={maxHeight ? { maxHeight } : undefined}
        >
          <code className="whitespace-pre">{code}</code>
        </pre>
      )}
      {/* eslint-enable jsx-a11y/no-noninteractive-tabindex */}
    </div>
  );
}

export const HighlightedCode = memo(HighlightedCodeComponent);
