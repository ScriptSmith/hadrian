import type { Meta, StoryObj } from "@storybook/react";

import { ResponseActions } from "./ResponseActions";

const meta: Meta<typeof ResponseActions> = {
  title: "Chat/ResponseActions",
  component: ResponseActions,
  parameters: {
    layout: "centered",
  },

  args: {
    content: "This is sample content that would be copied.",
    onSelectBest: () => {},
    onRegenerate: () => {},
    onExpand: () => {},
    onHide: () => {},
    onSpeak: () => {},
    onStopSpeaking: () => {},
  },
  decorators: [
    (Story) => (
      <div className="group/card p-4 border rounded-lg bg-card hover:shadow-md transition-shadow">
        <p className="text-sm text-muted-foreground mb-2">
          Hover over this card to see secondary buttons slide in (desktop only):
        </p>
        <Story />
      </div>
    ),
  ],
};

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  args: {
    canSelectBest: true,
    canExpand: true,
  },
};

export const WithEditAndDebug: Story = {
  args: {
    canSelectBest: true,
    canExpand: true,
    onEdit: () => {},
    onOpenDebug: () => {},
  },
};

export const SelectedAsBest: Story = {
  args: {
    isSelectedBest: true,
    canSelectBest: true,
    canExpand: true,
  },
};

export const Expanded: Story = {
  args: {
    isExpanded: true,
    canSelectBest: true,
    canExpand: true,
  },
};

export const SingleResponse: Story = {
  args: {
    canSelectBest: false,
    canExpand: false,
  },
};

export const PrimaryOnly: Story = {
  args: {
    config: {
      showSelectBest: false,
      showRegenerate: true,
      showCopy: true,
      showExpand: false,
      showHide: false,
      showSpeak: false,
    },
    onHide: undefined,
    onSpeak: undefined,
    onExpand: undefined,
    onSelectBest: undefined,
  },
};

export const AllActions: Story = {
  args: {
    canSelectBest: true,
    canExpand: true,
    onEdit: () => {},
    onOpenDebug: () => {},
    speakingState: "idle",
  },
};

export const Speaking: Story = {
  args: {
    canSelectBest: true,
    canExpand: true,
    speakingState: "playing",
  },
};

export const SpeakLoading: Story = {
  args: {
    canSelectBest: true,
    canExpand: true,
    speakingState: "loading",
  },
};

export const NoRegenerate: Story = {
  args: {
    canSelectBest: true,
    canExpand: true,
    config: {
      showSelectBest: true,
      showRegenerate: false,
      showCopy: true,
      showExpand: true,
    },
  },
};

export const CopyOnly: Story = {
  args: {
    config: {
      showSelectBest: false,
      showRegenerate: false,
      showCopy: true,
      showExpand: false,
      showHide: false,
      showSpeak: false,
    },
    onHide: undefined,
    onSpeak: undefined,
    onExpand: undefined,
    onSelectBest: undefined,
    onRegenerate: undefined,
  },
};

export const AllDisabled: Story = {
  args: {
    config: {
      showSelectBest: false,
      showRegenerate: false,
      showCopy: false,
      showExpand: false,
      showHide: false,
      showSpeak: false,
    },
    onHide: undefined,
    onSpeak: undefined,
    onExpand: undefined,
    onSelectBest: undefined,
    onRegenerate: undefined,
  },
};
