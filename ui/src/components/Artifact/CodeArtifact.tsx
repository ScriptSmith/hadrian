/**
 * CodeArtifact - Syntax-Highlighted Code Output
 *
 * Renders code output from tools like code_interpreter with syntax highlighting.
 * Uses a simple pre/code block approach that works with the prose styling.
 */

import { memo, useState } from "react";
import { Copy, Check } from "lucide-react";

import type { Artifact, CodeArtifactData } from "@/components/chat-types";
import { Button } from "@/components/Button/Button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/Tooltip/Tooltip";
import { cn } from "@/utils/cn";

export interface CodeArtifactProps {
  artifact: Artifact;
  className?: string;
}

function isCodeArtifactData(data: unknown): data is CodeArtifactData {
  return (
    typeof data === "object" &&
    data !== null &&
    "code" in data &&
    typeof (data as CodeArtifactData).code === "string"
  );
}

function CodeArtifactComponent({ artifact, className }: CodeArtifactProps) {
  const [copied, setCopied] = useState(false);

  // Validate and extract data
  if (!isCodeArtifactData(artifact.data)) {
    return <div className="p-4 text-sm text-muted-foreground">Invalid code artifact data</div>;
  }

  const { code, language } = artifact.data;

  const handleCopy = async () => {
    await navigator.clipboard.writeText(code);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className={cn("relative group", className)}>
      {/* Copy button */}
      <div className="absolute right-2 top-2 opacity-0 group-hover:opacity-100 transition-opacity z-10">
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              variant="secondary"
              size="sm"
              className="h-7 w-7 p-0"
              onClick={handleCopy}
              aria-label={copied ? "Copied" : "Copy code"}
            >
              {copied ? <Check className="h-3.5 w-3.5" /> : <Copy className="h-3.5 w-3.5" />}
            </Button>
          </TooltipTrigger>
          <TooltipContent>{copied ? "Copied!" : "Copy code"}</TooltipContent>
        </Tooltip>
      </div>

      {/* Language badge */}
      {language && (
        <div className="absolute left-2 top-2 z-10">
          <span className="text-[10px] font-mono text-muted-foreground bg-muted/80 px-1.5 py-0.5 rounded">
            {language}
          </span>
        </div>
      )}

      {/* Code block */}
      {/* eslint-disable jsx-a11y/no-noninteractive-tabindex -- scrollable region needs keyboard access (axe: scrollable-region-focusable) */}
      <pre
        tabIndex={0}
        className={cn(
          "p-4 pt-8 overflow-x-auto text-sm font-mono",
          "bg-muted/50 text-foreground",
          "max-h-[400px] overflow-y-auto"
        )}
      >
        <code className="whitespace-pre">{code}</code>
      </pre>
      {/* eslint-enable jsx-a11y/no-noninteractive-tabindex */}
    </div>
  );
}

export const CodeArtifact = memo(CodeArtifactComponent);
