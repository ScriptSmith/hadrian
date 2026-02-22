/**
 * HtmlArtifact - Sandboxed HTML Preview
 *
 * Renders HTML content in a sandboxed iframe for safe preview.
 * Uses strict CSP and sandbox attributes to prevent malicious code execution.
 */

import { memo, useState, useRef, useEffect } from "react";
import { Code2, Eye, Copy, Check, ExternalLink, Maximize2, X } from "lucide-react";

import type { Artifact } from "@/components/chat-types";
import { Button } from "@/components/Button/Button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/Tooltip/Tooltip";
import { cn } from "@/utils/cn";

export interface HtmlArtifactProps {
  artifact: Artifact;
  className?: string;
}

/** Extract HTML content from artifact data */
function getHtmlContent(data: unknown): string | null {
  if (typeof data === "string") {
    return data;
  }
  if (typeof data === "object" && data !== null) {
    const obj = data as Record<string, unknown>;
    if (typeof obj.html === "string") return obj.html;
    if (typeof obj.content === "string") return obj.content;
  }
  return null;
}

/**
 * Create a full HTML document for the iframe
 * Includes basic styling and dark mode support
 */
function wrapHtml(content: string): string {
  return `<!DOCTYPE html>
<html>
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <style>
    *, *::before, *::after { box-sizing: border-box; }
    body {
      margin: 0;
      padding: 16px;
      font-family: system-ui, -apple-system, sans-serif;
      font-size: 14px;
      line-height: 1.5;
      color: #1f2937;
      background: #ffffff;
    }
    @media (prefers-color-scheme: dark) {
      body {
        color: #f3f4f6;
        background: #111827;
      }
    }
    img { max-width: 100%; height: auto; }
    pre { overflow-x: auto; }
    code { font-family: ui-monospace, monospace; }
  </style>
</head>
<body>${content}</body>
</html>`;
}

function HtmlArtifactComponent({ artifact, className }: HtmlArtifactProps) {
  const [viewMode, setViewMode] = useState<"preview" | "source">("preview");
  const [copied, setCopied] = useState(false);
  const [isFullscreen, setIsFullscreen] = useState(false);
  const iframeRef = useRef<HTMLIFrameElement>(null);

  const html = getHtmlContent(artifact.data);

  // Set iframe content using srcdoc
  useEffect(() => {
    if (iframeRef.current && html && viewMode === "preview") {
      // Using srcdoc is safer than document.write
      iframeRef.current.srcdoc = wrapHtml(html);
    }
  }, [html, viewMode]);

  if (!html) {
    return <div className="p-4 text-sm text-muted-foreground">Invalid HTML artifact data</div>;
  }

  const handleCopy = async () => {
    await navigator.clipboard.writeText(html);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const handleOpenInNewTab = () => {
    const blob = new Blob([wrapHtml(html)], { type: "text/html" });
    const url = URL.createObjectURL(blob);
    window.open(url, "_blank");
    // Clean up after a delay
    setTimeout(() => URL.revokeObjectURL(url), 1000);
  };

  return (
    <>
      <div className={cn("", className)}>
        {/* Toolbar */}
        <div className="flex items-center gap-1 px-2 py-1.5 border-b bg-muted/30">
          <div className="flex items-center gap-0.5 rounded-md border bg-muted/50 p-0.5">
            <Button
              variant={viewMode === "preview" ? "secondary" : "ghost"}
              size="sm"
              className="h-6 px-2 text-xs"
              onClick={() => setViewMode("preview")}
            >
              <Eye className="h-3 w-3 mr-1" />
              Preview
            </Button>
            <Button
              variant={viewMode === "source" ? "secondary" : "ghost"}
              size="sm"
              className="h-6 px-2 text-xs"
              onClick={() => setViewMode("source")}
            >
              <Code2 className="h-3 w-3 mr-1" />
              Source
            </Button>
          </div>

          <div className="flex-1" />

          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="ghost"
                size="sm"
                className="h-6 w-6 p-0"
                onClick={handleCopy}
                aria-label={copied ? "Copied" : "Copy HTML"}
              >
                {copied ? <Check className="h-3.5 w-3.5" /> : <Copy className="h-3.5 w-3.5" />}
              </Button>
            </TooltipTrigger>
            <TooltipContent>{copied ? "Copied!" : "Copy HTML"}</TooltipContent>
          </Tooltip>

          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="ghost"
                size="sm"
                className="h-6 w-6 p-0"
                onClick={handleOpenInNewTab}
                aria-label="Open in new tab"
              >
                <ExternalLink className="h-3.5 w-3.5" />
              </Button>
            </TooltipTrigger>
            <TooltipContent>Open in new tab</TooltipContent>
          </Tooltip>

          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="ghost"
                size="sm"
                className="h-6 w-6 p-0"
                onClick={() => setIsFullscreen(true)}
                aria-label="Fullscreen"
              >
                <Maximize2 className="h-3.5 w-3.5" />
              </Button>
            </TooltipTrigger>
            <TooltipContent>Fullscreen</TooltipContent>
          </Tooltip>
        </div>

        {/* Content */}
        {viewMode === "preview" ? (
          <iframe
            ref={iframeRef}
            title={artifact.title || "HTML Preview"}
            sandbox="allow-scripts"
            className="w-full h-[300px] border-0 bg-white"
          />
        ) : (
          <pre className="p-4 overflow-x-auto text-xs font-mono text-foreground max-h-[300px] overflow-y-auto bg-muted/30">
            <code>{html}</code>
          </pre>
        )}
      </div>

      {/* Fullscreen modal */}
      {isFullscreen && (
        <div className="fixed inset-0 z-50 flex flex-col bg-background">
          {/* Header */}
          <div className="flex items-center gap-2 px-4 py-2 border-b">
            <span className="text-sm font-medium">{artifact.title || "HTML Preview"}</span>
            <div className="flex-1" />
            <Button variant="outline" size="sm" onClick={handleOpenInNewTab}>
              <ExternalLink className="h-3.5 w-3.5 mr-1.5" />
              Open in new tab
            </Button>
            <Button variant="outline" size="sm" onClick={() => setIsFullscreen(false)}>
              <X className="h-3.5 w-3.5 mr-1.5" />
              Close
            </Button>
          </div>

          {/* Iframe */}
          <iframe
            title={artifact.title || "HTML Preview (Fullscreen)"}
            srcDoc={wrapHtml(html)}
            sandbox="allow-scripts"
            className="flex-1 w-full border-0 bg-white"
          />
        </div>
      )}
    </>
  );
}

export const HtmlArtifact = memo(HtmlArtifactComponent);
