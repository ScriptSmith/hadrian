import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { Meta, StoryObj } from "@storybook/react";
import { expect, userEvent, within, fn } from "storybook/test";
import { useEffect } from "react";

import type {
  ChatMessage,
  ModelResponse,
  Citation,
  ToolExecutionRound,
  Artifact,
} from "@/components/chat-types";
import type { ModelInfo } from "@/components/ModelSelector/ModelSelector";
import { ConfigProvider } from "@/config/ConfigProvider";
import { PreferencesProvider } from "@/preferences/PreferencesProvider";
import { ToastProvider } from "@/components/Toast/Toast";
import { TooltipProvider } from "@/components/Tooltip/Tooltip";
import { useConversationStore } from "@/stores/conversationStore";
import { useStreamingStore } from "@/stores/streamingStore";
import { useChatUIStore } from "@/stores/chatUIStore";

import { ChatView } from "./ChatView";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: false,
    },
  },
});

// Helper hook to set up store state for stories
function useStoreSetup({
  messages,
  selectedModels,
  modelResponses,
  systemPrompt = "",
  disabledModels = [],
}: {
  messages: ChatMessage[];
  selectedModels: string[];
  modelResponses?: ModelResponse[];
  systemPrompt?: string;
  disabledModels?: string[];
}) {
  const { setMessages, setSelectedModels } = useConversationStore();
  const streamingStore = useStreamingStore();
  const { setDisabledModels, setActionConfig, setSystemPrompt, setAllModelSettings } =
    useChatUIStore();

  useEffect(() => {
    setMessages(messages);
    setSelectedModels(selectedModels);
    setDisabledModels(disabledModels);
    setSystemPrompt(systemPrompt);
    setAllModelSettings({});
    setActionConfig({
      showFeedback: true,
      showSelectBest: true,
      showRegenerate: true,
      showCopy: true,
      showExpand: true,
    });

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
}

const meta: Meta<typeof ChatView> = {
  title: "Chat/ChatView",
  component: ChatView,
  parameters: {
    layout: "fullscreen",
    a11y: {
      config: {
        rules: [
          { id: "landmark-banner-is-top-level", enabled: false },
          { id: "landmark-contentinfo-is-top-level", enabled: false },
          { id: "landmark-main-is-top-level", enabled: false },
        ],
      },
    },
  },
  decorators: [
    (Story) => (
      <QueryClientProvider client={queryClient}>
        <ConfigProvider>
          <PreferencesProvider>
            <ToastProvider>
              <TooltipProvider>
                <div className="h-screen">
                  <Story />
                </div>
              </TooltipProvider>
            </ToastProvider>
          </PreferencesProvider>
        </ConfigProvider>
      </QueryClientProvider>
    ),
  ],
};

export default meta;
type Story = StoryObj<typeof meta>;

const mockModels: ModelInfo[] = [
  {
    id: "anthropic/claude-4.6-opus",
    owned_by: "anthropic",
    context_length: 200000,
    pricing: { prompt: "15", completion: "75" },
  },
  {
    id: "anthropic/claude-4-sonnet",
    owned_by: "anthropic",
    context_length: 200000,
    pricing: { prompt: "3", completion: "15" },
  },
  {
    id: "openai/gpt-5.3",
    owned_by: "openai",
    context_length: 128000,
    pricing: { prompt: "5", completion: "15" },
  },
  {
    id: "google/gemini-3.1-pro",
    owned_by: "google",
    context_length: 1000000,
    pricing: { prompt: "1.25", completion: "5" },
  },
];

const sampleConversation: ChatMessage[] = [
  {
    id: "1",
    role: "user",
    content: "Can you explain how React hooks work?",
    timestamp: new Date("2024-01-15T10:00:00"),
  },
  {
    id: "2",
    role: "assistant",
    model: "anthropic/claude-4.6-opus",
    content: `React Hooks are functions that let you "hook into" React state and lifecycle features from function components. Here are the most common ones:

## useState
\`\`\`jsx
const [count, setCount] = useState(0);
\`\`\`
Manages local state in a component.

## useEffect
\`\`\`jsx
useEffect(() => {
  document.title = \`Count: \${count}\`;
}, [count]);
\`\`\`
Handles side effects like data fetching, subscriptions, or DOM mutations.

## useContext
\`\`\`jsx
const theme = useContext(ThemeContext);
\`\`\`
Accesses context values without prop drilling.

Would you like me to explain any of these in more detail?`,
    timestamp: new Date("2024-01-15T10:00:15"),
    usage: {
      inputTokens: 25,
      outputTokens: 180,
      totalTokens: 205,
      cost: 0.0062,
    },
  },
  {
    id: "3",
    role: "user",
    content: "Yes, can you show me a practical example of useEffect with cleanup?",
    timestamp: new Date("2024-01-15T10:01:00"),
  },
  {
    id: "4",
    role: "assistant",
    model: "anthropic/claude-4.6-opus",
    content: `Here's a practical example of useEffect with cleanup - a window resize listener:

\`\`\`jsx
import { useState, useEffect } from 'react';

function WindowSize() {
  const [size, setSize] = useState({
    width: window.innerWidth,
    height: window.innerHeight
  });

  useEffect(() => {
    // Handler to update state
    const handleResize = () => {
      setSize({
        width: window.innerWidth,
        height: window.innerHeight
      });
    };

    // Add event listener
    window.addEventListener('resize', handleResize);

    // Cleanup function - runs on unmount
    return () => {
      window.removeEventListener('resize', handleResize);
    };
  }, []); // Empty deps = run once on mount

  return (
    <div>
      Window: {size.width} x {size.height}
    </div>
  );
}
\`\`\`

The cleanup function (the returned function) is crucial to prevent memory leaks!`,
    timestamp: new Date("2024-01-15T10:01:30"),
    usage: {
      inputTokens: 230,
      outputTokens: 220,
      totalTokens: 450,
      cost: 0.0135,
    },
  },
];

/**
 * Test: Empty state renders with title and input
 */
function EmptyStory({ onSendMessage }: { onSendMessage: (content: string) => void }) {
  useStoreSetup({ messages: [], selectedModels: ["anthropic/claude-4.6-opus"] });

  return <ChatView availableModels={mockModels} onSendMessage={onSendMessage} />;
}

export const Empty: Story = {
  args: {
    onSendMessage: fn(),
  },
  render: (args) => <EmptyStory onSendMessage={args.onSendMessage} />,
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // Verify empty chat state shows welcome message
    await expect(
      canvas.getByRole("heading", { name: "How can I help you today?" })
    ).toBeInTheDocument();

    // Verify empty chat state shows model info (from EmptyChat component)
    await expect(canvas.getByText(/claude-4.6-opus/i)).toBeInTheDocument();

    // Verify input is present and enabled
    const textarea = canvas.getByPlaceholderText("Type a message...");
    await expect(textarea).toBeInTheDocument();
    await expect(textarea).toBeEnabled();

    // Verify send button exists but is disabled (no content)
    const sendButton = canvas.getByRole("button", { name: /send/i });
    await expect(sendButton).toBeDisabled();
  },
};

/**
 * Test: Conversation with messages renders title, messages, and usage
 */
function WithConversationStory({
  onSendMessage,
  onClearMessages,
}: {
  onSendMessage: (content: string) => void;
  onClearMessages: () => void;
}) {
  useStoreSetup({ messages: sampleConversation, selectedModels: ["anthropic/claude-4.6-opus"] });

  return (
    <ChatView
      availableModels={mockModels}
      onSendMessage={onSendMessage}
      onClearMessages={onClearMessages}
    />
  );
}

export const WithConversation: Story = {
  args: {
    onSendMessage: fn(),
    onClearMessages: fn(),
  },
  render: (args) => (
    <WithConversationStory
      onSendMessage={args.onSendMessage}
      onClearMessages={args.onClearMessages!}
    />
  ),
  play: async ({ canvasElement, args }) => {
    const canvas = within(canvasElement);

    // Verify messages are rendered
    await expect(canvas.getByText("Can you explain how React hooks work?")).toBeInTheDocument();

    // Verify token usage is displayed (total from sampleConversation: 205 + 450 = 655 tokens)
    await expect(canvas.getByText(/655/)).toBeInTheDocument();

    // Verify clear button exists in header area (icon button with Trash icon)
    // Find the header's actions area and locate the trash icon button
    const headerActions = canvasElement.querySelector(".shrink-0.border-b");
    const trashButton = headerActions?.querySelector("button:has(svg.lucide-trash-2)");
    await expect(trashButton).toBeInTheDocument();
    await userEvent.click(trashButton!);
    await expect(args.onClearMessages).toHaveBeenCalled();
  },
};

/**
 * Test: Streaming state shows stop button and streaming indicators
 */
function StreamingStory({
  onSendMessage,
  onStopStreaming,
}: {
  onSendMessage: (content: string) => void;
  onStopStreaming: () => void;
}) {
  const streamingMessages: ChatMessage[] = [
    {
      id: "1",
      role: "user",
      content: "Compare Redux vs React Context for state management",
      timestamp: new Date(),
    },
  ];

  const streamingResponses: ModelResponse[] = [
    {
      model: "anthropic/claude-4.6-opus",
      content:
        "Let me explain the key differences between these two approaches to state management...",
      isStreaming: true,
    },
    {
      model: "openai/gpt-5.3",
      content: "When comparing Redux and React Context, there are several factors to consider:",
      isStreaming: true,
    },
  ];

  // Set up stores for streaming
  useStoreSetup({
    messages: streamingMessages,
    selectedModels: ["anthropic/claude-4.6-opus", "openai/gpt-5.3"],
    modelResponses: streamingResponses,
  });

  return (
    <ChatView
      availableModels={mockModels}
      isStreaming
      onSendMessage={onSendMessage}
      onStopStreaming={onStopStreaming}
    />
  );
}

export const Streaming: Story = {
  args: {
    onSendMessage: fn(),
    onStopStreaming: fn(),
  },
  render: (args) => (
    <StreamingStory onSendMessage={args.onSendMessage} onStopStreaming={args.onStopStreaming!} />
  ),
  play: async ({ canvasElement, args }) => {
    const canvas = within(canvasElement);

    // Verify streaming content from both models
    await expect(canvas.getByText(/key differences/)).toBeInTheDocument();
    await expect(canvas.getByText(/Redux and React Context/)).toBeInTheDocument();

    // Verify stop button is visible during streaming
    const stopButton = canvas.getByRole("button", { name: /stop/i });
    await expect(stopButton).toBeInTheDocument();
    await expect(stopButton).toBeEnabled();

    // Click stop button
    await userEvent.click(stopButton);
    await expect(args.onStopStreaming).toHaveBeenCalled();
  },
};

/**
 * Test: No models selected disables input with appropriate placeholder
 */
function NoModelsSelectedStory({ onSendMessage }: { onSendMessage: (content: string) => void }) {
  useStoreSetup({ messages: [], selectedModels: [] });

  return <ChatView availableModels={mockModels} onSendMessage={onSendMessage} />;
}

export const NoModelsSelected: Story = {
  args: {
    onSendMessage: fn(),
  },
  render: (args) => <NoModelsSelectedStory onSendMessage={args.onSendMessage} />,
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // Verify input shows correct placeholder for no models selected
    const textarea = canvas.getByPlaceholderText("Select a model to start chatting...");
    await expect(textarea).toBeInTheDocument();
    await expect(textarea).toBeDisabled();

    // Verify send button is disabled
    const sendButton = canvas.getByRole("button", { name: /send/i });
    await expect(sendButton).toBeDisabled();

    // Verify empty state prompts to select models
    await expect(canvas.getByText("Select a model to start chatting.")).toBeInTheDocument();
  },
};

/**
 * Test: Multi-model conversation with system prompt indicator
 */
function MultiModelConversationStory({
  onSendMessage,
  onClearMessages,
}: {
  onSendMessage: (content: string) => void;
  onClearMessages: () => void;
}) {
  const multiModelMessages: ChatMessage[] = [
    {
      id: "1",
      role: "user",
      content: "What's the best way to handle errors in async JavaScript?",
      timestamp: new Date("2024-01-15T10:00:00"),
    },
    {
      id: "2",
      role: "assistant",
      model: "anthropic/claude-4.6-opus",
      content:
        "Use **try/catch** with async/await for clean, readable error handling:\n\n```js\ntry {\n  const data = await fetchData();\n} catch (error) {\n  console.error('Failed:', error);\n}\n```",
      timestamp: new Date("2024-01-15T10:00:05"),
      usage: { inputTokens: 20, outputTokens: 60, totalTokens: 80, cost: 0.0024 },
    },
    {
      id: "3",
      role: "assistant",
      model: "openai/gpt-5.3",
      content:
        "I recommend using `.catch()` chains for promises or async/await with try/catch. For production, consider using error boundaries in React and global error handlers.",
      timestamp: new Date("2024-01-15T10:00:06"),
      usage: { inputTokens: 20, outputTokens: 45, totalTokens: 65, cost: 0.0018 },
    },
    {
      id: "4",
      role: "assistant",
      model: "google/gemini-3.1-pro",
      content:
        "Best practices include: 1) Always catch errors, 2) Use specific error types, 3) Log errors appropriately, 4) Provide fallback behavior. Consider libraries like `neverthrow` for type-safe error handling.",
      timestamp: new Date("2024-01-15T10:00:07"),
      usage: { inputTokens: 20, outputTokens: 55, totalTokens: 75, cost: 0.0015 },
    },
  ];

  useStoreSetup({
    messages: multiModelMessages,
    selectedModels: ["anthropic/claude-4.6-opus", "openai/gpt-5.3", "google/gemini-3.1-pro"],
    systemPrompt: "You are a helpful coding assistant.",
  });

  return (
    <ChatView
      availableModels={mockModels}
      onSendMessage={onSendMessage}
      onClearMessages={onClearMessages}
    />
  );
}

export const MultiModelConversation: Story = {
  args: {
    onSendMessage: fn(),
    onClearMessages: fn(),
  },
  render: (args) => (
    <MultiModelConversationStory
      onSendMessage={args.onSendMessage}
      onClearMessages={args.onClearMessages!}
    />
  ),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // Verify multi-model response count (3 responses)
    await expect(canvas.getByText(/3 responses/)).toBeInTheDocument();

    // Verify all three model responses are rendered (use getAllByText for duplicates)
    const tryCatchElements = canvas.getAllByText(/try\/catch/);
    await expect(tryCatchElements.length).toBeGreaterThan(0);
    await expect(canvas.getByText(/error boundaries/)).toBeInTheDocument();
    await expect(canvas.getByText(/neverthrow/)).toBeInTheDocument();

    // Verify system prompt is active - settings button has text-primary class
    // (The button turns primary color when system prompt is set)
    const settingsButton = canvasElement.querySelector('button[class*="text-primary"]');
    await expect(settingsButton).toBeInTheDocument();

    // Verify total usage is displayed (80 + 65 + 75 = 220 tokens)
    await expect(canvas.getByText(/220/)).toBeInTheDocument();
  },
};

/**
 * Test: All models disabled shows appropriate placeholder
 */
function AllModelsDisabledStory({ onSendMessage }: { onSendMessage: (content: string) => void }) {
  useStoreSetup({
    messages: [],
    selectedModels: ["anthropic/claude-4.6-opus", "openai/gpt-5.3"],
    disabledModels: ["anthropic/claude-4.6-opus", "openai/gpt-5.3"],
  });

  return <ChatView availableModels={mockModels} onSendMessage={onSendMessage} />;
}

export const AllModelsDisabled: Story = {
  args: {
    onSendMessage: fn(),
  },
  render: (args) => <AllModelsDisabledStory onSendMessage={args.onSendMessage} />,
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // Verify input shows correct placeholder when all models are disabled
    const textarea = canvas.getByPlaceholderText(
      "All models are disabled. Enable a model to continue..."
    );
    await expect(textarea).toBeInTheDocument();
    await expect(textarea).toBeDisabled();

    // Verify send button is disabled
    const sendButton = canvas.getByRole("button", { name: /send/i });
    await expect(sendButton).toBeDisabled();
  },
};

/**
 * Test: Sending a message calls onSendMessage callback
 */
function SendMessageStory({ onSendMessage }: { onSendMessage: (content: string) => void }) {
  useStoreSetup({ messages: [], selectedModels: ["anthropic/claude-4.6-opus"] });

  return <ChatView availableModels={mockModels} onSendMessage={onSendMessage} />;
}

export const SendMessage: Story = {
  args: {
    onSendMessage: fn(),
  },
  render: (args) => <SendMessageStory onSendMessage={args.onSendMessage} />,
  play: async ({ canvasElement, args }) => {
    const canvas = within(canvasElement);

    // Type a message
    const textarea = canvas.getByPlaceholderText("Type a message...");
    await userEvent.type(textarea, "Hello, world!");

    // Verify send button becomes enabled
    const sendButton = canvas.getByRole("button", { name: /send/i });
    await expect(sendButton).toBeEnabled();

    // Click send
    await userEvent.click(sendButton);

    // Verify callback was called with message
    await expect(args.onSendMessage).toHaveBeenCalledWith("Hello, world!", []);

    // Verify input was cleared
    await expect(textarea).toHaveValue("");
  },
};

/**
 * Test: Settings modal opens when settings button is clicked
 */
function SettingsModalStory({ onSendMessage }: { onSendMessage: (content: string) => void }) {
  useStoreSetup({ messages: [], selectedModels: ["anthropic/claude-4.6-opus"] });

  return <ChatView availableModels={mockModels} onSendMessage={onSendMessage} />;
}

export const SettingsModal: Story = {
  args: {
    onSendMessage: fn(),
  },
  render: (args) => <SettingsModalStory onSendMessage={args.onSendMessage} />,
  play: async ({ canvasElement }) => {
    // Find the settings button in the input area (icon button with Settings2 icon)
    const inputArea = canvasElement.querySelector(".border-t");
    const settingsButton = inputArea?.querySelector("button:has(svg)");

    await expect(settingsButton).toBeInTheDocument();
    await userEvent.click(settingsButton!);

    // Verify modal opens - modals render in a portal, so check the entire document
    // Look for the modal title "Conversation Settings"
    const modalTitle = await within(document.body).findByText("Conversation Settings");
    await expect(modalTitle).toBeInTheDocument();
  },
};

/**
 * Test: Loading models state shows loading indicator in empty chat
 */
function LoadingModelsStory({ onSendMessage }: { onSendMessage: (content: string) => void }) {
  useStoreSetup({ messages: [], selectedModels: ["anthropic/claude-4.6-opus"] });

  return (
    <ChatView availableModels={mockModels} onSendMessage={onSendMessage} isLoadingModels={true} />
  );
}

export const LoadingModels: Story = {
  args: {
    onSendMessage: fn(),
  },
  render: (args) => <LoadingModelsStory onSendMessage={args.onSendMessage} />,
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // Verify loading state is indicated (EmptyChat shows model loading spinner)
    // The loading state should show a loading indicator in the empty chat area
    const loadingElement = canvas.getByRole("status");
    await expect(loadingElement).toBeInTheDocument();
  },
};

// ============================================================================
// Homepage showcase stories
// ============================================================================

/**
 * Showcase: Knowledge Bases tool — model searches uploaded documents and cites results
 */
function KnowledgeBasesStory({ onSendMessage }: { onSendMessage: (content: string) => void }) {
  const fileSearchCitations: Citation[] = [
    {
      id: "cit-1",
      type: "file",
      fileId: "file_q3report",
      filename: "q3_financial_report.pdf",
      snippet:
        "Revenue grew 23% year-over-year to $4.2B, driven by strong enterprise adoption and international expansion into APAC markets.",
      score: 0.95,
    },
    {
      id: "cit-2",
      type: "file",
      fileId: "file_forecast",
      filename: "2025_forecast_model.xlsx",
      snippet:
        "Projected Q4 revenue of $4.8B assumes 15% sequential growth with operating margin improving to 28%.",
      score: 0.88,
    },
    {
      id: "cit-3",
      type: "file",
      fileId: "file_earnings",
      filename: "earnings_call_transcript.md",
      snippet:
        'CEO noted: "We expect sustained double-digit growth through 2026, supported by our expanded product portfolio and deepening customer relationships."',
      score: 0.82,
    },
  ];

  const fileSearchArtifact: Artifact = {
    id: "search-artifact-1",
    type: "file_search",
    title: "Search Results",
    role: "output",
    data: {
      query: "Q3 revenue growth and Q4 outlook",
      vectorStoreIds: ["vs_financial_docs"],
      totalResults: 3,
      results: [
        {
          fileId: "file_q3report",
          filename: "q3_financial_report.pdf",
          score: 0.95,
          content:
            "Revenue grew 23% year-over-year to $4.2B, driven by strong enterprise adoption and international expansion into APAC markets. Gross margin expanded 200bps to 72%.",
        },
        {
          fileId: "file_forecast",
          filename: "2025_forecast_model.xlsx",
          score: 0.88,
          content:
            "Projected Q4 revenue of $4.8B assumes 15% sequential growth with operating margin improving to 28%. Full-year 2025 revenue estimated at $17.1B.",
        },
        {
          fileId: "file_earnings",
          filename: "earnings_call_transcript.md",
          score: 0.82,
          content:
            'CEO noted: "We expect sustained double-digit growth through 2026, supported by our expanded product portfolio and deepening customer relationships."',
        },
      ],
    },
  };

  const fileSearchRounds: ToolExecutionRound[] = [
    {
      round: 1,
      executions: [
        {
          id: "exec-fs-1",
          toolName: "file_search",
          status: "success",
          startTime: Date.now() - 850,
          endTime: Date.now() - 200,
          duration: 650,
          input: { query: "Q3 revenue growth and Q4 outlook" },
          inputArtifacts: [],
          outputArtifacts: [fileSearchArtifact],
          round: 1,
        },
      ],
      totalDuration: 650,
    },
  ];

  const messages: ChatMessage[] = [
    {
      id: "kb-1",
      role: "user",
      content: "What were the key takeaways from Q3 earnings and what's the Q4 outlook?",
      timestamp: new Date("2025-10-20T14:30:00"),
    },
    {
      id: "kb-2",
      role: "assistant",
      model: "anthropic/claude-4.6-opus",
      content: `Based on the financial documents in your knowledge base, here's a summary:

## Q3 Performance

Revenue reached **$4.2B**, up **23% year-over-year**, driven by:
- Strong enterprise adoption
- International expansion into APAC markets
- Gross margin expansion of 200bps to 72%

## Q4 Outlook

The forecast model projects **$4.8B** in Q4 revenue, assuming:
- 15% sequential growth
- Operating margin improving to **28%**
- Full-year 2025 revenue estimated at **$17.1B**

Management expressed confidence in sustained double-digit growth through 2026, citing an expanded product portfolio and deepening customer relationships.`,
      timestamp: new Date("2025-10-20T14:30:08"),
      usage: {
        inputTokens: 1240,
        outputTokens: 185,
        totalTokens: 1425,
        cost: 0.0324,
        firstTokenMs: 650,
        totalDurationMs: 3200,
        tokensPerSecond: 57.8,
      },
      citations: fileSearchCitations,
      toolExecutionRounds: fileSearchRounds,
    },
  ];

  useStoreSetup({ messages, selectedModels: ["anthropic/claude-4.6-opus"] });

  return <ChatView availableModels={mockModels} onSendMessage={onSendMessage} />;
}

export const KnowledgeBases: Story = {
  args: {
    onSendMessage: fn(),
  },
  render: (args) => <KnowledgeBasesStory onSendMessage={args.onSendMessage} />,
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // Verify user message
    await expect(canvas.getByText(/Q3 earnings/)).toBeInTheDocument();

    // Verify assistant content
    await expect(canvas.getByText(/\$4\.2B/)).toBeInTheDocument();

    // Verify citations are rendered
    await expect(canvas.getByText("q3_financial_report.pdf")).toBeInTheDocument();
    await expect(canvas.getByText("2025_forecast_model.xlsx")).toBeInTheDocument();
  },
};

/**
 * Showcase: Code execution — model runs Python and displays chart artifact inline
 */
function ExecuteCodeStory({ onSendMessage }: { onSendMessage: (content: string) => void }) {
  const pythonInputArtifact: Artifact = {
    id: "py-input-1",
    type: "code",
    title: "Python",
    role: "input",
    data: {
      language: "python",
      code: `import numpy as np
import matplotlib.pyplot as plt
from matplotlib.collections import LineCollection

# Simulate the Lorenz system
sigma, rho, beta = 10, 28, 8/3
dt = 0.002
steps = 20000

xyz = np.empty((steps, 3))
xyz[0] = [0.1, 0, 0]
for i in range(1, steps):
    x, y, z = xyz[i-1]
    xyz[i] = xyz[i-1] + dt * np.array([
        sigma * (y - x),
        x * (rho - z) - y,
        x * y - beta * z,
    ])

fig, ax = plt.subplots(figsize=(10, 7), facecolor='#0f172a')
ax.set_facecolor('#0f172a')

# Color by velocity (speed of change)
velocity = np.linalg.norm(np.diff(xyz, axis=0), axis=1)
points = xyz[:-1, [0, 2]]  # project onto x-z plane
segments = np.stack([points[:-1], points[1:]], axis=1)

lc = LineCollection(segments, cmap='turbo', linewidths=0.6, alpha=0.9)
lc.set_array(velocity[:-1])
ax.add_collection(lc)
ax.autoscale()
ax.set_xlabel('x', color='#94a3b8')
ax.set_ylabel('z', color='#94a3b8')
ax.set_title('Lorenz Attractor — Colored by Velocity', color='white', fontsize=14)
ax.tick_params(colors='#475569')
for spine in ax.spines.values():
    spine.set_color('#1e293b')
plt.tight_layout()
plt.savefig('lorenz.png', dpi=180, facecolor='#0f172a')
print(f"Rendered {steps:,} steps, max velocity: {velocity.max():.1f}")`,
    },
  };

  const chartOutputArtifact: Artifact = {
    id: "chart-output-1",
    type: "image",
    title: "Lorenz Attractor",
    role: "output",
    mimeType: "image/png",
    data: { src: "story-assets/lorenz.png" },
  };

  const stdoutArtifact: Artifact = {
    id: "py-stdout-1",
    type: "code",
    title: "stdout",
    role: "output",
    data: { language: "text", code: "Rendered 20,000 steps, max velocity: 46.3" },
  };

  const displaySelectionArtifact: Artifact = {
    id: "display-sel-1",
    type: "display_selection",
    role: "output",
    data: {
      artifactIds: ["chart-output-1"],
      layout: "inline",
    },
  };

  const pythonRounds: ToolExecutionRound[] = [
    {
      round: 1,
      executions: [
        {
          id: "exec-py-1",
          toolName: "code_interpreter",
          status: "success",
          startTime: Date.now() - 3800,
          endTime: Date.now() - 600,
          duration: 3200,
          input: {},
          inputArtifacts: [pythonInputArtifact],
          outputArtifacts: [stdoutArtifact, chartOutputArtifact],
          round: 1,
        },
      ],
      totalDuration: 3200,
    },
    {
      round: 2,
      executions: [
        {
          id: "exec-display-1",
          toolName: "display_artifacts",
          status: "success",
          startTime: Date.now() - 500,
          endTime: Date.now() - 200,
          duration: 300,
          input: { artifactIds: ["chart-output-1"], layout: "inline" },
          inputArtifacts: [],
          outputArtifacts: [displaySelectionArtifact],
          round: 2,
        },
      ],
      totalDuration: 300,
    },
  ];

  const messages: ChatMessage[] = [
    {
      id: "py-1",
      role: "user",
      content:
        "Simulate the Lorenz attractor and visualize the chaotic trajectory colored by velocity.",
      timestamp: new Date("2025-10-20T15:10:00"),
    },
    {
      id: "py-2",
      role: "assistant",
      model: "openai/gpt-5.3",
      content: `Here's the Lorenz attractor projected onto the x-z plane, with each point colored by its instantaneous velocity through phase space.

**What you're seeing:**
- The system traces two lobes — the classic "butterfly" shape of deterministic chaos
- **High-velocity regions** (bright yellow) occur during transitions between lobes, where the trajectory is flung across the attractor
- **Low-velocity regions** (deep purple) mark the tight spirals where the system lingers before switching
- Despite being fully deterministic (\u03C3=10, \u03C1=28, \u03B2=8/3), the trajectory never repeats — a hallmark of strange attractors`,
      timestamp: new Date("2025-10-20T15:10:12"),
      usage: {
        inputTokens: 92,
        outputTokens: 380,
        totalTokens: 472,
        cost: 0.0079,
        firstTokenMs: 3800,
        totalDurationMs: 7200,
        tokensPerSecond: 52.8,
      },
      artifacts: [chartOutputArtifact, displaySelectionArtifact],
      toolExecutionRounds: pythonRounds,
    },
  ];

  useStoreSetup({ messages, selectedModels: ["openai/gpt-5.3"] });

  return <ChatView availableModels={mockModels} onSendMessage={onSendMessage} />;
}

export const ExecuteCode: Story = {
  args: {
    onSendMessage: fn(),
  },
  render: (args) => <ExecuteCodeStory onSendMessage={args.onSendMessage} />,
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // Verify user message
    await expect(canvas.getByText(/Simulate the Lorenz attractor/)).toBeInTheDocument();

    // Verify assistant content has key data points
    await expect(canvas.getByText(/butterfly/)).toBeInTheDocument();
    await expect(canvas.getByText(/strange attractors/)).toBeInTheDocument();

    // Verify tool execution block is rendered (2 tools across 2 rounds)
    await expect(canvas.getByText(/2 tools/)).toBeInTheDocument();
  },
};
