/**
 * CodeArtifact - Syntax-Highlighted Code Output
 *
 * Renders code output from tools like code_interpreter with syntax highlighting
 * via Shiki. Uses the shared HighlightedCode component.
 */

import { memo } from "react";

import type { Artifact, CodeArtifactData } from "@/components/chat-types";
import { HighlightedCode } from "@/components/HighlightedCode/HighlightedCode";
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
  if (!isCodeArtifactData(artifact.data)) {
    return <div className="p-4 text-sm text-muted-foreground">Invalid code artifact data</div>;
  }

  const { code, language } = artifact.data;

  return (
    <HighlightedCode
      code={code}
      language={language}
      showCopy
      showLanguage
      maxHeight="400px"
      className={cn("rounded-md overflow-hidden", className)}
    />
  );
}

export const CodeArtifact = memo(CodeArtifactComponent);
