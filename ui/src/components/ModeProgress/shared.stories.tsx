import type { Meta, StoryObj } from "@storybook/react";
import { Combine } from "lucide-react";

import { PreferencesProvider } from "@/preferences/PreferencesProvider";
import {
  ProgressContainer,
  StatusBadge,
  ModeHeader,
  ModelBadge,
  UsageSummary,
  ResponseCard,
  ExpandButton,
} from "./shared";

/**
 * Shared building block components for mode progress indicators.
 *
 * These components are used by various mode progress displays like
 * ChainProgress, SynthesisProgress, RefinementProgress, etc.
 */
const meta: Meta = {
  title: "Chat/ModeProgress/Shared",
  parameters: {
    layout: "centered",
    docs: {
      description: {
        component: "Building block components for creating consistent mode progress UIs.",
      },
    },
  },
  decorators: [
    (Story) => (
      <PreferencesProvider>
        <div className="p-4 w-[500px]">
          <Story />
        </div>
      </PreferencesProvider>
    ),
  ],
};

export default meta;

/**
 * StatusBadge - Small pill showing current phase status.
 */
export const StatusBadges: StoryObj = {
  render: () => (
    <div className="flex flex-wrap gap-2">
      <StatusBadge text="GATHERING" variant="initial" />
      <StatusBadge text="SYNTHESIZING" variant="active" />
      <StatusBadge text="COMPLETE" variant="complete" />
      <StatusBadge text="WARNING" variant="warning" />
    </div>
  ),
};

/**
 * ModeHeader - Consistent header with name and optional badge.
 *
 * Note: Icons are rendered by ProgressContainer (with loading spinner support),
 * so ModeHeader only handles the name and badge.
 */
export const ModeHeaders: StoryObj = {
  render: () => (
    <div className="space-y-4">
      <ModeHeader name="Synthesized" badge={<StatusBadge text="GATHERING" variant="initial" />} />
      <ModeHeader name="Refined" badge={<StatusBadge text="REFINING" variant="active" />} />
      <ModeHeader name="Chained" badge={<StatusBadge text="COMPLETE" variant="complete" />} />
    </div>
  ),
};

/**
 * ModelBadge - Small badge showing a model name with optional status indicator.
 */
export const ModelBadges: StoryObj = {
  render: () => (
    <div className="space-y-4">
      <div className="flex flex-wrap gap-2">
        <ModelBadge model="claude-3-opus" variant="default" />
        <ModelBadge model="gpt-4-turbo" variant="primary" />
        <ModelBadge model="gemini-pro" variant="blue" />
        <ModelBadge model="mistral-large" variant="amber" />
        <ModelBadge model="llama-3.1-70b" variant="orange" />
      </div>
      <div className="flex flex-wrap gap-2">
        <ModelBadge model="claude-3-opus" variant="primary" showCheck />
        <ModelBadge model="gpt-4-turbo" variant="default" showLoading />
      </div>
    </div>
  ),
};

/**
 * UsageSummary - Display token count and cost in a compact format.
 */
export const UsageSummaries: StoryObj = {
  render: () => (
    <div className="space-y-2">
      <UsageSummary totalTokens={1250} totalCost={0.0125} />
      <UsageSummary totalTokens={5000} totalCost={0.05} label="Total" />
      <UsageSummary totalTokens={750} />
    </div>
  ),
};

/**
 * ExpandButton - Reusable expand/collapse button.
 */
export const ExpandButtons: StoryObj = {
  render: () => (
    <div className="flex gap-4">
      <ExpandButton isExpanded={false} onToggle={() => {}} />
      <ExpandButton isExpanded={true} onToggle={() => {}} />
      <ExpandButton
        isExpanded={false}
        onToggle={() => {}}
        collapsedLabel="Show sources"
        expandedLabel="Hide sources"
      />
    </div>
  ),
};

/**
 * ResponseCard - Expandable card showing a response with truncation support.
 */
export const ResponseCards: StoryObj = {
  render: () => (
    <div className="space-y-4">
      <ResponseCard
        title="claude-3-opus"
        content="This is a short response from the model."
        usage={{ inputTokens: 50, outputTokens: 100, totalTokens: 150 }}
      />
      <ResponseCard
        title="gpt-4-turbo"
        content="This is a much longer response that demonstrates the truncation behavior. When content exceeds the preview length, it will be truncated with an expand button. Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua."
        usage={{ inputTokens: 100, outputTokens: 250, totalTokens: 350 }}
        variant="blue"
      />
      <ResponseCard
        title="gemini-pro"
        content="Another response with the orange variant styling."
        usage={{ inputTokens: 80, outputTokens: 120, totalTokens: 200 }}
        variant="orange"
      />
    </div>
  ),
};

/**
 * ProgressContainer - Full container example with all elements.
 */
export const ProgressContainerComplete: StoryObj = {
  render: () => (
    <ProgressContainer
      phase="complete"
      icon={Combine}
      header={
        <ModeHeader name="Synthesized" badge={<StatusBadge text="COMPLETE" variant="complete" />} />
      }
      expandableSection={
        <ResponseCard
          title="claude-3-opus"
          content="The synthesized response content."
          usage={{ inputTokens: 100, outputTokens: 200, totalTokens: 300 }}
        />
      }
      showExpandable
    >
      <div className="mt-2">
        <div className="flex gap-2">
          <ModelBadge model="claude-3-opus" variant="primary" showCheck />
          <ModelBadge model="gpt-4-turbo" variant="primary" showCheck />
        </div>
        <p className="text-[10px] text-muted-foreground mt-1">2 sources synthesized</p>
      </div>
    </ProgressContainer>
  ),
};

/**
 * ProgressContainer - Loading state example.
 */
export const ProgressContainerLoading: StoryObj = {
  render: () => (
    <ProgressContainer
      phase="initial"
      isLoading
      icon={Combine}
      header={
        <ModeHeader name="Synthesized" badge={<StatusBadge text="GATHERING" variant="initial" />} />
      }
    >
      <div className="mt-2">
        <div className="flex gap-2">
          <ModelBadge model="claude-3-opus" variant="primary" showCheck />
          <ModelBadge model="gpt-4-turbo" variant="default" showLoading />
        </div>
        <p className="text-[10px] text-muted-foreground mt-1">1/2 responses received</p>
      </div>
    </ProgressContainer>
  ),
};
