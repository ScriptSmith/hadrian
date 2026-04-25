/**
 * Video-specific stories for the demo recording pipeline.
 * These stories are designed for Playwright to drive — they simulate
 * typing, sending, and streaming responses via store manipulation.
 */

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { Meta, StoryObj } from "@storybook/react";
import { fn } from "storybook/test";
import { useEffect, useRef, useCallback } from "react";

import type { ModelInfo } from "@/components/ModelSelector/ModelSelector";
import { AuthProvider } from "@/auth";
import { ConfigProvider } from "@/config/ConfigProvider";
import { PreferencesProvider } from "@/preferences/PreferencesProvider";
import { ConfirmDialogProvider } from "@/components/ConfirmDialog/ConfirmDialog";
import { ToastProvider } from "@/components/Toast/Toast";
import { TooltipProvider } from "@/components/Tooltip/Tooltip";
import { useConversationStore } from "@/stores/conversationStore";
import { useStreamingStore } from "@/stores/streamingStore";
import { useChatUIStore } from "@/stores/chatUIStore";

import { ChatView } from "./ChatView";

const queryClient = new QueryClient({
  defaultOptions: { queries: { retry: false } },
});

const mockModels: ModelInfo[] = [
  {
    id: "anthropic/claude-4.7-opus",
    owned_by: "anthropic",
    context_length: 200000,
    pricing: { prompt: "15", completion: "75" },
  },
  {
    id: "google/gemini-3.1-pro",
    owned_by: "google",
    context_length: 1000000,
    pricing: { prompt: "1.25", completion: "5" },
  },
  {
    id: "openai/gpt-5.4",
    owned_by: "openai",
    context_length: 128000,
    pricing: { prompt: "5", completion: "15" },
  },
];

const selectedModels = mockModels.map((m) => m.id);

// Per-model streaming responses (shown character-by-character)
const streamedResponses: Record<string, string> = {
  "anthropic/claude-4.7-opus": `Great question! Here's a simple explanation:

## What is Quantum Computing?

Regular computers use **bits** — tiny switches that are either 0 or 1. Quantum computers use **qubits**, which can be 0, 1, or *both at once* (called **superposition**).

Think of solving a maze: a regular computer tries one path at a time, while a quantum computer explores many paths simultaneously.

### Real-world applications
- **Cryptography** — breaking and creating unbreakable codes
- **Drug discovery** — simulating molecular interactions
- **Optimization** — solving logistics and scheduling at scale

The catch? Qubits need near **absolute zero** (-273°C) and total isolation. Early days, but progress is rapid.`,

  "google/gemini-3.1-pro": `## Quantum Computing, Simply

Classical bits are always 0 or 1. Quantum **qubits** use two key properties:

**Superposition** — A qubit exists as 0 and 1 simultaneously until measured, like a coin spinning in the air.

**Entanglement** — Linked qubits share state instantly, regardless of distance.

Together, these let quantum computers evaluate vast numbers of possibilities at once. Problems that take classical computers thousands of years might take a quantum computer minutes.

Key players like Google and IBM are racing toward practical quantum advantage — we're closer than you think.`,

  "openai/gpt-5.4": `**Classical computers** are like reading a book one page at a time. **Quantum computers** read all pages at once.

The magic: **qubits** exploit superposition (multiple states at once) and entanglement (instant correlations between particles).

### What can they do?
- Break and build encryption
- Simulate molecules for drug design
- Optimize supply chains
- Accelerate machine learning

### Reality check
Today's quantum computers have ~1,000 noisy qubits. We need millions of stable ones for most applications. But breakthroughs are happening every year.`,
};

const modelUsage: Record<
  string,
  {
    inputTokens: number;
    outputTokens: number;
    totalTokens: number;
    cost: number;
    firstTokenMs: number;
    totalDurationMs: number;
    tokensPerSecond: number;
  }
> = {
  "anthropic/claude-4.7-opus": {
    inputTokens: 45,
    outputTokens: 210,
    totalTokens: 255,
    cost: 0.0164,
    firstTokenMs: 450,
    totalDurationMs: 4800,
    tokensPerSecond: 43.8,
  },
  "google/gemini-3.1-pro": {
    inputTokens: 45,
    outputTokens: 195,
    totalTokens: 240,
    cost: 0.001,
    firstTokenMs: 320,
    totalDurationMs: 4200,
    tokensPerSecond: 46.4,
  },
  "openai/gpt-5.4": {
    inputTokens: 45,
    outputTokens: 175,
    totalTokens: 220,
    cost: 0.0029,
    firstTokenMs: 380,
    totalDurationMs: 3900,
    tokensPerSecond: 44.9,
  },
};

/**
 * Video story: empty chat → user types → sends → streaming response appears.
 * Playwright drives the typing and clicking; the story simulates the streaming response
 * after the send button is clicked.
 */
function VideoSendMessageStory({ onSendMessage }: { onSendMessage: (content: string) => void }) {
  const { setMessages, setSelectedModels, addUserMessage } = useConversationStore();
  const { setDisabledModels, setActionConfig, setSystemPrompt, setAllModelSettings } =
    useChatUIStore();
  // Read isStreaming from the streaming store (single boolean subscription)
  const isStreaming = useStreamingStore((s) => s.isStreaming);
  const sentRef = useRef(false);

  useEffect(() => {
    setMessages([]);
    setSelectedModels(selectedModels);
    setDisabledModels([]);
    setSystemPrompt("");
    setAllModelSettings({});
    setActionConfig({
      showFeedback: true,
      showSelectBest: true,
      showRegenerate: true,
      showCopy: true,
      showExpand: true,
    });
    return () => {
      useStreamingStore.getState().clearStreams();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Intercept send — add user message and start simulated streaming
  const handleSend = useCallback(
    (content: string) => {
      if (sentRef.current) return;
      sentRef.current = true;

      // Add the user message to the conversation
      addUserMessage(content);

      // Start streaming all models after a brief "thinking" delay
      setTimeout(() => {
        const store = useStreamingStore.getState();
        store.initStreaming(selectedModels);

        // Stagger model starts for realism
        const delays: Record<string, number> = {
          "anthropic/claude-4.7-opus": 0,
          "google/gemini-3.1-pro": 200,
          "openai/gpt-5.4": 400,
        };

        for (const model of selectedModels) {
          let charIndex = 0;
          const response = streamedResponses[model]!;
          setTimeout(() => {
            const interval = setInterval(() => {
              if (charIndex < response.length) {
                const chunkSize = 2 + Math.floor(Math.random() * 3);
                const chunk = response.slice(charIndex, charIndex + chunkSize);
                useStreamingStore.getState().appendContent(model, chunk);
                charIndex += chunkSize;
              } else {
                clearInterval(interval);
                useStreamingStore.getState().completeStream(model, modelUsage[model]!);
              }
            }, 25);
          }, delays[model] ?? 0);
        }
      }, 500);

      onSendMessage(content);
    },
    [addUserMessage, onSendMessage]
  );

  return (
    <ChatView availableModels={mockModels} onSendMessage={handleSend} isStreaming={isStreaming} />
  );
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
          <AuthProvider>
            <PreferencesProvider>
              <ConfirmDialogProvider>
                <ToastProvider>
                  <TooltipProvider>
                    <div className="h-screen">
                      <Story />
                    </div>
                  </TooltipProvider>
                </ToastProvider>
              </ConfirmDialogProvider>
            </PreferencesProvider>
          </AuthProvider>
        </ConfigProvider>
      </QueryClientProvider>
    ),
  ],
};

export default meta;
type Story = StoryObj<typeof meta>;

export const VideoSendMessage: Story = {
  args: {
    onSendMessage: fn(),
  },
  render: (args) => <VideoSendMessageStory onSendMessage={args.onSendMessage} />,
};
