import type { Meta, StoryObj } from "@storybook/react";
import { TournamentProgress } from "./TournamentProgress";
import type { TournamentMatchData, MessageUsage } from "@/components/chat-types";
import { PreferencesProvider } from "@/preferences/PreferencesProvider";

const meta: Meta<typeof TournamentProgress> = {
  title: "Components/TournamentProgress",
  component: TournamentProgress,
  parameters: {},
  decorators: [
    (Story) => (
      <PreferencesProvider>
        <div className="max-w-2xl">
          <Story />
        </div>
      </PreferencesProvider>
    ),
  ],
};

export default meta;
type Story = StoryObj<typeof TournamentProgress>;

// Sample usage data
const sampleUsage: MessageUsage = {
  inputTokens: 150,
  outputTokens: 250,
  totalTokens: 400,
  cost: 0.0045,
};

// Sample models
const models4 = ["claude-3-opus", "gpt-4-turbo", "gemini-pro", "mistral-large"];
const models8 = [
  "claude-3-opus",
  "gpt-4-turbo",
  "gemini-pro",
  "mistral-large",
  "claude-3-sonnet",
  "gpt-4",
  "gemini-1.5-flash",
  "llama-3-70b",
];

// Sample initial responses
const sampleInitialResponses = models4.map((model) => ({
  model,
  content: `This is a sample response from ${model}. It provides a thoughtful answer to the user's question with relevant details and examples.`,
  usage: sampleUsage,
}));

// Sample completed match for persisted data
const samplePersistedMatch: TournamentMatchData = {
  id: "0-0",
  round: 0,
  competitor1: "claude-3-opus",
  competitor2: "gpt-4-turbo",
  winner: "claude-3-opus",
  judge: "gemini-pro",
  reasoning: "A",
  response1:
    "Claude's response: A detailed and well-structured answer with examples and clear explanations.",
  response2:
    "GPT-4's response: A comprehensive answer covering multiple perspectives and use cases.",
  usage1: sampleUsage,
  usage2: sampleUsage,
  judgeUsage: { inputTokens: 500, outputTokens: 10, totalTokens: 510, cost: 0.005 },
};

/**
 * Done Phase - Tournament complete with 4 models (persisted)
 *
 * Note: The "generating" and "competing" phases require live streaming state from the store.
 * Stories can only show persisted (done) state since they don't have access to the streaming store.
 * Use the actual app to see live streaming progress.
 */
export const Done4Models: Story = {
  args: {
    persistedMetadata: {
      bracket: [models4, ["claude-3-opus", "gemini-pro"], ["claude-3-opus"]],
      matches: [
        { ...samplePersistedMatch, id: "0-0", winner: "claude-3-opus" },
        {
          ...samplePersistedMatch,
          id: "0-1",
          competitor1: "gemini-pro",
          competitor2: "mistral-large",
          winner: "gemini-pro",
        },
        {
          ...samplePersistedMatch,
          id: "1-0",
          round: 1,
          competitor1: "claude-3-opus",
          competitor2: "gemini-pro",
          winner: "claude-3-opus",
          judge: "gpt-4-turbo",
        },
      ],
      initialResponses: sampleInitialResponses,
      eliminatedPerRound: [["gpt-4-turbo", "mistral-large"], ["gemini-pro"]],
      winner: "claude-3-opus",
    },
  },
};

/**
 * Done Phase - Tournament complete with 8 models (3 rounds)
 */
export const Done8Models: Story = {
  args: {
    persistedMetadata: {
      bracket: [
        models8,
        ["claude-3-opus", "gemini-pro", "claude-3-sonnet", "gemini-1.5-flash"],
        ["claude-3-opus", "claude-3-sonnet"],
        ["claude-3-opus"],
      ],
      matches: [
        // Round 0
        {
          ...samplePersistedMatch,
          id: "0-0",
          competitor1: "claude-3-opus",
          competitor2: "gpt-4-turbo",
          winner: "claude-3-opus",
        },
        {
          ...samplePersistedMatch,
          id: "0-1",
          competitor1: "gemini-pro",
          competitor2: "mistral-large",
          winner: "gemini-pro",
        },
        {
          ...samplePersistedMatch,
          id: "0-2",
          competitor1: "claude-3-sonnet",
          competitor2: "gpt-4",
          winner: "claude-3-sonnet",
        },
        {
          ...samplePersistedMatch,
          id: "0-3",
          competitor1: "gemini-1.5-flash",
          competitor2: "llama-3-70b",
          winner: "gemini-1.5-flash",
        },
        // Round 1
        {
          ...samplePersistedMatch,
          id: "1-0",
          round: 1,
          competitor1: "claude-3-opus",
          competitor2: "gemini-pro",
          winner: "claude-3-opus",
        },
        {
          ...samplePersistedMatch,
          id: "1-1",
          round: 1,
          competitor1: "claude-3-sonnet",
          competitor2: "gemini-1.5-flash",
          winner: "claude-3-sonnet",
        },
        // Round 2 (Final)
        {
          ...samplePersistedMatch,
          id: "2-0",
          round: 2,
          competitor1: "claude-3-opus",
          competitor2: "claude-3-sonnet",
          winner: "claude-3-opus",
        },
      ],
      initialResponses: models8.map((model) => ({
        model,
        content: `Response from ${model}...`,
        usage: sampleUsage,
      })),
      eliminatedPerRound: [
        ["gpt-4-turbo", "mistral-large", "gpt-4", "llama-3-70b"],
        ["gemini-pro", "gemini-1.5-flash"],
        ["claude-3-sonnet"],
      ],
      winner: "claude-3-opus",
    },
  },
};

/**
 * Done Phase - With persisted data (loaded from saved conversation)
 */
export const PersistedDone: Story = {
  args: {
    persistedMetadata: {
      bracket: [models4, ["claude-3-opus", "gemini-pro"], ["claude-3-opus"]],
      matches: [
        {
          id: "0-0",
          round: 0,
          competitor1: "claude-3-opus",
          competitor2: "gpt-4-turbo",
          winner: "claude-3-opus",
          judge: "gemini-pro",
          reasoning: "A",
          response1: "Claude's response with excellent structure and examples.",
          response2: "GPT-4's response covering various aspects.",
          usage1: sampleUsage,
          usage2: sampleUsage,
          judgeUsage: { inputTokens: 500, outputTokens: 10, totalTokens: 510, cost: 0.005 },
        },
        {
          id: "0-1",
          round: 0,
          competitor1: "gemini-pro",
          competitor2: "mistral-large",
          winner: "gemini-pro",
          judge: "claude-3-opus",
          reasoning: "A",
          response1: "Gemini's insightful response.",
          response2: "Mistral's concise response.",
          usage1: sampleUsage,
          usage2: sampleUsage,
          judgeUsage: { inputTokens: 500, outputTokens: 10, totalTokens: 510, cost: 0.005 },
        },
        {
          id: "1-0",
          round: 1,
          competitor1: "claude-3-opus",
          competitor2: "gemini-pro",
          winner: "claude-3-opus",
          judge: "gpt-4-turbo",
          reasoning: "A",
          response1: "Claude's final comprehensive response.",
          response2: "Gemini's final thoughtful response.",
          usage1: sampleUsage,
          usage2: sampleUsage,
          judgeUsage: { inputTokens: 500, outputTokens: 10, totalTokens: 510, cost: 0.005 },
        },
      ] as TournamentMatchData[],
      winner: "claude-3-opus",
      eliminatedPerRound: [["gpt-4-turbo", "mistral-large"], ["gemini-pro"]],
    },
  },
};

/**
 * Done Phase - With expanded details showing match comparisons
 */
export const DoneExpanded: Story = {
  args: {
    ...Done4Models.args,
  },
  play: async ({ canvasElement }) => {
    // Auto-expand the details
    const button = canvasElement.querySelector("button");
    if (button && button.textContent?.includes("Show details")) {
      button.click();
    }
  },
};

/**
 * Empty State - No data provided (should not render)
 */
export const EmptyState: Story = {
  args: {},
};
