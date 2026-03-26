import { useEffect, useRef } from "react";
import { createPortal } from "react-dom";

import { ChatMessage } from "@/components/ChatMessage/ChatMessage";
import type { ChatMessage as ChatMessageType, MessageUsage } from "@/components/chat-types";
import { HadrianIcon } from "@/components/HadrianIcon/HadrianIcon";
import { MultiModelResponse } from "@/components/MultiModelResponse/MultiModelResponse";
import { useConfig } from "@/config/ConfigProvider";
import { usePreferences } from "@/preferences/PreferencesProvider";
import type { TotalUsageResult } from "@/stores/conversationStore";
import { captureElementAsBlob } from "@/utils/exportScreenshot";
import { formatCost, formatTokens } from "@/utils/formatters";

interface MessageGroup {
  id: string;
  userMessage: ChatMessageType;
  assistantResponses: ChatMessageType[];
}

interface ScreenshotRendererProps {
  title: string;
  messageGroups: MessageGroup[];
  instanceLabels: Map<string, string>;
  totalUsage?: TotalUsageResult | null;
  titleGenerationUsage?: MessageUsage;
  onComplete: (blob?: Blob, error?: Error) => void;
}

export function ScreenshotRenderer({
  title,
  messageGroups,
  instanceLabels,
  totalUsage,
  titleGenerationUsage,
  onComplete,
}: ScreenshotRendererProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const { config } = useConfig();
  const { resolvedTheme } = usePreferences();

  const branding = config?.branding;
  const logoUrl =
    resolvedTheme === "dark" && branding?.logo_dark_url
      ? branding.logo_dark_url
      : branding?.logo_url;
  const appName = branding?.title || "Hadrian Gateway";
  const isCustomBranded = !!(
    (branding?.title && branding.title !== "Hadrian Gateway") ||
    branding?.logo_url ||
    branding?.logo_dark_url
  );

  // Compute combined usage for display
  const grandTotalTokens =
    (totalUsage?.grandTotal.totalTokens ?? 0) + (titleGenerationUsage?.totalTokens ?? 0);
  const grandTotalCost = (totalUsage?.grandTotal.cost ?? 0) + (titleGenerationUsage?.cost ?? 0);

  const onCompleteRef = useRef(onComplete);
  useEffect(() => {
    onCompleteRef.current = onComplete;
  }, [onComplete]);

  useEffect(() => {
    let cancelled = false;
    let timeoutId: ReturnType<typeof setTimeout>;

    const raf = requestAnimationFrame(() => {
      timeoutId = setTimeout(async () => {
        if (cancelled) return;
        try {
          const el = containerRef.current;
          if (!el) throw new Error("Screenshot container not found");
          const blob = await captureElementAsBlob(el);
          if (!cancelled) onCompleteRef.current(blob);
        } catch (err) {
          if (!cancelled)
            onCompleteRef.current(undefined, err instanceof Error ? err : new Error(String(err)));
        }
      }, 500);
    });

    return () => {
      cancelled = true;
      cancelAnimationFrame(raf);
      clearTimeout(timeoutId);
    };
  }, [title]);

  const themeClass = document.documentElement.classList.contains("dark") ? "dark" : "";

  return createPortal(
    <div
      ref={containerRef}
      aria-hidden="true"
      className={`${themeClass} bg-background text-foreground`}
      style={{
        position: "fixed",
        left: "-99999px",
        top: 0,
        width: 800,
        padding: 32,
      }}
    >
      {/* Branding header */}
      <div className="mb-6 flex items-center justify-between border-b border-border pb-4">
        <div className="flex items-center gap-2.5">
          {logoUrl ? (
            <img src={logoUrl} alt={appName} className="h-8 w-8 rounded-lg object-contain" />
          ) : (
            <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-primary/10">
              <HadrianIcon size={24} className="text-primary" />
            </div>
          )}
          <span className="font-semibold tracking-tight">{appName}</span>
        </div>
        {!isCustomBranded && (
          <span className="text-xs text-muted-foreground">hadriangateway.com</span>
        )}
      </div>

      {/* Title + usage breakdown */}
      <div className="mb-6">
        <h1 className="text-xl font-semibold">{title}</h1>
        {totalUsage && grandTotalTokens > 0 && (
          <div className="mt-3 text-xs text-muted-foreground">
            <div className="font-medium text-foreground mb-1">Usage</div>
            <div>Input: {formatTokens(totalUsage.total.inputTokens)} tokens</div>
            <div>Output: {formatTokens(totalUsage.total.outputTokens)} tokens</div>
            {(totalUsage.total.cachedTokens ?? 0) > 0 && (
              <div>Cached: {formatTokens(totalUsage.total.cachedTokens!)} tokens</div>
            )}
            {(totalUsage.total.reasoningTokens ?? 0) > 0 && (
              <div>Reasoning: {formatTokens(totalUsage.total.reasoningTokens!)} tokens</div>
            )}
            {totalUsage.modeOverhead.totalTokens > 0 && (
              <div>
                Mode overhead: {formatTokens(totalUsage.modeOverhead.totalTokens)} tokens
                {(totalUsage.modeOverhead.cost ?? 0) > 0 && (
                  <> · {formatCost(totalUsage.modeOverhead.cost!)}</>
                )}
              </div>
            )}
            {titleGenerationUsage && (
              <div>
                Title generation: {formatTokens(titleGenerationUsage.totalTokens)} tokens
                {(titleGenerationUsage.cost ?? 0) > 0 && (
                  <> · {formatCost(titleGenerationUsage.cost!)}</>
                )}
              </div>
            )}
            <div className="mt-1 font-medium text-foreground">
              Total: {formatTokens(grandTotalTokens)} tokens
              {grandTotalCost > 0 && <> · {formatCost(grandTotalCost)}</>}
            </div>
          </div>
        )}
      </div>

      {messageGroups.map((group) => (
        <div key={group.id} className="pb-6">
          <ChatMessage message={group.userMessage} />
          {group.assistantResponses.length > 0 && (
            <MultiModelResponse
              forceStacked
              responses={group.assistantResponses.map((m) => {
                const instanceId = m.instanceId ?? m.model ?? "unknown";
                return {
                  model: m.model || "unknown",
                  instanceId,
                  messageId: m.id,
                  label: instanceLabels.get(instanceId),
                  content: m.content,
                  isStreaming: false,
                  error: m.error,
                  usage: m.usage,
                  feedback: m.feedback,
                  modeMetadata: m.modeMetadata,
                  citations: m.citations,
                  artifacts: m.artifacts,
                  toolExecutionRounds: m.toolExecutionRounds,
                  completedRounds: m.completedRounds,
                  debugMessageId: m.debugMessageId,
                };
              })}
              timestamp={group.assistantResponses[0].timestamp}
            />
          )}
        </div>
      ))}
    </div>,
    document.body
  );
}
