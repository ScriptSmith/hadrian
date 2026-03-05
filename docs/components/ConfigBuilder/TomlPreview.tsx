"use client";

import { Check, Copy, Download } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { type Highlighter, createHighlighter } from "shiki";
import { useTheme } from "next-themes";

let highlighterPromise: Promise<Highlighter> | null = null;

function getHighlighter() {
  if (!highlighterPromise) {
    highlighterPromise = createHighlighter({
      themes: ["github-light", "github-dark"],
      langs: ["toml"],
    });
  }
  return highlighterPromise;
}

export function TomlPreview({ toml }: { toml: string }) {
  const [html, setHtml] = useState("");
  const [copied, setCopied] = useState(false);
  const debounceRef = useRef<ReturnType<typeof setTimeout>>(undefined);
  const { resolvedTheme } = useTheme();
  const theme = resolvedTheme === "dark" ? "github-dark" : "github-light";

  useEffect(() => {
    clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(() => {
      getHighlighter().then((hl) => {
        setHtml(hl.codeToHtml(toml, { lang: "toml", theme }));
      });
    }, 150);
    return () => clearTimeout(debounceRef.current);
  }, [toml, theme]);

  const handleCopy = useCallback(async () => {
    await navigator.clipboard.writeText(toml);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  }, [toml]);

  const handleDownload = useCallback(() => {
    const blob = new Blob([toml], { type: "application/toml" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = "hadrian.toml";
    a.click();
    URL.revokeObjectURL(url);
  }, [toml]);

  return (
    <div className="flex h-full flex-col overflow-hidden rounded-lg border border-fd-border bg-fd-card">
      <div className="flex items-center justify-between border-b border-fd-border bg-fd-muted/50 px-4 py-2">
        <span className="text-sm font-medium text-fd-foreground">hadrian.toml</span>
        <div className="flex gap-1">
          <button
            type="button"
            onClick={handleCopy}
            className="rounded-md p-1.5 text-fd-muted-foreground transition-colors hover:bg-fd-muted hover:text-fd-foreground"
            aria-label="Copy TOML"
          >
            {copied ? <Check className="h-4 w-4 text-green-500" /> : <Copy className="h-4 w-4" />}
          </button>
          <button
            type="button"
            onClick={handleDownload}
            className="rounded-md p-1.5 text-fd-muted-foreground transition-colors hover:bg-fd-muted hover:text-fd-foreground"
            aria-label="Download hadrian.toml"
          >
            <Download className="h-4 w-4" />
          </button>
        </div>
      </div>
      {html ? (
        <div
          className="flex-1 overflow-auto p-4 text-sm [&_pre]:!m-0 [&_pre]:!bg-transparent [&_pre]:!p-0"
          tabIndex={0}
          aria-label="TOML preview"
          dangerouslySetInnerHTML={{ __html: html }}
        />
      ) : (
        <div className="flex-1 overflow-auto p-4 text-sm" tabIndex={0} aria-label="TOML preview">
          <pre className="text-fd-muted-foreground">
            <code>{toml}</code>
          </pre>
        </div>
      )}
    </div>
  );
}
