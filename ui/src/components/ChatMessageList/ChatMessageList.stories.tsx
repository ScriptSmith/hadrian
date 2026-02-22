import type { Meta, StoryObj } from "@storybook/react";
import { expect, within } from "storybook/test";
import { useEffect } from "react";

import type { ChatMessage, ModelResponse } from "@/components/chat-types";
import { TooltipProvider } from "@/components/Tooltip/Tooltip";
import { PreferencesProvider } from "@/preferences/PreferencesProvider";
import { useConversationStore } from "@/stores/conversationStore";
import { useStreamingStore } from "@/stores/streamingStore";
import { useChatUIStore } from "@/stores/chatUIStore";

import { ChatMessageList } from "./ChatMessageList";

// Helper component to set up store state for stories
function StoreSetup({
  messages,
  modelResponses,
  selectedModels,
  disabledModels,
  children,
}: {
  messages: ChatMessage[];
  modelResponses?: ModelResponse[];
  selectedModels: string[];
  disabledModels?: string[];
  children: React.ReactNode;
}) {
  const { setMessages, setSelectedModels } = useConversationStore();
  const streamingStore = useStreamingStore();
  const { setDisabledModels } = useChatUIStore();

  useEffect(() => {
    setMessages(messages);
    setSelectedModels(selectedModels);
    setDisabledModels(disabledModels ?? []);

    // Set up streaming responses if provided
    if (modelResponses && modelResponses.length > 0) {
      const models = modelResponses.map((r) => r.model);
      streamingStore.initStreaming(models);
      for (const response of modelResponses) {
        if (response.content) {
          streamingStore.setContent(response.model, response.content);
        }
        if (!response.isStreaming) {
          streamingStore.completeStream(response.model, response.usage);
        }
      }
    }

    return () => {
      streamingStore.clearStreams();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps -- Static story data, run once on mount
  }, []);

  return <>{children}</>;
}

const meta: Meta<typeof ChatMessageList> = {
  title: "Chat/ChatMessageList",
  component: ChatMessageList,
  parameters: {
    layout: "fullscreen",
  },
  decorators: [
    (Story) => (
      <PreferencesProvider>
        <TooltipProvider>
          <div className="h-[600px] flex flex-col">
            <Story />
          </div>
        </TooltipProvider>
      </PreferencesProvider>
    ),
  ],
};

export default meta;
type Story = StoryObj<typeof meta>;

const sampleMessages: ChatMessage[] = [
  {
    id: "1",
    role: "user",
    content: "What is the capital of France?",
    timestamp: new Date("2024-01-15T10:00:00"),
  },
  {
    id: "2",
    role: "assistant",
    model: "anthropic/claude-3-opus",
    content:
      "The capital of France is **Paris**. It's located in the north-central part of the country on the Seine River.\n\nParis is not only the capital but also the largest city in France, with a population of over 2 million in the city proper and over 12 million in the metropolitan area.",
    timestamp: new Date("2024-01-15T10:00:05"),
    usage: {
      inputTokens: 12,
      outputTokens: 65,
      totalTokens: 77,
      cost: 0.0023,
    },
  },
  {
    id: "3",
    role: "user",
    content: "What are some famous landmarks there?",
    timestamp: new Date("2024-01-15T10:01:00"),
  },
  {
    id: "4",
    role: "assistant",
    model: "anthropic/claude-3-opus",
    content: `Paris has many world-famous landmarks:

1. **Eiffel Tower** - The iconic iron lattice tower built in 1889
2. **Louvre Museum** - Home to the Mona Lisa and thousands of artworks
3. **Notre-Dame Cathedral** - A masterpiece of Gothic architecture
4. **Arc de Triomphe** - Honors those who fought for France
5. **Sacré-Cœur** - Beautiful basilica on Montmartre hill

Would you like more details about any of these?`,
    timestamp: new Date("2024-01-15T10:01:10"),
    usage: {
      inputTokens: 85,
      outputTokens: 120,
      totalTokens: 205,
      cost: 0.0061,
    },
  },
];

const multiModelMessages: ChatMessage[] = [
  {
    id: "1",
    role: "user",
    content: "Explain quantum computing in simple terms.",
    timestamp: new Date("2024-01-15T10:00:00"),
  },
  {
    id: "2",
    role: "assistant",
    model: "anthropic/claude-3-opus",
    content:
      "Quantum computing uses **qubits** instead of regular bits. While a normal bit is either 0 or 1, a qubit can be both at the same time (superposition). This allows quantum computers to process many possibilities simultaneously, making them potentially much faster for certain problems like cryptography and drug discovery.",
    timestamp: new Date("2024-01-15T10:00:05"),
    usage: { inputTokens: 15, outputTokens: 70, totalTokens: 85, cost: 0.0025 },
  },
  {
    id: "3",
    role: "assistant",
    model: "openai/gpt-4o",
    content:
      "Think of quantum computing like this: a regular computer flips coins one at a time. A quantum computer can spin many coins at once and check all the results simultaneously. This parallel processing makes it incredibly powerful for specific tasks, though it's still early technology.",
    timestamp: new Date("2024-01-15T10:00:06"),
    usage: { inputTokens: 15, outputTokens: 60, totalTokens: 75, cost: 0.002 },
  },
];

const manyModelMessages: ChatMessage[] = [
  { ...sampleMessages[0] },
  { ...sampleMessages[1], model: "anthropic/claude-3-opus" },
  { ...sampleMessages[1], model: "openai/gpt-4o" },
  { ...sampleMessages[1], model: "google/palm-2" },
  { ...sampleMessages[1], model: "google/flan-t5-xxl" },
  { ...sampleMessages[1], model: "google/flan-t5-xxl-instruct" },
  { ...sampleMessages[1], model: "google/flan-t5-xxl-instruct-v2" },
];

const streamingResponses: ModelResponse[] = [
  {
    model: "anthropic/claude-3-opus",
    content: "I'm thinking about your question...",
    isStreaming: true,
  },
  {
    model: "openai/gpt-4o",
    content: "",
    isStreaming: true,
  },
];

/**
 * Test: Empty state shows EmptyChat component when no messages or streaming
 */
export const Empty: Story = {
  decorators: [
    (Story) => (
      <StoreSetup messages={[]} selectedModels={["anthropic/claude-3-opus"]}>
        <Story />
      </StoreSetup>
    ),
  ],
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // Verify the empty state is rendered (EmptyChat component shows model info)
    // EmptyChat renders when there are no messages
    await expect(canvas.getByText(/claude-3-opus/i)).toBeInTheDocument();
  },
};

/**
 * Test: Empty state with no models selected shows prompt to select models
 */
export const EmptyNoModels: Story = {
  decorators: [
    (Story) => (
      <StoreSetup messages={[]} selectedModels={[]}>
        <Story />
      </StoreSetup>
    ),
  ],
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // When no models are selected, EmptyChat should prompt to select models
    await expect(canvas.getByText(/select a model/i)).toBeInTheDocument();
  },
};

/**
 * Test: Single model conversation renders user and assistant messages
 * Verifies virtualization renders visible messages correctly
 */
export const SingleModel: Story = {
  decorators: [
    (Story) => (
      <StoreSetup messages={sampleMessages} selectedModels={["anthropic/claude-3-opus"]}>
        <Story />
      </StoreSetup>
    ),
  ],
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // Verify user messages are rendered
    await expect(canvas.getByText("What is the capital of France?")).toBeInTheDocument();
    await expect(canvas.getByText("What are some famous landmarks there?")).toBeInTheDocument();

    // Verify assistant responses are rendered (check for partial content)
    await expect(canvas.getByText(/capital of France is/)).toBeInTheDocument();
    await expect(canvas.getByText(/Eiffel Tower/)).toBeInTheDocument();

    // Verify user messages have correct role (via article aria-label)
    const userMessages = canvas.getAllByRole("article", { name: /your message/i });
    await expect(userMessages).toHaveLength(2);
  },
};

/**
 * Test: Multiple model responses are grouped and rendered together
 * When multiple models respond to the same user message, they appear as a group
 */
export const MultipleModels: Story = {
  decorators: [
    (Story) => (
      <StoreSetup
        messages={multiModelMessages}
        selectedModels={["anthropic/claude-3-opus", "openai/gpt-4o"]}
      >
        <Story />
      </StoreSetup>
    ),
  ],
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // Verify user message is rendered
    await expect(
      canvas.getByText("Explain quantum computing in simple terms.")
    ).toBeInTheDocument();

    // Verify both model responses are rendered
    await expect(canvas.getByText(/qubits/)).toBeInTheDocument();
    await expect(canvas.getByText(/flips coins/)).toBeInTheDocument();

    // Verify multi-response header shows "2 responses"
    await expect(canvas.getByText(/2 responses/)).toBeInTheDocument();
  },
};

/**
 * Test: Many model responses are rendered correctly
 * Verifies the component handles 6+ model responses in a single group
 */
export const ManyModels: Story = {
  decorators: [
    (Story) => (
      <StoreSetup
        messages={manyModelMessages}
        selectedModels={["anthropic/claude-3-opus", "openai/gpt-4o"]}
      >
        <Story />
      </StoreSetup>
    ),
  ],
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // Verify user message is present
    await expect(canvas.getByText("What is the capital of France?")).toBeInTheDocument();

    // Verify multi-response header shows correct count (6 assistant responses)
    await expect(canvas.getByText(/6 responses/)).toBeInTheDocument();

    // Verify view toggle buttons are present for multi-response
    // The grid/stacked toggle should appear
    const buttons = canvasElement.querySelectorAll('button[class*="h-6 w-6"]');
    await expect(buttons.length).toBeGreaterThanOrEqual(2); // grid and stacked buttons
  },
};

/**
 * Test: Streaming responses render outside virtualization
 * During streaming, responses appear at the bottom and show streaming indicators
 */
export const Streaming: Story = {
  decorators: [
    (Story) => (
      <StoreSetup
        messages={[
          {
            id: "1",
            role: "user",
            content: "What's the meaning of life?",
            timestamp: new Date(),
          },
        ]}
        modelResponses={streamingResponses}
        selectedModels={["anthropic/claude-3-opus", "openai/gpt-4o"]}
      >
        <Story />
      </StoreSetup>
    ),
  ],
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // Verify user message is rendered
    await expect(canvas.getByText("What's the meaning of life?")).toBeInTheDocument();

    // Verify streaming content appears (one model has content, one doesn't)
    await expect(canvas.getByText("I'm thinking about your question...")).toBeInTheDocument();

    // Verify typing indicator for model with no content yet (shows "Thinking...")
    await expect(canvas.getByText(/Thinking/)).toBeInTheDocument();
  },
};

/**
 * Test: Messages with file attachments render file previews
 */
export const WithFiles: Story = {
  decorators: [
    (Story) => (
      <StoreSetup
        messages={[
          {
            id: "1",
            role: "user",
            content: "Can you analyze this image?",
            timestamp: new Date("2024-01-15T10:00:00"),
            files: [
              {
                id: "file-1",
                name: "screenshot.png",
                type: "image/png",
                size: 245000,
                base64: "",
                preview: "https://picsum.photos/200/150",
              },
            ],
          },
          {
            id: "2",
            role: "assistant",
            model: "anthropic/claude-3-opus",
            content:
              "I can see the image you've shared. It appears to be a screenshot. Let me analyze it for you...",
            timestamp: new Date("2024-01-15T10:00:05"),
            usage: { inputTokens: 1500, outputTokens: 45, totalTokens: 1545, cost: 0.046 },
          },
        ]}
        selectedModels={["anthropic/claude-3-opus"]}
      >
        <Story />
      </StoreSetup>
    ),
  ],
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // Verify user message with file content
    await expect(canvas.getByText("Can you analyze this image?")).toBeInTheDocument();

    // Verify file name is displayed
    await expect(canvas.getByText("screenshot.png")).toBeInTheDocument();

    // Verify image preview is rendered with descriptive alt text
    const image = canvas.getByRole("img", { name: "Preview of screenshot.png" });
    await expect(image).toBeInTheDocument();

    // Verify assistant response is rendered
    await expect(canvas.getByText(/appears to be a screenshot/)).toBeInTheDocument();
  },
};

// Generate a long conversation for virtualization testing
const longConversation: ChatMessage[] = Array.from({ length: 20 }, (_, i) => [
  {
    id: `user-${i}`,
    role: "user" as const,
    content: `Question ${i + 1}: What is ${i + 1} times ${i + 2}?`,
    timestamp: new Date(2024, 0, 15, 10, i),
  },
  {
    id: `assistant-${i}`,
    role: "assistant" as const,
    model: "anthropic/claude-3-opus",
    content: `The answer to ${i + 1} times ${i + 2} is ${(i + 1) * (i + 2)}.`,
    timestamp: new Date(2024, 0, 15, 10, i, 5),
    usage: { inputTokens: 10, outputTokens: 15, totalTokens: 25, cost: 0.001 },
  },
]).flat();

/**
 * Test: Virtualization with long conversation
 * Verifies that only visible messages are rendered in the DOM (overscan of 3)
 * and that scrolling reveals additional messages
 */
export const LongConversation: Story = {
  decorators: [
    (Story) => (
      <StoreSetup messages={longConversation} selectedModels={["anthropic/claude-3-opus"]}>
        <Story />
      </StoreSetup>
    ),
  ],
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // Verify some messages are rendered (the virtualized list should render visible + overscan)
    // First message should be visible
    await expect(canvas.getByText("Question 1: What is 1 times 2?")).toBeInTheDocument();

    // The virtualizer uses overscan of 3, so we expect limited DOM nodes
    // Count all user message articles to verify virtualization is working
    const userMessages = canvas.getAllByRole("article", { name: /your message/i });

    // With 600px height and ~200px per group estimate, plus 3 overscan,
    // we should have fewer than 20 user messages rendered
    // (The exact number depends on measurements, but should be less than total)
    await expect(userMessages.length).toBeLessThan(20);
    await expect(userMessages.length).toBeGreaterThan(0);
  },
};

/**
 * Test: Disabled models affect streaming but not committed messages
 *
 * NOTE: Per the component design, disabledModels only affects FUTURE queries
 * (prevents querying disabled instances during streaming). Committed messages
 * from disabled models are still shown - use hiddenResponseIds to hide those.
 *
 * This test verifies that committed messages from all models are still rendered
 * even when one model is disabled.
 */
export const WithDisabledModel: Story = {
  decorators: [
    (Story) => (
      <StoreSetup
        messages={multiModelMessages}
        selectedModels={["anthropic/claude-3-opus", "openai/gpt-4o"]}
        disabledModels={["openai/gpt-4o"]}
      >
        <Story />
      </StoreSetup>
    ),
  ],
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // Verify user message is present
    await expect(
      canvas.getByText("Explain quantum computing in simple terms.")
    ).toBeInTheDocument();

    // Verify Claude response is rendered
    await expect(canvas.getByText(/qubits/)).toBeInTheDocument();

    // GPT-4 committed message is still rendered (disabledModels only affects streaming)
    // The "flips coins" text should still be present in committed messages
    await expect(canvas.getByText(/flips coins/)).toBeInTheDocument();

    // With 2 model responses, "2 responses" badge should appear
    await expect(canvas.getByText("2 responses")).toBeInTheDocument();
  },
};
