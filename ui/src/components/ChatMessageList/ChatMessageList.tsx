import { useRef, useCallback, useMemo, useEffect } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";

import { ChatMessage } from "@/components/ChatMessage/ChatMessage";
import { ScrollToBottomButton } from "@/components/ScrollToBottomButton";
import { ScrollToPreviousButton } from "@/components/ScrollToPreviousButton";
import { ChainProgress } from "@/components/ChainProgress/ChainProgress";
import { ConsensusProgress } from "@/components/ConsensusProgress/ConsensusProgress";
import { CouncilProgress } from "@/components/CouncilProgress/CouncilProgress";
import { CritiqueProgress } from "@/components/CritiqueProgress/CritiqueProgress";
import { DebateProgress } from "@/components/DebateProgress/DebateProgress";
import { ElectedProgress } from "@/components/ElectedProgress/ElectedProgress";
import { ConfidenceProgress } from "@/components/ConfidenceProgress/ConfidenceProgress";
import { ExplainerProgress } from "@/components/ExplainerProgress/ExplainerProgress";
import { HierarchicalProgress } from "@/components/HierarchicalProgress/HierarchicalProgress";
import { RefinementProgress } from "@/components/RefinementProgress/RefinementProgress";
import { ScattershotProgress } from "@/components/ScattershotProgress/ScattershotProgress";
import { RoutingDecision } from "@/components/RoutingDecision/RoutingDecision";
import { SynthesisProgress } from "@/components/SynthesisProgress/SynthesisProgress";
import { TournamentProgress } from "@/components/TournamentProgress/TournamentProgress";
import { useAutoScroll } from "@/hooks/useAutoScroll";
import type { ChatMessage as ChatMessageType } from "@/components/chat-types";
import { EmptyChat } from "@/components/EmptyChat/EmptyChat";
import { MultiModelResponse } from "@/components/MultiModelResponse/MultiModelResponse";
import {
  useMessages,
  useConversationStore,
  useSelectedModels,
  useSelectedInstances,
} from "@/stores/conversationStore";
import { useAllStreams, useIsStreaming } from "@/stores/streamingStore";
import {
  useDisabledModels,
  useSelectedBestResponses,
  useActionConfig,
  useChatUIStore,
  useHiddenResponseIds,
  useWidescreenMode,
} from "@/stores/chatUIStore";

/**
 * ChatMessageList - Virtualized Message List with Streaming Support
 *
 * ## Architecture Overview
 *
 * This component is the core of the chat UI performance story. It combines:
 * 1. **Virtualization** - Only renders visible message groups in the DOM
 * 2. **Streaming isolation** - Active streams render outside virtualization
 * 3. **Memoized children** - ChatMessage and MultiModelResponse use custom memo comparators
 *
 * ## Component Tree
 *
 * ```
 * ChatMessageList
 * ├── useVirtualizer (from @tanstack/react-virtual)
 * │   └── VirtualItem[] (only visible items)
 * │       ├── ChatMessage (user message) - memoized
 * │       └── MultiModelResponse (assistant responses) - memoized
 * │
 * └── Streaming Section (outside virtualization, always at bottom)
 *     └── MultiModelResponse (active streams)
 * ```
 *
 * ## Re-render Behavior
 *
 * **New token arrives during streaming:**
 * - ChatMessageList: ❌ NO RE-RENDER (subscribes to `useAllStreams`, but streaming
 *   section is keyed by session, not by content)
 * - VirtualItem ChatMessages: ❌ NO RE-RENDER (memo comparator passes)
 * - Streaming MultiModelResponse: ✅ RE-RENDERS via its internal `useStreamContent`
 *
 * **User sends new message:**
 * - ChatMessageList: ✅ RE-RENDERS (messages array changed)
 * - Existing VirtualItem ChatMessages: ❌ NO RE-RENDER (same props, memo passes)
 * - New VirtualItem: ✅ MOUNTS
 *
 * ## Why Streaming Is Outside Virtualization
 *
 * Virtualization works by measuring element heights after render. During streaming,
 * content height changes every ~20-50ms. Keeping streaming content outside
 * virtualization avoids constant re-measurement and ensures smooth scrolling.
 *
 * ## Performance-Critical Patterns Used
 *
 * 1. `useMemo` for messageGroups - computed once per messages change
 * 2. `useCallback` for stable onSelectBest/onRegenerate references
 * 3. `streamingSessionIdRef` to prevent animation replay on re-render
 * 4. Surgical store selectors (useMessages, useAllStreams, etc.)
 */

/**
 * A message group represents a user message and its associated assistant responses.
 * This grouping is computed at render time via useMemo, not stored.
 */
interface MessageGroup {
  id: string;
  userMessage: ChatMessageType;
  assistantResponses: ChatMessageType[];
  /** Responses that have been hidden by the user */
  hiddenResponses: ChatMessageType[];
}

interface ChatMessageListProps {
  /** Whether to show loading state for models */
  isLoadingModels?: boolean;
  /** Whether models finished loading but none are available */
  noModelsAvailable?: boolean;
  /** Callback to regenerate a response */
  onRegenerate?: (messageId: string, model: string) => void;
  /** Callback to fork conversation from a specific message */
  onForkFromMessage?: (messageId: string) => void;
  /** Callback to edit a message and re-run from that point */
  onEditAndRerun?: (messageId: string, newContent: string) => void;
  /** Callback to regenerate all responses for a user message (same as edit & resend with unchanged content) */
  onRegenerateAll?: (messageId: string) => void;
}

export function ChatMessageList({
  isLoadingModels = false,
  noModelsAvailable = false,
  onRegenerate,
  onForkFromMessage,
  onEditAndRerun,
  onRegenerateAll,
}: ChatMessageListProps) {
  // ============================================================================
  // STORE SUBSCRIPTIONS (Surgical Selectors)
  // Each selector subscribes to the minimum data needed to prevent re-renders
  // ============================================================================
  const messages = useMessages();
  const modelResponses = useAllStreams();
  const isStreaming = useIsStreaming();
  const selectedModels = useSelectedModels();
  const selectedInstances = useSelectedInstances();
  const disabledModels = useDisabledModels();
  const selectedBestResponses = useSelectedBestResponses();
  const actionConfig = useActionConfig();
  const hiddenResponseIds = useHiddenResponseIds();
  const widescreenMode = useWidescreenMode();

  // Create a lookup map for instance labels (instanceId -> label)
  const instanceLabels = useMemo(() => {
    const map = new Map<string, string>();
    for (const instance of selectedInstances) {
      if (instance.label) {
        map.set(instance.id, instance.label);
      }
    }
    return map;
  }, [selectedInstances]);

  // Get store actions for selection/hiding
  const { setSelectedBest: setStoreBest } = useConversationStore();
  const { setSelectedBest: setUIBest, hideResponse, showResponse } = useChatUIStore();

  // ============================================================================
  // STABLE CALLBACKS
  // useCallback ensures function references don't change between renders,
  // which is critical for memo comparators in child components
  // ============================================================================
  const onSelectBest = useCallback(
    (messageId: string, model: string | null) => {
      // Update both stores (conversation store for persistence, UI store for display)
      setStoreBest(messageId, model);
      setUIBest(messageId, model);
    },
    [setStoreBest, setUIBest]
  );

  const onHide = useCallback(
    (groupId: string, instanceId: string) => {
      hideResponse(groupId, instanceId);
    },
    [hideResponse]
  );

  const onShowHidden = useCallback(
    (groupId: string, instanceId: string) => {
      showResponse(groupId, instanceId);
    },
    [showResponse]
  );

  // ============================================================================
  // AUTO-SCROLL
  // Uses 'instant' during streaming for smooth token updates,
  // 'smooth' for non-streaming (e.g., after sending a message)
  // Passing isStreaming disables ResizeObserver scroll checks during streaming
  // to prevent fighting with user's scroll intent
  // ============================================================================
  const { containerRef, userHasScrolledUp, handleScroll, scrollToBottom } = useAutoScroll({
    isStreaming,
  });
  const streamingTimestampRef = useRef<Date>(new Date());

  // ============================================================================
  // MEMOIZED COMPUTATIONS
  // These are recomputed only when dependencies change
  // ============================================================================

  // Filter streaming responses to exclude disabled instances
  const filteredModelResponses = useMemo(() => {
    return modelResponses.filter((r) => {
      // Check by instanceId first, then fall back to model
      const instanceId = r.instanceId ?? r.model;
      return !disabledModels.includes(instanceId);
    });
  }, [modelResponses, disabledModels]);

  /**
   * Pre-compute message groups: each user message paired with its following assistant responses.
   *
   * This grouping enables rendering user + assistant responses as a unit in virtualization.
   * The memo ensures we only recompute when messages or hidden responses change.
   *
   * NOTE: disabledModels is intentionally NOT used here for committed messages.
   * - disabledModels only affects FUTURE queries (prevents querying disabled instances)
   * - hiddenResponseIds is used for hiding individual past responses
   * This separation allows users to disable a model for future queries without hiding past responses.
   */
  const messageGroups = useMemo(() => {
    const groups: MessageGroup[] = [];
    for (let i = 0; i < messages.length; i++) {
      const message = messages[i];
      if (message.role === "user") {
        const groupId = message.id;
        const assistantResponses: ChatMessageType[] = [];
        const hiddenResponses: ChatMessageType[] = [];
        // Collect all following assistant messages until the next user message
        for (let j = i + 1; j < messages.length; j++) {
          const nextMsg = messages[j];
          if (nextMsg.role === "assistant") {
            const instanceId = nextMsg.instanceId ?? nextMsg.model;
            // Separate visible and hidden responses
            // (disabled models only affects future queries, not past responses)
            const hiddenKey = `${groupId}:${instanceId}`;
            if (hiddenResponseIds.has(hiddenKey)) {
              hiddenResponses.push(nextMsg);
            } else {
              assistantResponses.push(nextMsg);
            }
          } else {
            break;
          }
        }
        groups.push({
          id: groupId,
          userMessage: message,
          assistantResponses,
          hiddenResponses,
        });
      }
    }
    return groups;
  }, [messages, hiddenResponseIds]);

  const hasMessages = messages.length > 0;
  const hasStreamingResponses = filteredModelResponses.length > 0;

  // ============================================================================
  // STREAMING SESSION TRACKING
  // Prevents animation replay when component re-renders during streaming
  // ============================================================================
  const streamingSessionIdRef = useRef(0);
  const prevHasStreamingRef = useRef(false);
  if (hasStreamingResponses && !prevHasStreamingRef.current) {
    streamingTimestampRef.current = new Date();
    streamingSessionIdRef.current += 1;
  }
  prevHasStreamingRef.current = hasStreamingResponses;

  // ============================================================================
  // VIRTUALIZATION
  // Only renders visible message groups + overscan buffer
  // ============================================================================
  const virtualizer = useVirtualizer({
    count: messageGroups.length,
    getScrollElement: () => containerRef.current,
    estimateSize: () => 200, // Estimated height per message group
    overscan: 3, // Render 3 extra items above/below viewport for smooth scrolling
    enabled: messageGroups.length > 0,
  });

  // Track message count to detect new user messages
  const prevMessagesLengthRef = useRef(messages.length);

  // Only auto-scroll when a NEW user message is added (not during streaming)
  // This allows users to read at their own pace during streaming
  useEffect(() => {
    if (!userHasScrolledUp && messages.length > prevMessagesLengthRef.current) {
      scrollToBottom(false); // smooth scroll for new user message
    }
    prevMessagesLengthRef.current = messages.length;
  }, [messages.length, userHasScrolledUp, scrollToBottom]);

  // Handler to scroll to the previous message group
  const handleScrollToPrevious = useCallback(() => {
    if (messageGroups.length === 0 || !containerRef.current) return;

    const container = containerRef.current;
    const currentScrollTop = container.scrollTop;

    // Find the virtualizer item that's currently at the top of the viewport
    const virtualItems = virtualizer.getVirtualItems();
    let targetIndex = -1;

    for (let i = virtualItems.length - 1; i >= 0; i--) {
      const item = virtualItems[i];
      if (item.start < currentScrollTop - 10) {
        // Found the previous item (10px buffer for edge cases)
        targetIndex = item.index;
        break;
      }
    }

    // If no previous item found, go to the first message
    if (targetIndex < 0) {
      targetIndex = 0;
    }

    // Scroll to the target message group
    virtualizer.scrollToIndex(targetIndex, { align: "start", behavior: "smooth" });
  }, [messageGroups.length, virtualizer, containerRef]);

  // Callback for scroll-to-bottom button (always use smooth scroll for user-initiated action)
  const handleScrollToBottomClick = useCallback(() => {
    scrollToBottom(false);
  }, [scrollToBottom]);

  return (
    <div className="relative flex-1 flex flex-col min-h-0">
      <div
        ref={containerRef}
        onScroll={handleScroll}
        className="flex-1 overflow-y-auto scrollbar-thin"
      >
        <div className={`mx-auto px-3 py-4 sm:px-4 sm:py-6 ${widescreenMode ? "" : "max-w-6xl"}`}>
          {!hasMessages && !hasStreamingResponses ? (
            <div className="h-[calc(100vh-280px)] sm:h-[calc(100vh-300px)] flex items-center justify-center">
              <EmptyChat
                selectedModels={selectedModels}
                isLoadingModels={isLoadingModels}
                noModelsAvailable={noModelsAvailable}
              />
            </div>
          ) : (
            <div
              className="relative"
              style={{
                // Use max of virtualizer size and estimated size to prevent layout jumps
                height:
                  Math.max(virtualizer.getTotalSize(), messageGroups.length * 200) +
                  (hasStreamingResponses ? 200 : 0),
              }}
            >
              {/* Virtualized message groups */}
              {virtualizer.getVirtualItems().map((virtualItem) => {
                const group = messageGroups[virtualItem.index];
                return (
                  <div
                    key={group.id}
                    ref={virtualizer.measureElement}
                    data-index={virtualItem.index}
                    className="absolute left-0 right-0 pb-4 sm:pb-6"
                    style={{ transform: `translateY(${virtualItem.start}px)` }}
                  >
                    <ChatMessage
                      message={group.userMessage}
                      onFork={onForkFromMessage}
                      onSaveEdit={onEditAndRerun}
                      onRegenerate={onRegenerateAll}
                    />
                    {group.assistantResponses.length > 0 && (
                      <>
                        {/* Show persisted mode indicators for chained/routed messages */}
                        {group.assistantResponses[0].modeMetadata?.mode === "routed" && (
                          <div className="mb-3">
                            <RoutingDecision
                              persistedMetadata={{
                                routerModel:
                                  group.assistantResponses[0].modeMetadata.routerModel || "",
                                selectedModel: group.assistantResponses[0].model || "",
                                reasoning:
                                  group.assistantResponses[0].modeMetadata.routingReasoning,
                                routerUsage: group.assistantResponses[0].modeMetadata.routerUsage,
                              }}
                            />
                          </div>
                        )}
                        {/* ChainProgress is only shown during live streaming, not for persisted messages */}
                        {group.assistantResponses[0].modeMetadata?.mode === "synthesized" && (
                          <div className="mb-3">
                            <SynthesisProgress
                              persistedMetadata={{
                                synthesizerModel:
                                  group.assistantResponses[0].modeMetadata.synthesizerModel || "",
                                completedModels:
                                  group.assistantResponses[0].modeMetadata.sourceResponses?.map(
                                    (r) => r.model
                                  ) || [],
                                sourceResponses:
                                  group.assistantResponses[0].modeMetadata.sourceResponses,
                              }}
                            />
                          </div>
                        )}
                        {group.assistantResponses[0].modeMetadata?.mode === "refined" && (
                          <RefinementProgress
                            persistedMetadata={{
                              currentRound:
                                group.assistantResponses[0].modeMetadata.refinementRound ?? 0,
                              totalRounds:
                                group.assistantResponses[0].modeMetadata.totalRounds ?? 1,
                              rounds:
                                group.assistantResponses[0].modeMetadata.refinementHistory || [],
                            }}
                          />
                        )}
                        {group.assistantResponses[0].modeMetadata?.mode === "critiqued" && (
                          <CritiqueProgress
                            persistedMetadata={{
                              primaryModel:
                                group.assistantResponses[0].modeMetadata.primaryModel || "",
                              initialResponse:
                                group.assistantResponses[0].modeMetadata.initialResponse,
                              initialUsage: group.assistantResponses[0].modeMetadata.initialUsage,
                              critiques: group.assistantResponses[0].modeMetadata.critiques || [],
                            }}
                          />
                        )}
                        {group.assistantResponses[0].modeMetadata?.mode === "elected" && (
                          <ElectedProgress
                            persistedMetadata={{
                              candidates: group.assistantResponses[0].modeMetadata.candidates || [],
                              votes: group.assistantResponses[0].modeMetadata.votes,
                              voteCounts: group.assistantResponses[0].modeMetadata.voteCounts,
                              winner: group.assistantResponses[0].modeMetadata.winner,
                              voteUsage: group.assistantResponses[0].modeMetadata.voteUsage,
                            }}
                          />
                        )}
                        {group.assistantResponses[0].modeMetadata?.mode === "tournament" && (
                          <div className="mb-3">
                            <TournamentProgress
                              persistedMetadata={{
                                bracket: group.assistantResponses[0].modeMetadata.bracket || [],
                                matches: group.assistantResponses[0].modeMetadata.matches || [],
                                winner: group.assistantResponses[0].modeMetadata.tournamentWinner,
                                eliminatedPerRound:
                                  group.assistantResponses[0].modeMetadata.eliminatedPerRound,
                              }}
                            />
                          </div>
                        )}
                        {group.assistantResponses[0].modeMetadata?.mode === "consensus" && (
                          <div className="mb-3">
                            <ConsensusProgress
                              persistedMetadata={{
                                rounds: group.assistantResponses[0].modeMetadata.rounds || [],
                                finalScore: group.assistantResponses[0].modeMetadata.consensusScore,
                                consensusReached:
                                  group.assistantResponses[0].modeMetadata.consensusReached,
                                aggregateUsage:
                                  group.assistantResponses[0].modeMetadata.aggregateUsage,
                              }}
                            />
                          </div>
                        )}
                        {group.assistantResponses[0].modeMetadata?.mode === "debated" && (
                          <div className="mb-3">
                            <DebateProgress
                              persistedMetadata={{
                                turns: group.assistantResponses[0].modeMetadata.debateTurns || [],
                                positions:
                                  group.assistantResponses[0].modeMetadata.debatePositions || {},
                                debateRounds: group.assistantResponses[0].modeMetadata.debateRounds,
                                summarizerModel:
                                  group.assistantResponses[0].modeMetadata.summarizerModel,
                                aggregateUsage:
                                  group.assistantResponses[0].modeMetadata.aggregateUsage,
                              }}
                            />
                          </div>
                        )}
                        {group.assistantResponses[0].modeMetadata?.mode === "council" && (
                          <div className="mb-3">
                            <CouncilProgress
                              persistedMetadata={{
                                statements:
                                  group.assistantResponses[0].modeMetadata.councilStatements || [],
                                roles: group.assistantResponses[0].modeMetadata.councilRoles || {},
                                councilRounds:
                                  group.assistantResponses[0].modeMetadata.councilRounds,
                                synthesizerModel:
                                  group.assistantResponses[0].modeMetadata.summarizerModel,
                                aggregateUsage:
                                  group.assistantResponses[0].modeMetadata.aggregateUsage,
                              }}
                            />
                          </div>
                        )}
                        {group.assistantResponses[0].modeMetadata?.mode === "hierarchical" && (
                          <div className="mb-3">
                            <HierarchicalProgress
                              persistedMetadata={{
                                subtasks: group.assistantResponses[0].modeMetadata.subtasks || [],
                                workerResults:
                                  group.assistantResponses[0].modeMetadata.workerResults || [],
                                coordinatorModel:
                                  group.assistantResponses[0].modeMetadata.coordinatorModel,
                                aggregateUsage:
                                  group.assistantResponses[0].modeMetadata.aggregateUsage,
                              }}
                            />
                          </div>
                        )}
                        {group.assistantResponses[0].modeMetadata?.mode === "explainer" && (
                          <div className="mb-3">
                            <ExplainerProgress
                              persistedMetadata={{
                                explanations:
                                  group.assistantResponses[0].modeMetadata.explanations || [],
                                levels:
                                  group.assistantResponses[0].modeMetadata.explainerLevels || [],
                                aggregateUsage:
                                  group.assistantResponses[0].modeMetadata.aggregateUsage,
                              }}
                            />
                          </div>
                        )}
                        {group.assistantResponses[0].modeMetadata?.mode ===
                          "confidence-weighted" && (
                          <div className="mb-3">
                            <ConfidenceProgress
                              persistedMetadata={{
                                responses:
                                  group.assistantResponses[0].modeMetadata.confidenceResponses ||
                                  [],
                                synthesizerModel:
                                  group.assistantResponses[0].modeMetadata.synthesizerModel,
                              }}
                            />
                          </div>
                        )}
                        <MultiModelResponse
                          responses={group.assistantResponses.map((m) => {
                            // Use instanceId if set, otherwise fall back to model for backwards compat
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
                              debugMessageId: m.debugMessageId,
                            };
                          })}
                          hiddenResponses={group.hiddenResponses.map((m) => {
                            const instanceId = m.instanceId ?? m.model ?? "unknown";
                            return {
                              model: m.model || "unknown",
                              instanceId,
                              label: instanceLabels.get(instanceId),
                            };
                          })}
                          timestamp={group.assistantResponses[0].timestamp}
                          groupId={group.id}
                          onSelectBest={onSelectBest}
                          onRegenerate={onRegenerate}
                          onHide={onHide}
                          onSaveEdit={onEditAndRerun}
                          onShowHidden={onShowHidden}
                          selectedBest={selectedBestResponses[group.id]}
                          actionConfig={actionConfig}
                          historyMode={group.userMessage.historyMode}
                        />
                      </>
                    )}
                  </div>
                );
              })}

              {/*
              STREAMING SECTION - Outside Virtualization

              Active streaming responses render here, positioned absolutely at the bottom.
              This is intentionally outside the virtualized list because:
              1. Streaming content height changes constantly (every token)
              2. Virtualization re-measures heights, which would cause jank
              3. The streaming section should always be visible (no virtualization cutoff)

              The key={streamingSessionIdRef.current} ensures animation only plays once
              per streaming session, not on every content update.
            */}
              {/* Show streaming section when we have streaming responses */}
              {hasStreamingResponses && (
                <div
                  className="absolute left-0 right-0"
                  style={{
                    // Use virtualizer total size, with fallback to estimated size for unmeasured groups
                    transform: `translateY(${Math.max(virtualizer.getTotalSize(), messageGroups.length * 200)}px)`,
                  }}
                >
                  {/* Routing decision indicator for routed mode */}
                  <RoutingDecision />
                  {/* Chain progress indicator for chained mode */}
                  <ChainProgress
                    models={selectedModels.filter((m) => !disabledModels.includes(m))}
                  />
                  {/* Synthesis progress indicator for synthesized mode */}
                  <SynthesisProgress
                    allModels={selectedModels.filter((m) => !disabledModels.includes(m))}
                  />
                  {/* Refinement progress indicator for refined mode */}
                  <RefinementProgress />
                  {/* Critique progress indicator for critiqued mode */}
                  <CritiqueProgress />
                  {/* Election progress indicator for elected mode */}
                  <ElectedProgress
                    allModels={selectedModels.filter((m) => !disabledModels.includes(m))}
                  />
                  {/* Tournament progress indicator for tournament mode */}
                  <TournamentProgress
                    allModels={selectedModels.filter((m) => !disabledModels.includes(m))}
                  />
                  {/* Consensus progress indicator for consensus mode */}
                  <ConsensusProgress
                    allModels={selectedModels.filter((m) => !disabledModels.includes(m))}
                  />
                  {/* Debate progress indicator for debated mode */}
                  <DebateProgress
                    allModels={selectedModels.filter((m) => !disabledModels.includes(m))}
                  />
                  {/* Council progress indicator for council mode */}
                  <CouncilProgress
                    allModels={selectedModels.filter((m) => !disabledModels.includes(m))}
                  />
                  {/* Hierarchical progress indicator for hierarchical mode */}
                  <HierarchicalProgress />
                  {/* Scattershot progress indicator for scattershot mode */}
                  <ScattershotProgress />
                  {/* Explainer progress indicator for explainer mode */}
                  <ExplainerProgress />
                  {/* Confidence-weighted progress indicator for confidence-weighted mode */}
                  <ConfidenceProgress
                    allModels={selectedModels.filter((m) => !disabledModels.includes(m))}
                  />
                  {/* Key ensures animation only plays once per streaming session */}
                  {hasStreamingResponses && (
                    <div key={streamingSessionIdRef.current} className="animate-slide-up-bounce">
                      <MultiModelResponse
                        responses={filteredModelResponses.map((r) => {
                          // Use instanceId if set, otherwise fall back to model
                          const instanceId = r.instanceId ?? r.model;
                          return {
                            ...r,
                            instanceId,
                            label: instanceLabels.get(instanceId),
                          };
                        })}
                        timestamp={streamingTimestampRef.current}
                        actionConfig={actionConfig}
                      />
                    </div>
                  )}
                </div>
              )}
            </div>
          )}
        </div>
      </div>
      {/* Navigation buttons - positioned in a vertical stack */}
      <div className="absolute bottom-4 right-4 z-10 flex flex-col gap-2">
        <ScrollToPreviousButton
          visible={userHasScrolledUp && messageGroups.length > 1}
          onClick={handleScrollToPrevious}
        />
        <ScrollToBottomButton visible={userHasScrolledUp} onClick={handleScrollToBottomClick} />
      </div>
    </div>
  );
}
