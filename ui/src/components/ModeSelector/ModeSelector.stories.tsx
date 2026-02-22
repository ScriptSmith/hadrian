import type { Meta, StoryObj } from "@storybook/react";
import { useState } from "react";

import type { ConversationMode } from "@/components/chat-types";

import { ModeSelector } from "./ModeSelector";

const meta: Meta<typeof ModeSelector> = {
  title: "Chat/ModeSelector",
  component: ModeSelector,
  parameters: {
    layout: "centered",
  },
  argTypes: {
    mode: {
      control: "select",
      options: [
        "multiple",
        "chained",
        "routed",
        "synthesized",
        "refined",
        "critiqued",
        "elected",
        "tournament",
        "consensus",
        "debated",
        "council",
        "hierarchical",
        "alloyed",
        "scattershot",
        "confidence-weighted",
        "evolutionary",
        "explainer",
      ],
    },
    selectedModelCount: {
      control: { type: "number", min: 0, max: 6 },
    },
    isStreaming: {
      control: "boolean",
    },
  },
};

export default meta;
type Story = StoryObj<typeof meta>;

/** Interactive version that maintains its own state */
function InteractiveModeSelector({
  initialMode = "multiple",
  selectedModelCount = 1,
  isStreaming = false,
}: {
  initialMode?: ConversationMode;
  selectedModelCount?: number;
  isStreaming?: boolean;
}) {
  const [mode, setMode] = useState<ConversationMode>(initialMode);
  return (
    <ModeSelector
      mode={mode}
      onModeChange={setMode}
      selectedModelCount={selectedModelCount}
      isStreaming={isStreaming}
    />
  );
}

export const Default: Story = {
  render: () => <InteractiveModeSelector />,
};

export const MultipleMode: Story = {
  args: {
    mode: "multiple",
    selectedModelCount: 3,
    isStreaming: false,
    onModeChange: () => {},
  },
};

export const ChainedMode: Story = {
  render: () => <InteractiveModeSelector initialMode="chained" selectedModelCount={3} />,
};

export const WithSingleModel: Story = {
  render: () => (
    <div className="space-y-4">
      <p className="text-sm text-muted-foreground">
        With only 1 model selected, modes requiring 2+ models are disabled.
      </p>
      <InteractiveModeSelector selectedModelCount={1} />
    </div>
  ),
};

export const WithTwoModels: Story = {
  render: () => (
    <div className="space-y-4">
      <p className="text-sm text-muted-foreground">
        With 2 models selected, most modes are available.
      </p>
      <InteractiveModeSelector selectedModelCount={2} />
    </div>
  ),
};

export const WithFourModels: Story = {
  render: () => (
    <div className="space-y-4">
      <p className="text-sm text-muted-foreground">
        With 4+ models selected, all modes are available (including tournament).
      </p>
      <InteractiveModeSelector selectedModelCount={4} />
    </div>
  ),
};

export const WhileStreaming: Story = {
  render: () => (
    <div className="space-y-4">
      <p className="text-sm text-muted-foreground">Mode selector is disabled during streaming.</p>
      <InteractiveModeSelector selectedModelCount={3} isStreaming={true} />
    </div>
  ),
};

/** Shows modes from different phases */
export const DifferentPhases: Story = {
  render: () => (
    <div className="space-y-4">
      <p className="text-sm text-muted-foreground">
        Each phase has a colored icon. Hover over tabs to see phase names.
      </p>
      <div className="flex flex-wrap gap-4">
        <div className="text-center">
          <p className="text-xs text-muted-foreground mb-2">Core (blue)</p>
          <InteractiveModeSelector initialMode="multiple" selectedModelCount={4} />
        </div>
        <div className="text-center">
          <p className="text-xs text-muted-foreground mb-2">Synthesis (violet)</p>
          <InteractiveModeSelector initialMode="synthesized" selectedModelCount={4} />
        </div>
        <div className="text-center">
          <p className="text-xs text-muted-foreground mb-2">Competitive (amber)</p>
          <InteractiveModeSelector initialMode="elected" selectedModelCount={4} />
        </div>
        <div className="text-center">
          <p className="text-xs text-muted-foreground mb-2">Advanced (emerald)</p>
          <InteractiveModeSelector initialMode="debated" selectedModelCount={4} />
        </div>
        <div className="text-center">
          <p className="text-xs text-muted-foreground mb-2">Experimental (rose)</p>
          <InteractiveModeSelector initialMode="scattershot" selectedModelCount={4} />
        </div>
      </div>
    </div>
  ),
};

export const InChatHeader: Story = {
  render: () => (
    <div className="flex items-center gap-3 rounded-lg border p-3 bg-background/95">
      <h1 className="font-semibold text-sm">New Chat</h1>
      <InteractiveModeSelector selectedModelCount={3} />
      <span className="text-xs text-muted-foreground px-2 py-1 rounded bg-muted/50">
        2.5k tokens
      </span>
    </div>
  ),
};
