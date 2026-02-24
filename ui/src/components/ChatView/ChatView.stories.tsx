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
      code: `import matplotlib.pyplot as plt
import matplotlib.ticker as mticker

categories = ['Cloud', 'Enterprise', 'Consumer', 'Services', 'Hardware']
q3_revenue = [1.42, 1.05, 0.82, 0.58, 0.33]
q2_revenue = [1.18, 0.94, 0.79, 0.52, 0.31]

x = range(len(categories))
width = 0.35

fig, ax = plt.subplots(figsize=(10, 6))
bars_q2 = ax.bar([i - width/2 for i in x], q2_revenue, width, label='Q2', color='#94a3b8')
bars_q3 = ax.bar([i + width/2 for i in x], q3_revenue, width, label='Q3', color='#3b82f6')

ax.set_ylabel('Revenue ($B)')
ax.set_title('Revenue by Segment: Q2 vs Q3 2025')
ax.set_xticks(x)
ax.set_xticklabels(categories)
ax.yaxis.set_major_formatter(mticker.FormatStrFormatter('$%.2f'))
ax.legend()
ax.bar_label(bars_q3, fmt='$%.2f', padding=3, fontsize=9)
plt.tight_layout()
plt.savefig('revenue_comparison.png', dpi=150)
print("Chart saved successfully")`,
    },
  };

  const chartOutputArtifact: Artifact = {
    id: "chart-output-1",
    type: "chart",
    title: "Revenue by Segment",
    role: "output",
    data: {
      spec: {
        $schema: "https://vega.github.io/schema/vega-lite/v6.json",
        title: "Revenue by Segment: Q2 vs Q3 2025",
        width: 500,
        height: 300,
        data: {
          values: [
            { segment: "Cloud", quarter: "Q2", revenue: 1.18 },
            { segment: "Cloud", quarter: "Q3", revenue: 1.42 },
            { segment: "Enterprise", quarter: "Q2", revenue: 0.94 },
            { segment: "Enterprise", quarter: "Q3", revenue: 1.05 },
            { segment: "Consumer", quarter: "Q2", revenue: 0.79 },
            { segment: "Consumer", quarter: "Q3", revenue: 0.82 },
            { segment: "Services", quarter: "Q2", revenue: 0.52 },
            { segment: "Services", quarter: "Q3", revenue: 0.58 },
            { segment: "Hardware", quarter: "Q2", revenue: 0.31 },
            { segment: "Hardware", quarter: "Q3", revenue: 0.33 },
          ],
        },
        mark: "bar",
        encoding: {
          x: { field: "segment", type: "nominal", title: "Segment", axis: { labelAngle: 0 } },
          xOffset: { field: "quarter" },
          y: {
            field: "revenue",
            type: "quantitative",
            title: "Revenue ($B)",
          },
          color: {
            field: "quarter",
            type: "nominal",
            scale: { range: ["#94a3b8", "#3b82f6"] },
            title: "Quarter",
          },
        },
      },
    },
  };

  const stdoutArtifact: Artifact = {
    id: "py-stdout-1",
    type: "code",
    title: "stdout",
    role: "output",
    data: { language: "text", code: "Chart saved successfully" },
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
      content: "Can you visualize Q2 vs Q3 revenue by business segment?",
      timestamp: new Date("2025-10-20T15:10:00"),
    },
    {
      id: "py-2",
      role: "assistant",
      model: "openai/gpt-5.3",
      content: `I created a grouped bar chart comparing Q2 and Q3 revenue across all five business segments.

**Key highlights:**
- **Cloud** led growth with **$1.42B** in Q3, up 20% from Q2
- **Enterprise** crossed $1B for the first time at **$1.05B**
- **Consumer** and **Services** showed steady single-digit growth
- **Hardware** remained stable at **$0.33B**

Total Q3 revenue of **$4.20B** represents a **12.3%** increase over Q2's $3.74B.`,
      timestamp: new Date("2025-10-20T15:10:12"),
      usage: {
        inputTokens: 85,
        outputTokens: 310,
        totalTokens: 395,
        cost: 0.0066,
        firstTokenMs: 3800,
        totalDurationMs: 7200,
        tokensPerSecond: 43.1,
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
    await expect(canvas.getByText(/Q2 vs Q3 revenue/)).toBeInTheDocument();

    // Verify assistant content has key data points
    await expect(canvas.getByText(/\$1\.42B/)).toBeInTheDocument();
    await expect(canvas.getByText(/12\.3%/)).toBeInTheDocument();

    // Verify tool execution block is rendered (2 tools across 2 rounds)
    await expect(canvas.getByText(/2 tools/)).toBeInTheDocument();
  },
};
