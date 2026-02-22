import type { Meta, StoryObj } from "@storybook/react";
import { RefinementProgress } from "./RefinementProgress";

const meta: Meta<typeof RefinementProgress> = {
  title: "Components/RefinementProgress",
  component: RefinementProgress,
  parameters: {},
  decorators: [
    (Story) => (
      <div className="max-w-md">
        <Story />
      </div>
    ),
  ],
};

export default meta;
type Story = StoryObj<typeof RefinementProgress>;

const mockRounds = [
  {
    model: "anthropic/claude-3-opus",
    content:
      "Here is the initial response that provides a comprehensive overview of the topic. This is a longer response that demonstrates how the content would appear in the card view, including multiple sentences and potentially multiple paragraphs of content.",
    usage: { inputTokens: 150, outputTokens: 200, totalTokens: 350, cost: 0.012 },
  },
  {
    model: "openai/gpt-4o",
    content:
      "Building on the previous response, I'd like to add some additional context and refine certain points. The initial analysis was good but missed some key aspects that I'll address here.",
    usage: { inputTokens: 400, outputTokens: 250, totalTokens: 650, cost: 0.018 },
  },
  {
    model: "anthropic/claude-3-sonnet",
    content:
      "Taking the best elements from both previous responses, here is a refined and improved answer that synthesizes the insights and corrects any minor issues.",
    usage: { inputTokens: 700, outputTokens: 180, totalTokens: 880, cost: 0.008 },
  },
];

/**
 * Shows completed refinement with three rounds.
 * This is the typical view for persisted messages in the chat history.
 */
export const Done: Story = {
  args: {
    persistedMetadata: {
      currentRound: 2,
      totalRounds: 3,
      rounds: mockRounds,
    },
  },
};

/**
 * Shows completed refinement with only two rounds.
 */
export const TwoRounds: Story = {
  args: {
    persistedMetadata: {
      currentRound: 1,
      totalRounds: 2,
      rounds: [
        {
          model: "anthropic/claude-3-opus",
          content: "Initial response from the first model.",
          usage: { inputTokens: 100, outputTokens: 150, totalTokens: 250, cost: 0.008 },
        },
        {
          model: "openai/gpt-4o",
          content: "Refined response improving on the initial.",
          usage: { inputTokens: 300, outputTokens: 180, totalTokens: 480, cost: 0.015 },
        },
      ],
    },
  },
};

/**
 * Shows completed refinement with longer responses including markdown.
 */
export const LongResponse: Story = {
  args: {
    persistedMetadata: {
      currentRound: 2,
      totalRounds: 3,
      rounds: [
        {
          model: "anthropic/claude-3-opus",
          content: `# Initial Analysis

This is a comprehensive analysis of the topic at hand. The initial response covers several key areas:

1. **First Point**: A detailed explanation of the first aspect
2. **Second Point**: Analysis of the second consideration
3. **Third Point**: Discussion of implementation details

## Technical Details

The technical implementation requires careful consideration of:

- Performance optimization
- Memory management
- Error handling
- User experience

## Conclusion

In summary, this initial response provides a solid foundation for further refinement.`,
          usage: { inputTokens: 200, outputTokens: 400, totalTokens: 600, cost: 0.025 },
        },
        {
          model: "openai/gpt-4o",
          content: `Building on the initial analysis, I've identified several areas for improvement:

## Enhanced Points

The first point could be strengthened by adding more concrete examples. Consider the following:

\`\`\`python
def example_function():
    # Implementation details
    return result
\`\`\`

## Additional Considerations

There are some aspects that weren't fully addressed in the initial response:

- Security implications
- Scalability concerns
- Testing strategies

This refinement adds practical value to the discussion.`,
          usage: { inputTokens: 700, outputTokens: 350, totalTokens: 1050, cost: 0.032 },
        },
        {
          model: "anthropic/claude-3-sonnet",
          content: `# Final Refined Response

Taking the best elements from both previous responses, here is the comprehensive answer:

## Key Takeaways

1. The analysis is sound and well-structured
2. The code examples provide practical guidance
3. All major considerations have been addressed

## Implementation Recommendation

Based on the refined analysis, I recommend the following approach...`,
          usage: { inputTokens: 1100, outputTokens: 200, totalTokens: 1300, cost: 0.015 },
        },
      ],
    },
  },
};

/**
 * Shows completed single-round refinement (no expand button).
 */
export const SingleRound: Story = {
  args: {
    persistedMetadata: {
      currentRound: 0,
      totalRounds: 1,
      rounds: [
        {
          model: "anthropic/claude-3-opus",
          content: "A single round of refinement - no history to expand.",
          usage: { inputTokens: 100, outputTokens: 150, totalTokens: 250, cost: 0.008 },
        },
      ],
    },
  },
};
