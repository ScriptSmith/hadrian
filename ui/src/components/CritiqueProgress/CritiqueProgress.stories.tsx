import type { Meta, StoryObj } from "@storybook/react";
import { CritiqueProgress } from "./CritiqueProgress";

const meta: Meta<typeof CritiqueProgress> = {
  title: "Components/CritiqueProgress",
  component: CritiqueProgress,
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
type Story = StoryObj<typeof CritiqueProgress>;

const mockCritiques = [
  {
    model: "openai/gpt-4o",
    content:
      "The response is good but could be improved by adding more specific examples and addressing edge cases. Here are my specific suggestions:\n\n1. Add concrete examples\n2. Consider error handling\n3. Discuss performance implications",
    usage: { inputTokens: 400, outputTokens: 150, totalTokens: 550, cost: 0.015 },
  },
  {
    model: "google/gemini-pro",
    content:
      "Consider restructuring the explanation to be more beginner-friendly. The technical depth is appropriate but the order of concepts could be improved. Start with the basics before diving into advanced topics.",
    usage: { inputTokens: 380, outputTokens: 120, totalTokens: 500, cost: 0.008 },
  },
];

/**
 * Shows completed critique with initial response and critiques.
 * This is the typical view for persisted messages in the chat history.
 */
export const Done: Story = {
  args: {
    persistedMetadata: {
      primaryModel: "anthropic/claude-3-opus",
      initialResponse:
        "Here is my initial response to the question. This is a longer response that demonstrates how the content would appear in the card view, including multiple sentences and potentially multiple paragraphs of content.",
      initialUsage: { inputTokens: 150, outputTokens: 200, totalTokens: 350, cost: 0.012 },
      critiques: mockCritiques,
    },
  },
};

/**
 * Shows completed critique with a single critic.
 */
export const SingleCritique: Story = {
  args: {
    persistedMetadata: {
      primaryModel: "anthropic/claude-3-opus",
      initialResponse: "Initial response from the primary model.",
      initialUsage: { inputTokens: 100, outputTokens: 150, totalTokens: 250, cost: 0.008 },
      critiques: [
        {
          model: "openai/gpt-4o",
          content: "A single critique providing feedback on the initial response.",
          usage: { inputTokens: 300, outputTokens: 100, totalTokens: 400, cost: 0.012 },
        },
      ],
    },
  },
};

/**
 * Shows completed critique with longer responses including markdown.
 */
export const LongResponses: Story = {
  args: {
    persistedMetadata: {
      primaryModel: "anthropic/claude-3-opus",
      initialResponse: `# Initial Analysis

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

In summary, this initial response provides a solid foundation for critique and revision.`,
      initialUsage: { inputTokens: 200, outputTokens: 400, totalTokens: 600, cost: 0.025 },
      critiques: [
        {
          model: "openai/gpt-4o",
          content: `## Strengths

The analysis covers important ground and provides a good structure. The technical details section is particularly useful.

## Areas for Improvement

1. **More Examples**: The first point would benefit from concrete examples
2. **Code Samples**: Consider adding code snippets to illustrate concepts
3. **Edge Cases**: Some edge cases are not addressed

## Specific Suggestions

\`\`\`python
# Consider adding examples like this
def example():
    pass
\`\`\``,
          usage: { inputTokens: 700, outputTokens: 300, totalTokens: 1000, cost: 0.028 },
        },
        {
          model: "google/gemini-pro",
          content: `## Structural Feedback

The overall structure is logical, but I'd recommend:

1. Starting with a brief summary for readers in a hurry
2. Adding a "Prerequisites" section before diving into technical details
3. Including a "Related Topics" section at the end

## Tone and Accessibility

The technical depth is appropriate for an expert audience, but consider:

- Adding simpler explanations for key concepts
- Using more analogies to make abstract concepts concrete
- Breaking up longer paragraphs for better readability`,
          usage: { inputTokens: 650, outputTokens: 280, totalTokens: 930, cost: 0.018 },
        },
      ],
    },
  },
};

/**
 * Shows completed critique without initial response stored (critiques only).
 */
export const CritiquesOnly: Story = {
  args: {
    persistedMetadata: {
      primaryModel: "anthropic/claude-3-opus",
      critiques: mockCritiques,
    },
  },
};
