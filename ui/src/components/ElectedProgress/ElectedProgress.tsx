import { Vote, Trophy } from "lucide-react";

import type { CandidateResponse, VoteData } from "@/stores/streamingStore";
import { useActiveElectedState } from "@/stores/streamingStore";
import type { MessageUsage } from "@/components/chat-types";
import {
  ProgressContainer,
  ModeHeader,
  StatusBadge,
  ModelBadge,
  ExpandButton,
  UsageSummary,
  ResponseCard,
  getShortModelName,
  aggregateUsage,
  type ProgressPhase,
} from "@/components/ModeProgress";
import { cn } from "@/utils/cn";

interface ElectedProgressProps {
  /** All models participating (for display during responding) */
  allModels?: string[];
  /**
   * Persisted metadata for displaying historical messages.
   * When provided, this takes precedence over live streaming state.
   */
  persistedMetadata?: {
    candidates: Array<{
      model: string;
      content: string;
      usage?: MessageUsage;
    }>;
    votes?: Array<{
      voter: string;
      votedFor: string;
      reasoning?: string;
      usage?: MessageUsage;
    }>;
    voteCounts?: Record<string, number>;
    winner?: string;
    voteUsage?: MessageUsage;
  };
}

/**
 * ElectedProgress - Visual indicator for elected mode
 *
 * Shows the election process:
 * 1. During "responding" phase: Shows progress of parallel model responses
 * 2. During "voting" phase: Shows voting progress
 * 3. During "done" phase: Shows winner with vote breakdown and expandable details
 *
 * Uses the new `useActiveElectedState()` selector for live streaming state.
 * For persisted messages, accepts `persistedMetadata` prop to display historical data.
 */
export function ElectedProgress({ allModels, persistedMetadata }: ElectedProgressProps) {
  // Use new discriminated union selector for live streaming state
  const liveState = useActiveElectedState();

  // Determine which state to use: persisted metadata or live streaming state
  const isPersisted = !!persistedMetadata;
  const phase = isPersisted ? "done" : (liveState?.phase ?? "done");
  const candidates: CandidateResponse[] =
    persistedMetadata?.candidates ?? liveState?.candidates ?? [];
  const completedResponses = liveState?.completedResponses ?? candidates.length;
  const totalModels = liveState?.totalModels ?? candidates.length;
  const votes: VoteData[] = persistedMetadata?.votes ?? liveState?.votes ?? [];
  const completedVotes = liveState?.completedVotes ?? votes.length;
  const winner = persistedMetadata?.winner ?? liveState?.winner;
  const voteCounts = persistedMetadata?.voteCounts ?? liveState?.voteCounts ?? {};

  // Don't render if there's no state at all (neither live nor persisted)
  if (!liveState && !persistedMetadata) {
    return null;
  }

  const isResponding = phase === "responding";
  const isVoting = phase === "voting";
  const isDone = phase === "done";

  // Map internal phase to ProgressPhase
  // responding (blue) -> initial, voting (amber) -> active, done (green) -> complete
  const progressPhase: ProgressPhase = isResponding ? "initial" : isVoting ? "active" : "complete";

  // Get display models - during responding, show all models; after, show completed
  const displayModels = allModels || candidates.map((c) => c.model);
  const completedModels = candidates.map((c) => c.model);

  // Calculate aggregate usage
  const candidateUsage = aggregateUsage(candidates);
  const computedVoteUsage = aggregateUsage(votes);
  const voteUsageTotals = persistedMetadata?.voteUsage
    ? {
        totalTokens: persistedMetadata.voteUsage.totalTokens,
        cost: persistedMetadata.voteUsage.cost ?? 0,
      }
    : computedVoteUsage;
  const totalUsage = {
    totalTokens: candidateUsage.totalTokens + voteUsageTotals.totalTokens,
    totalCost: (candidateUsage.cost ?? 0) + (voteUsageTotals.cost ?? 0),
  };

  const hasCandidates = candidates.length > 0;
  const hasVotes = votes.length > 0;

  // Build status badge text
  const statusText = isResponding ? "RESPONDING" : isVoting ? "VOTING" : "COMPLETE";

  return (
    <div className="mb-3">
      <ProgressContainer
        phase={progressPhase}
        isLoading={isResponding || isVoting}
        icon={Vote}
        header={
          <ModeHeader
            name="Elected"
            badge={<StatusBadge text={statusText} variant={progressPhase} />}
          />
        }
        expandableSection={
          hasCandidates ? (
            <>
              {/* Candidate responses */}
              <div>
                <p className="text-[10px] font-medium text-muted-foreground mb-2">
                  Candidate Responses
                </p>
                <div className="space-y-2">
                  {candidates.map((candidate, index) => {
                    const isWinner = candidate.model === winner;
                    return (
                      <ResponseCard
                        key={candidate.model + index}
                        title={`${getShortModelName(candidate.model)}${isWinner ? " (Winner)" : ""}`}
                        content={candidate.content}
                        usage={candidate.usage}
                        variant={isWinner ? "blue" : "default"}
                      />
                    );
                  })}
                </div>
              </div>

              {/* Vote details */}
              {hasVotes && (
                <div>
                  <p className="text-[10px] font-medium text-muted-foreground mb-2">Vote Details</p>
                  <div className="flex flex-wrap gap-2">
                    {votes.map((vote, index) => (
                      <div
                        key={vote.voter + index}
                        className="flex items-center gap-1.5 px-2 py-1 rounded bg-muted/50 text-[10px]"
                      >
                        <span className="text-muted-foreground">
                          {getShortModelName(vote.voter)}
                        </span>
                        <span className="text-muted-foreground/50">voted for</span>
                        <span className="font-medium">{getShortModelName(vote.votedFor)}</span>
                      </div>
                    ))}
                  </div>
                </div>
              )}
            </>
          ) : undefined
        }
        expandLabel={{ collapsed: "Show details", expanded: "Hide details" }}
        showExpandable={hasCandidates}
        renderFooter={
          isDone
            ? ({ isExpanded, toggleExpand, hasExpandable }) => (
                <div className="mt-2 space-y-2">
                  {/* Winner announcement */}
                  <div className="flex items-center gap-2">
                    <Trophy className="h-4 w-4 text-yellow-500" />
                    <span className="text-xs font-medium">Winner:</span>
                    <span className="px-1.5 py-0.5 rounded text-[10px] font-semibold bg-primary/10 text-primary">
                      {winner ? getShortModelName(winner) : "N/A"}
                    </span>
                    {winner && voteCounts[winner] !== undefined && (
                      <span className="text-[10px] text-muted-foreground">
                        ({voteCounts[winner]} vote
                        {voteCounts[winner] !== 1 ? "s" : ""})
                      </span>
                    )}
                  </div>

                  {/* Vote breakdown */}
                  {hasCandidates && (
                    <div className="flex items-center gap-2 flex-wrap">
                      {candidates.map((candidate) => {
                        const count = voteCounts[candidate.model] || 0;
                        const isWinner = candidate.model === winner;
                        return (
                          <div
                            key={candidate.model}
                            className={cn(
                              "flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-medium",
                              isWinner
                                ? "bg-primary/10 text-primary"
                                : "bg-muted text-muted-foreground"
                            )}
                          >
                            <span className="max-w-[60px] truncate">
                              {getShortModelName(candidate.model)}
                            </span>
                            <span className="font-semibold">{count}</span>
                          </div>
                        );
                      })}
                    </div>
                  )}

                  {/* Usage and expand button */}
                  <div className="flex items-center justify-between">
                    <UsageSummary
                      totalTokens={totalUsage.totalTokens}
                      totalCost={totalUsage.totalCost}
                    />
                    {hasExpandable && (
                      <ExpandButton
                        isExpanded={isExpanded}
                        onToggle={toggleExpand}
                        expandedLabel="Hide details"
                        collapsedLabel="Show details"
                      />
                    )}
                  </div>
                </div>
              )
            : undefined
        }
      >
        {/* Model progress during responding phase */}
        {isResponding && (
          <div className="mt-2 space-y-1">
            <div className="flex items-center gap-1.5 flex-wrap">
              {displayModels.map((model) => {
                const isComplete = completedModels.includes(model);
                return (
                  <ModelBadge
                    key={model}
                    model={model}
                    variant={isComplete ? "primary" : "default"}
                    showCheck={isComplete}
                    showLoading={!isComplete}
                  />
                );
              })}
            </div>
            <p className="text-[10px] text-muted-foreground">
              {completedResponses}/{totalModels} candidates ready
            </p>
          </div>
        )}

        {/* Voting progress during voting phase */}
        {isVoting && (
          <div className="mt-2 space-y-1">
            <div className="flex items-center gap-1.5 flex-wrap">
              {displayModels.map((model) => {
                const hasVoted = votes.some((v) => v.voter === model);
                return (
                  <ModelBadge
                    key={model}
                    model={model}
                    variant={hasVoted ? "amber" : "default"}
                    showCheck={hasVoted}
                    showLoading={!hasVoted}
                  />
                );
              })}
            </div>
            <p className="text-[10px] text-muted-foreground animate-pulse">
              {completedVotes}/{totalModels} votes cast...
            </p>
          </div>
        )}
      </ProgressContainer>
    </div>
  );
}
