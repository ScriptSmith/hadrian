import { useState, useMemo, useEffect, useCallback } from "react";
import { createPortal } from "react-dom";
import {
  X,
  Bug,
  ChevronDown,
  ChevronRight,
  Clock,
  CheckCircle,
  XCircle,
  Copy,
  Check,
  FileText,
} from "lucide-react";

import type { MessageDebugInfo, DebugRound } from "@/components/chat-types";

interface DebugModalProps {
  debugInfo: MessageDebugInfo;
  onClose: () => void;
}

/**
 * Modal for viewing debug information about message exchanges.
 * Shows logical rounds view with raw SSE events when captured.
 * Uses createPortal to render at document.body for proper layering.
 */
export function DebugModal({ debugInfo, onClose }: DebugModalProps) {
  const [expandedRounds, setExpandedRounds] = useState<Set<number>>(
    () => new Set(debugInfo.rounds.map((r) => r.round))
  );
  const [copiedSection, setCopiedSection] = useState<string | null>(null);

  const totalDuration = useMemo(() => {
    return debugInfo.rounds.reduce((sum, r) => {
      if (r.endTime && r.startTime) {
        return sum + (r.endTime - r.startTime);
      }
      return sum;
    }, 0);
  }, [debugInfo.rounds]);

  const toggleRound = (round: number) => {
    setExpandedRounds((prev) => {
      const next = new Set(prev);
      if (next.has(round)) {
        next.delete(round);
      } else {
        next.add(round);
      }
      return next;
    });
  };

  const copyToClipboard = useCallback(async (text: string, section: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopiedSection(section);
      setTimeout(() => setCopiedSection(null), 2000);
    } catch {
      console.error("Failed to copy to clipboard");
    }
  }, []);

  const generateMarkdown = useCallback(() => {
    const lines: string[] = [];

    // Header
    lines.push("# Debug Information");
    lines.push("");
    lines.push(`**Model:** ${debugInfo.model}`);
    lines.push(`**Debug ID:** \`${debugInfo.messageId}\``);
    lines.push(`**Status:** ${debugInfo.success ? "✅ Success" : "❌ Failed"}`);
    lines.push(`**Total Duration:** ${(totalDuration / 1000).toFixed(2)}s`);
    lines.push(`**Rounds:** ${debugInfo.rounds.length}`);
    if (debugInfo.error) {
      lines.push(`**Error:** ${debugInfo.error}`);
    }
    lines.push("");
    lines.push("---");
    lines.push("");

    // Each round
    for (const round of debugInfo.rounds) {
      const duration =
        round.endTime && round.startTime
          ? ((round.endTime - round.startTime) / 1000).toFixed(2)
          : "...";

      lines.push(`## Round ${round.round}`);
      lines.push("");
      lines.push(`**Duration:** ${duration}s`);
      lines.push("");

      // Input Items
      if (round.inputItems.length > 0) {
        lines.push("### Input Items");
        lines.push("");
        lines.push("```json");
        lines.push(JSON.stringify(round.inputItems, null, 2));
        lines.push("```");
        lines.push("");
      }

      // Request Body
      if (round.requestBody) {
        lines.push("### Request Body");
        lines.push("");
        lines.push("```json");
        lines.push(JSON.stringify(round.requestBody, null, 2));
        lines.push("```");
        lines.push("");
      }

      // Response Output
      if (round.responseOutput && round.responseOutput.length > 0) {
        lines.push("### Response Output");
        lines.push("");
        lines.push("```json");
        lines.push(JSON.stringify(round.responseOutput, null, 2));
        lines.push("```");
        lines.push("");
      }

      // Tool Calls
      if (round.toolCalls && round.toolCalls.length > 0) {
        lines.push("### Tool Calls");
        lines.push("");
        for (const tc of round.toolCalls) {
          lines.push(`#### ${tc.name} (\`${tc.id}\`)`);
          lines.push("");
          lines.push("```json");
          lines.push(JSON.stringify(tc.arguments, null, 2));
          lines.push("```");
          lines.push("");
        }
      }

      // Tool Results
      if (round.toolResults && round.toolResults.length > 0) {
        lines.push("### Tool Results");
        lines.push("");
        for (const tr of round.toolResults) {
          const statusIcon = tr.success ? "✅" : "❌";
          lines.push(`#### ${statusIcon} ${tr.toolName} (\`${tr.callId}\`)`);
          lines.push("");
          if (tr.error) {
            lines.push(`**Error:** ${tr.error}`);
            lines.push("");
          }
          if (tr.output) {
            lines.push("```");
            lines.push(
              tr.output.length > 2000 ? tr.output.slice(0, 2000) + "\n... (truncated)" : tr.output
            );
            lines.push("```");
            lines.push("");
          }
        }
      }

      // Continuation Items
      if (round.continuationItems && round.continuationItems.length > 0) {
        lines.push("### Continuation Items");
        lines.push("");
        lines.push("```json");
        lines.push(JSON.stringify(round.continuationItems, null, 2));
        lines.push("```");
        lines.push("");
      }

      // SSE Events
      if (round.sseEvents && round.sseEvents.length > 0) {
        lines.push("### Raw SSE Events");
        lines.push("");
        lines.push("<details>");
        lines.push("<summary>Show all events</summary>");
        lines.push("");
        lines.push("```json");
        lines.push(JSON.stringify(round.sseEvents, null, 2));
        lines.push("```");
        lines.push("");
        lines.push("</details>");
        lines.push("");
      }

      lines.push("---");
      lines.push("");
    }

    return lines.join("\n");
  }, [debugInfo, totalDuration]);

  const handleCopyAll = useCallback(async () => {
    const markdown = generateMarkdown();
    await copyToClipboard(markdown, "all");
  }, [generateMarkdown, copyToClipboard]);

  // Handle escape key to close
  const handleEscape = useCallback(
    (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        onClose();
      }
    },
    [onClose]
  );

  // Lock body scroll and handle escape key
  useEffect(() => {
    document.body.style.overflow = "hidden";
    document.addEventListener("keydown", handleEscape);
    return () => {
      document.body.style.overflow = "";
      document.removeEventListener("keydown", handleEscape);
    };
  }, [handleEscape]);

  return createPortal(
    <div className="fixed inset-0 z-[200] flex items-center justify-center bg-black/50">
      <div className="dark bg-zinc-900 text-zinc-100 rounded-lg shadow-xl w-[90vw] max-w-5xl max-h-[85vh] flex flex-col">
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 border-b border-zinc-700">
          <div className="flex items-center gap-2">
            <Bug className="w-5 h-5 text-zinc-300" />
            <h2 className="text-lg font-medium text-zinc-100">Debug View</h2>
            <span className="text-sm text-zinc-400">{debugInfo.model}</span>
          </div>
          <div className="flex items-center gap-4">
            <div className="flex items-center gap-2 text-sm text-zinc-300">
              <Clock className="w-4 h-4" />
              {(totalDuration / 1000).toFixed(2)}s
            </div>
            <div className="flex items-center gap-2">
              {debugInfo.success ? (
                <CheckCircle className="w-4 h-4 text-green-400" />
              ) : (
                <XCircle className="w-4 h-4 text-red-400" />
              )}
              <span className={`text-sm ${debugInfo.success ? "text-green-400" : "text-red-400"}`}>
                {debugInfo.success ? "Success" : "Failed"}
              </span>
            </div>
            <button
              onClick={onClose}
              className="p-1 text-zinc-300 hover:text-zinc-100"
              aria-label="Close debug modal"
            >
              <X className="w-5 h-5" />
            </button>
          </div>
        </div>

        {/* Content */}
        {/* eslint-disable-next-line jsx-a11y/no-noninteractive-tabindex -- scrollable region needs keyboard access (axe: scrollable-region-focusable) */}
        <div className="flex-1 overflow-y-auto p-4" tabIndex={0}>
          <div className="space-y-4">
            {debugInfo.rounds.map((round) => (
              <DebugRoundView
                key={round.round}
                round={round}
                isExpanded={expandedRounds.has(round.round)}
                onToggle={() => toggleRound(round.round)}
                copiedSection={copiedSection}
                onCopy={copyToClipboard}
              />
            ))}
          </div>
        </div>

        {/* Footer */}
        <div className="flex items-center justify-between px-4 py-3 border-t border-zinc-700">
          <div className="text-sm text-zinc-400">
            {debugInfo.rounds.length} round{debugInfo.rounds.length !== 1 ? "s" : ""} | Debug ID:{" "}
            <code className="text-zinc-300">{debugInfo.messageId}</code>
          </div>
          <div className="flex items-center gap-2">
            <button
              onClick={handleCopyAll}
              className="flex items-center gap-2 px-4 py-2 text-sm font-medium text-zinc-300 bg-zinc-800 hover:bg-zinc-700 rounded-md"
            >
              {copiedSection === "all" ? (
                <>
                  <Check className="w-4 h-4 text-green-500" />
                  Copied
                </>
              ) : (
                <>
                  <FileText className="w-4 h-4" />
                  Copy All as Markdown
                </>
              )}
            </button>
            <button
              onClick={onClose}
              className="px-4 py-2 text-sm font-medium text-zinc-300 bg-zinc-800 hover:bg-zinc-700 rounded-md"
            >
              Close
            </button>
          </div>
        </div>
      </div>
    </div>,
    document.body
  );
}

interface DebugRoundViewProps {
  round: DebugRound;
  isExpanded: boolean;
  onToggle: () => void;
  copiedSection: string | null;
  onCopy: (text: string, section: string) => void;
}

function DebugRoundView({
  round,
  isExpanded,
  onToggle,
  copiedSection,
  onCopy,
}: DebugRoundViewProps) {
  const duration =
    round.endTime && round.startTime
      ? ((round.endTime - round.startTime) / 1000).toFixed(2)
      : "...";

  const hasToolCalls = round.toolCalls && round.toolCalls.length > 0;

  return (
    <div className="border border-zinc-700 rounded-lg overflow-hidden">
      {/* Round Header */}
      <button
        onClick={onToggle}
        className="w-full flex items-center gap-2 px-4 py-3 bg-zinc-800/50 hover:bg-zinc-800 text-left"
        aria-label={`${isExpanded ? "Collapse" : "Expand"} round ${round.round}`}
      >
        {isExpanded ? (
          <ChevronDown className="w-4 h-4 text-zinc-300" />
        ) : (
          <ChevronRight className="w-4 h-4 text-zinc-300" />
        )}
        <span className="font-medium text-zinc-100">Round {round.round}</span>
        <span className="text-sm text-zinc-400">{duration}s</span>
        {hasToolCalls && (
          <span className="ml-2 px-2 py-0.5 text-xs bg-blue-500/20 text-blue-400 rounded">
            {round.toolCalls!.length} tool call{round.toolCalls!.length !== 1 ? "s" : ""}
          </span>
        )}
      </button>

      {/* Round Content */}
      {isExpanded && (
        <div className="divide-y divide-zinc-700">
          {/* Input Items */}
          <CollapsibleSection
            title="Input Items"
            subtitle={`${round.inputItems.length} items`}
            defaultExpanded={false}
            sectionId={`round-${round.round}-input`}
            copiedSection={copiedSection}
            onCopy={onCopy}
          >
            <pre className="text-xs text-zinc-300 overflow-x-auto">
              {JSON.stringify(round.inputItems, null, 2)}
            </pre>
          </CollapsibleSection>

          {/* Request Body */}
          {round.requestBody && (
            <CollapsibleSection
              title="Request Body"
              defaultExpanded={false}
              sectionId={`round-${round.round}-request`}
              copiedSection={copiedSection}
              onCopy={onCopy}
            >
              <pre className="text-xs text-zinc-300 overflow-x-auto">
                {JSON.stringify(round.requestBody, null, 2)}
              </pre>
            </CollapsibleSection>
          )}

          {/* Response Output */}
          {round.responseOutput && (
            <CollapsibleSection
              title="Response Output"
              subtitle={`${round.responseOutput.length} items`}
              defaultExpanded={true}
              sectionId={`round-${round.round}-response`}
              copiedSection={copiedSection}
              onCopy={onCopy}
            >
              <pre className="text-xs text-zinc-300 overflow-x-auto">
                {JSON.stringify(round.responseOutput, null, 2)}
              </pre>
            </CollapsibleSection>
          )}

          {/* Tool Calls */}
          {round.toolCalls && round.toolCalls.length > 0 && (
            <CollapsibleSection
              title="Tool Calls"
              subtitle={`${round.toolCalls.length} calls`}
              defaultExpanded={true}
              sectionId={`round-${round.round}-toolcalls`}
              copiedSection={copiedSection}
              onCopy={onCopy}
            >
              <div className="space-y-2">
                {round.toolCalls.map((tc) => (
                  <div key={tc.id} className="p-2 bg-zinc-800/50 rounded border border-zinc-700">
                    <div className="flex items-center gap-2 mb-1">
                      <span className="text-sm font-medium text-blue-400">{tc.name}</span>
                      <code className="text-xs text-zinc-400">{tc.id}</code>
                    </div>
                    <pre className="text-xs text-zinc-300 overflow-x-auto">
                      {JSON.stringify(tc.arguments, null, 2)}
                    </pre>
                  </div>
                ))}
              </div>
            </CollapsibleSection>
          )}

          {/* Tool Results */}
          {round.toolResults && round.toolResults.length > 0 && (
            <CollapsibleSection
              title="Tool Results"
              subtitle={`${round.toolResults.length} results`}
              defaultExpanded={true}
              sectionId={`round-${round.round}-toolresults`}
              copiedSection={copiedSection}
              onCopy={onCopy}
            >
              <div className="space-y-2">
                {round.toolResults.map((tr) => (
                  <div
                    key={tr.callId}
                    className={`p-2 rounded border ${
                      tr.success
                        ? "bg-green-500/10 border-green-500/30"
                        : "bg-red-500/10 border-red-500/30"
                    }`}
                  >
                    <div className="flex items-center gap-2 mb-1">
                      {tr.success ? (
                        <CheckCircle className="w-4 h-4 text-green-500" />
                      ) : (
                        <XCircle className="w-4 h-4 text-red-500" />
                      )}
                      <span className="text-sm font-medium text-zinc-100">{tr.toolName}</span>
                      <code className="text-xs text-zinc-400">{tr.callId}</code>
                    </div>
                    {tr.error && <div className="text-sm text-red-400 mb-1">{tr.error}</div>}
                    {tr.output && (
                      <pre className="text-xs text-zinc-300 overflow-x-auto max-h-48">
                        {tr.output.length > 1000
                          ? tr.output.slice(0, 1000) + "... (truncated)"
                          : tr.output}
                      </pre>
                    )}
                  </div>
                ))}
              </div>
            </CollapsibleSection>
          )}

          {/* Continuation Items */}
          {round.continuationItems && round.continuationItems.length > 0 && (
            <CollapsibleSection
              title="Continuation Items"
              subtitle={`${round.continuationItems.length} items sent to next round`}
              defaultExpanded={false}
              sectionId={`round-${round.round}-continuation`}
              copiedSection={copiedSection}
              onCopy={onCopy}
            >
              <pre className="text-xs text-zinc-300 overflow-x-auto">
                {JSON.stringify(round.continuationItems, null, 2)}
              </pre>
            </CollapsibleSection>
          )}

          {/* Raw SSE Events - shown when captured */}
          {round.sseEvents && round.sseEvents.length > 0 && (
            <CollapsibleSection
              title="Raw SSE Events"
              subtitle={`${round.sseEvents.length} events`}
              defaultExpanded={false}
              sectionId={`round-${round.round}-sse`}
              copiedSection={copiedSection}
              onCopy={onCopy}
            >
              {/* eslint-disable-next-line jsx-a11y/no-noninteractive-tabindex -- scrollable region needs keyboard access (axe: scrollable-region-focusable) */}
              <div className="space-y-1 max-h-96 overflow-y-auto" tabIndex={0}>
                {round.sseEvents.map((event, index) => (
                  <div key={index} className="p-1 bg-zinc-800/50 rounded text-xs font-mono">
                    <span className="text-zinc-400">
                      {new Date(event.timestamp).toISOString().slice(11, 23)}
                    </span>{" "}
                    <span className="text-amber-300">{event.type}</span>
                    <pre className="text-zinc-300 overflow-x-auto">
                      {JSON.stringify(event.data, null, 2)}
                    </pre>
                  </div>
                ))}
              </div>
            </CollapsibleSection>
          )}
        </div>
      )}
    </div>
  );
}

interface CollapsibleSectionProps {
  title: string;
  subtitle?: string;
  defaultExpanded?: boolean;
  sectionId: string;
  copiedSection: string | null;
  onCopy: (text: string, section: string) => void;
  children: React.ReactNode;
}

function CollapsibleSection({
  title,
  subtitle,
  defaultExpanded = true,
  sectionId,
  copiedSection,
  onCopy,
  children,
}: CollapsibleSectionProps) {
  const [isExpanded, setIsExpanded] = useState(defaultExpanded);

  const handleCopy = () => {
    // Get text content from children (assumes pre element)
    const container = document.querySelector(`[data-section-id="${sectionId}"]`);
    if (container) {
      const text = container.textContent || "";
      onCopy(text, sectionId);
    }
  };

  return (
    <div className="px-4 py-2">
      <div className="flex items-center justify-between mb-2">
        <button
          onClick={() => setIsExpanded(!isExpanded)}
          className="flex items-center gap-2 text-sm text-zinc-300 hover:text-zinc-100"
          aria-label={`${isExpanded ? "Collapse" : "Expand"} ${title} section`}
        >
          {isExpanded ? <ChevronDown className="w-3 h-3" /> : <ChevronRight className="w-3 h-3" />}
          <span className="font-medium">{title}</span>
          {subtitle && <span className="text-zinc-400">{subtitle}</span>}
        </button>
        {isExpanded && (
          <button
            onClick={handleCopy}
            className="flex items-center gap-1 text-xs text-zinc-400 hover:text-zinc-200"
            aria-label="Copy to clipboard"
          >
            {copiedSection === sectionId ? (
              <>
                <Check className="w-3 h-3 text-green-500" />
                Copied
              </>
            ) : (
              <>
                <Copy className="w-3 h-3" />
                Copy
              </>
            )}
          </button>
        )}
      </div>
      {/* eslint-disable jsx-a11y/no-noninteractive-tabindex -- scrollable region needs keyboard access (axe: scrollable-region-focusable) */}
      {isExpanded && (
        <div
          data-section-id={sectionId}
          className="bg-zinc-950 rounded p-2 overflow-x-auto max-h-96"
          tabIndex={0}
        >
          {children}
        </div>
      )}
      {/* eslint-enable jsx-a11y/no-noninteractive-tabindex */}
    </div>
  );
}
