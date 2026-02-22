import type { Meta, StoryObj } from "@storybook/react";

import { ExplainerProgress } from "./ExplainerProgress";
import type { ExplanationData } from "@/components/chat-types";

const meta = {
  title: "Chat/ExplainerProgress",
  component: ExplainerProgress,
  parameters: {},
  decorators: [
    (Story) => (
      <div className="max-w-2xl">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof ExplainerProgress>;

export default meta;
type Story = StoryObj<typeof meta>;

const mockExplanations: ExplanationData[] = [
  {
    level: "expert",
    model: "anthropic/claude-3.5-sonnet",
    content:
      "Neural networks utilize gradient descent optimization with backpropagation to minimize a differentiable loss function. The architecture comprises interconnected layers of artificial neurons with learnable weight matrices and bias vectors, applying non-linear activation functions to enable universal function approximation. Recent advances include attention mechanisms, residual connections, and normalization techniques that enable training of very deep architectures.",
    usage: { inputTokens: 150, outputTokens: 120, totalTokens: 270, cost: 0.0015 },
  },
  {
    level: "intermediate",
    model: "openai/gpt-4o",
    content:
      "Neural networks are computer programs that learn from examples. They have layers of connected units that process information. During training, the network adjusts its internal settings to get better at its task, like recognizing images or understanding text. Think of it like a student learning from practice problems.",
    usage: { inputTokens: 180, outputTokens: 90, totalTokens: 270, cost: 0.0012 },
  },
  {
    level: "beginner",
    model: "openai/gpt-4o-mini",
    content:
      "Imagine teaching a dog to do tricks. You show the dog what to do, and when it does it right, you give it a treat. Over time, the dog learns. Neural networks work similarly - you show them lots of examples, and they gradually learn patterns from those examples. They're like very patient students that can look at millions of examples!",
    usage: { inputTokens: 200, outputTokens: 100, totalTokens: 300, cost: 0.0005 },
  },
];

/**
 * Done state - all explanations complete.
 * This is the typical view for persisted messages in the chat history.
 */
export const Done: Story = {
  args: {
    persistedMetadata: {
      explanations: mockExplanations,
      levels: ["expert", "intermediate", "beginner"],
      aggregateUsage: {
        inputTokens: 530,
        outputTokens: 310,
        totalTokens: 840,
        cost: 0.0032,
      },
    },
  },
};

/**
 * Quantum computing explanations
 */
export const QuantumComputing: Story = {
  args: {
    persistedMetadata: {
      explanations: [
        {
          level: "expert",
          model: "anthropic/claude-3.5-sonnet",
          content:
            "Quantum computing leverages quantum mechanical phenomena such as superposition and entanglement to perform computations. Qubits can exist in multiple states simultaneously, enabling massive parallelism for certain problem classes like integer factorization and optimization.",
          usage: { inputTokens: 150, outputTokens: 120, totalTokens: 270, cost: 0.0015 },
        },
        {
          level: "intermediate",
          model: "openai/gpt-4o",
          content:
            "Quantum computers use the strange rules of quantum physics to solve problems differently than regular computers. While normal computer bits are either 0 or 1, quantum bits (qubits) can be both at once, allowing them to explore many possibilities simultaneously.",
          usage: { inputTokens: 180, outputTokens: 90, totalTokens: 270, cost: 0.0012 },
        },
        {
          level: "beginner",
          model: "openai/gpt-4o-mini",
          content:
            "Regular computers are like reading one book at a time. Quantum computers are like being able to read every book in a library at the same time! They use special physics tricks to check many answers at once, which makes them really fast for certain puzzles.",
          usage: { inputTokens: 200, outputTokens: 100, totalTokens: 300, cost: 0.0005 },
        },
      ],
      levels: ["expert", "intermediate", "beginner"],
      aggregateUsage: {
        inputTokens: 530,
        outputTokens: 310,
        totalTokens: 840,
        cost: 0.0032,
      },
    },
  },
};

/**
 * More audience levels - expert through child
 */
export const ManyLevels: Story = {
  args: {
    persistedMetadata: {
      explanations: [
        {
          level: "expert",
          model: "anthropic/claude-3.5-sonnet",
          content:
            "The immune system comprises innate and adaptive components. Innate immunity provides non-specific defense through physical barriers, phagocytes, and complement proteins. Adaptive immunity features antigen-specific lymphocytes (T and B cells) capable of immunological memory.",
          usage: { inputTokens: 100, outputTokens: 80, totalTokens: 180, cost: 0.001 },
        },
        {
          level: "intermediate",
          model: "openai/gpt-4o",
          content:
            "Your immune system has two main parts. The first line of defense attacks anything foreign. The second line learns to recognize specific threats and remembers them for next time. White blood cells are the main soldiers in this defense system.",
          usage: { inputTokens: 120, outputTokens: 70, totalTokens: 190, cost: 0.0008 },
        },
        {
          level: "beginner",
          model: "openai/gpt-4o-mini",
          content:
            "Your body has tiny defenders called white blood cells. When germs try to make you sick, these defenders fight them off. Some of them remember the germs so they can fight them faster next time - that's why you usually only get chicken pox once!",
          usage: { inputTokens: 140, outputTokens: 60, totalTokens: 200, cost: 0.0004 },
        },
        {
          level: "child",
          model: "openai/gpt-4o-mini",
          content:
            "Your body has tiny superhero soldiers inside! When bad guys called germs try to make you feel yucky, your superhero soldiers zoom around and fight them. And guess what? Your superheroes have really good memories - if a bad guy comes back, they remember how to beat them super fast!",
          usage: { inputTokens: 160, outputTokens: 70, totalTokens: 230, cost: 0.0005 },
        },
      ],
      levels: ["expert", "intermediate", "beginner", "child"],
      aggregateUsage: {
        inputTokens: 520,
        outputTokens: 280,
        totalTokens: 800,
        cost: 0.0027,
      },
    },
  },
};

/**
 * Single level - just one explanation
 */
export const SingleLevel: Story = {
  args: {
    persistedMetadata: {
      explanations: [
        {
          level: "beginner",
          model: "anthropic/claude-3.5-sonnet",
          content:
            "Artificial Intelligence is when we teach computers to do things that usually require human thinking. It's like teaching a very obedient student who can practice millions of times without getting tired. AI helps us with things like voice assistants, recommendation systems, and even driving cars!",
          usage: { inputTokens: 100, outputTokens: 80, totalTokens: 180, cost: 0.001 },
        },
      ],
      levels: ["beginner"],
      aggregateUsage: {
        inputTokens: 100,
        outputTokens: 80,
        totalTokens: 180,
        cost: 0.001,
      },
    },
  },
};
