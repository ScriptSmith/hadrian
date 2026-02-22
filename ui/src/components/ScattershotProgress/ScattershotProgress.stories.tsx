import type { Meta, StoryObj } from "@storybook/react";

import { ScattershotProgress } from "./ScattershotProgress";
import type { ScattershotVariationData } from "@/components/chat-types";

const meta = {
  title: "Components/ScattershotProgress",
  component: ScattershotProgress,
  parameters: {},
  decorators: [
    (Story) => (
      <div className="max-w-2xl">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof ScattershotProgress>;

export default meta;
type Story = StoryObj<typeof meta>;

const mockVariations: ScattershotVariationData[] = [
  {
    id: "model__variation_0",
    index: 0,
    params: { temperature: 0.0 },
    label: "temp=0.0",
    content:
      "This is a deterministic response with temperature set to 0. It provides factual, consistent output.",
    usage: { inputTokens: 50, outputTokens: 100, totalTokens: 150, cost: 0.002 },
  },
  {
    id: "model__variation_1",
    index: 1,
    params: { temperature: 0.5 },
    label: "temp=0.5",
    content:
      "This is a balanced response with moderate creativity. It maintains coherence while allowing some variation.",
    usage: { inputTokens: 50, outputTokens: 120, totalTokens: 170, cost: 0.0023 },
  },
  {
    id: "model__variation_2",
    index: 2,
    params: { temperature: 1.0 },
    label: "temp=1.0",
    content:
      "This is a creative response! The higher temperature allows for more varied and imaginative output. Sometimes the results are surprising and novel.",
    usage: { inputTokens: 50, outputTokens: 150, totalTokens: 200, cost: 0.003 },
  },
  {
    id: "model__variation_3",
    index: 3,
    params: { temperature: 1.5, topP: 0.9 },
    label: "temp=1.5, top_p=0.9",
    content:
      "Experimental mode! With very high temperature and nucleus sampling, this response explores unconventional ideas and phrasing. Results can be highly creative but sometimes less coherent.",
    usage: { inputTokens: 50, outputTokens: 180, totalTokens: 230, cost: 0.0035 },
  },
];

/**
 * Done state showing completed scattershot variations.
 * This is the typical view for persisted messages in the chat history.
 */
export const Done: Story = {
  args: {
    persistedMetadata: {
      variations: mockVariations,
      targetModel: "openai/gpt-4o",
      aggregateUsage: {
        inputTokens: 200,
        outputTokens: 550,
        totalTokens: 750,
        cost: 0.0108,
      },
    },
  },
};

/**
 * Two variations - simple temperature comparison
 */
export const TwoVariations: Story = {
  args: {
    persistedMetadata: {
      variations: [
        {
          id: "model__variation_0",
          index: 0,
          params: { temperature: 0.0 },
          label: "temp=0.0",
          content: "Precise, deterministic response.",
          usage: { inputTokens: 30, outputTokens: 50, totalTokens: 80, cost: 0.001 },
        },
        {
          id: "model__variation_1",
          index: 1,
          params: { temperature: 1.0 },
          label: "temp=1.0",
          content: "Creative, varied response with more flair!",
          usage: { inputTokens: 30, outputTokens: 60, totalTokens: 90, cost: 0.0012 },
        },
      ],
      targetModel: "anthropic/claude-3-opus",
      aggregateUsage: {
        inputTokens: 60,
        outputTokens: 110,
        totalTokens: 170,
        cost: 0.0022,
      },
    },
  },
};

/**
 * Many variations - testing with multiple parameter combinations
 */
export const ManyVariations: Story = {
  args: {
    persistedMetadata: {
      variations: [
        ...mockVariations,
        {
          id: "model__variation_4",
          index: 4,
          params: { temperature: 0.3 },
          label: "temp=0.3",
          content: "Conservative response with low creativity.",
          usage: { inputTokens: 50, outputTokens: 90, totalTokens: 140, cost: 0.0019 },
        },
        {
          id: "model__variation_5",
          index: 5,
          params: { temperature: 0.7 },
          label: "temp=0.7",
          content: "Moderately creative response finding good balance.",
          usage: { inputTokens: 50, outputTokens: 110, totalTokens: 160, cost: 0.0022 },
        },
      ],
      targetModel: "openai/gpt-4o",
      aggregateUsage: {
        inputTokens: 300,
        outputTokens: 750,
        totalTokens: 1050,
        cost: 0.0149,
      },
    },
  },
};

/**
 * With different target model
 */
export const ClaudeModel: Story = {
  args: {
    persistedMetadata: {
      variations: mockVariations.slice(0, 3),
      targetModel: "anthropic/claude-3-sonnet",
      aggregateUsage: {
        inputTokens: 150,
        outputTokens: 370,
        totalTokens: 520,
        cost: 0.0073,
      },
    },
  },
};
