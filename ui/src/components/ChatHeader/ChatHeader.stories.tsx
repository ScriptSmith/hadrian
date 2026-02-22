import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { Meta, StoryObj } from "@storybook/react";
import { useState } from "react";

import type { Conversation, MessageUsage, ModelInstance } from "@/components/chat-types";
import type { ModelInfo } from "@/components/ModelPicker/ModelPicker";
import { PreferencesProvider } from "@/preferences/PreferencesProvider";
import type { TotalUsageResult } from "@/stores/conversationStore";
import { TooltipProvider } from "@/components/Tooltip/Tooltip";

import { ChatHeader } from "./ChatHeader";

/** Helper to create TotalUsageResult with optional mode overhead */
function makeUsage(
  total: Partial<MessageUsage>,
  modeOverhead?: Partial<MessageUsage>
): TotalUsageResult {
  const t: MessageUsage = {
    inputTokens: total.inputTokens ?? 0,
    outputTokens: total.outputTokens ?? 0,
    totalTokens: total.totalTokens ?? 0,
    cost: total.cost,
    cachedTokens: total.cachedTokens,
    reasoningTokens: total.reasoningTokens,
  };
  const m: MessageUsage = {
    inputTokens: modeOverhead?.inputTokens ?? 0,
    outputTokens: modeOverhead?.outputTokens ?? 0,
    totalTokens: modeOverhead?.totalTokens ?? 0,
    cost: modeOverhead?.cost,
    cachedTokens: modeOverhead?.cachedTokens,
    reasoningTokens: modeOverhead?.reasoningTokens,
  };
  return {
    total: t,
    modeOverhead: m,
    grandTotal: {
      inputTokens: t.inputTokens + m.inputTokens,
      outputTokens: t.outputTokens + m.outputTokens,
      totalTokens: t.totalTokens + m.totalTokens,
      cost: (t.cost ?? 0) + (m.cost ?? 0),
      cachedTokens: (t.cachedTokens ?? 0) + (m.cachedTokens ?? 0),
      reasoningTokens: (t.reasoningTokens ?? 0) + (m.reasoningTokens ?? 0),
    },
  };
}

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: false,
    },
  },
});

const meta: Meta<typeof ChatHeader> = {
  title: "Chat/ChatHeader",
  component: ChatHeader,
  parameters: {
    layout: "fullscreen",
  },
  decorators: [
    (Story) => (
      <QueryClientProvider client={queryClient}>
        <PreferencesProvider>
          <TooltipProvider>
            <div className="w-full max-w-4xl mx-auto">
              <Story />
            </div>
          </TooltipProvider>
        </PreferencesProvider>
      </QueryClientProvider>
    ),
  ],
};

export default meta;
type Story = StoryObj<typeof meta>;

const mockModels: ModelInfo[] = [
  {
    id: "anthropic/claude-3-opus",
    owned_by: "anthropic",
    context_length: 200000,
    pricing: { prompt: "15", completion: "75" },
  },
  {
    id: "anthropic/claude-3-sonnet",
    owned_by: "anthropic",
    context_length: 200000,
    pricing: { prompt: "3", completion: "15" },
  },
  {
    id: "openai/gpt-4o",
    owned_by: "openai",
    context_length: 128000,
    pricing: { prompt: "5", completion: "15" },
  },
  {
    id: "google/gemini-1.5-pro",
    owned_by: "google",
    context_length: 1000000,
    pricing: { prompt: "1.25", completion: "5" },
  },
];

function DefaultStory() {
  const [instances, setInstances] = useState<ModelInstance[]>([
    { id: "anthropic/claude-3-opus", modelId: "anthropic/claude-3-opus" },
  ]);
  return (
    <ChatHeader
      selectedInstances={instances}
      onInstancesChange={setInstances}
      availableModels={mockModels}
    />
  );
}

export const Default: Story = {
  render: () => <DefaultStory />,
};

function WithUsageStory() {
  const [instances, setInstances] = useState<ModelInstance[]>([
    { id: "anthropic/claude-3-opus", modelId: "anthropic/claude-3-opus" },
  ]);
  const [disabledInstances, setDisabledInstances] = useState<string[]>([]);
  return (
    <ChatHeader
      selectedInstances={instances}
      onInstancesChange={setInstances}
      availableModels={mockModels}
      totalUsage={makeUsage({
        inputTokens: 1234,
        outputTokens: 5678,
        totalTokens: 6912,
        cost: 0.0345,
      })}
      canClear
      hasMessages
      onClear={() => console.log("Clear clicked")}
      disabledInstances={disabledInstances}
      onDisabledInstancesChange={setDisabledInstances}
    />
  );
}

export const WithUsage: Story = {
  render: () => <WithUsageStory />,
};

function MultipleModelsStory() {
  const [instances, setInstances] = useState<ModelInstance[]>([
    { id: "anthropic/claude-3-opus", modelId: "anthropic/claude-3-opus" },
    { id: "openai/gpt-4o", modelId: "openai/gpt-4o" },
    { id: "google/gemini-1.5-pro", modelId: "google/gemini-1.5-pro" },
  ]);
  const [disabledInstances, setDisabledInstances] = useState<string[]>([]);
  return (
    <ChatHeader
      selectedInstances={instances}
      onInstancesChange={setInstances}
      availableModels={mockModels}
      totalUsage={makeUsage({
        inputTokens: 3500,
        outputTokens: 12000,
        totalTokens: 15500,
        cost: 0.125,
        cachedTokens: 500,
      })}
      canClear
      hasMessages
      onClear={() => console.log("Clear clicked")}
      disabledInstances={disabledInstances}
      onDisabledInstancesChange={setDisabledInstances}
    />
  );
}

export const MultipleModels: Story = {
  render: () => <MultipleModelsStory />,
};

function StreamingStory() {
  const [instances, setInstances] = useState<ModelInstance[]>([
    { id: "anthropic/claude-3-opus", modelId: "anthropic/claude-3-opus" },
  ]);
  const [disabledInstances, setDisabledInstances] = useState<string[]>([]);
  return (
    <ChatHeader
      selectedInstances={instances}
      onInstancesChange={setInstances}
      availableModels={mockModels}
      totalUsage={makeUsage({
        inputTokens: 500,
        outputTokens: 1200,
        totalTokens: 1700,
        cost: 0.015,
      })}
      canClear
      hasMessages
      isStreaming
      onClear={() => console.log("Clear clicked")}
      disabledInstances={disabledInstances}
      onDisabledInstancesChange={setDisabledInstances}
    />
  );
}

export const Streaming: Story = {
  render: () => <StreamingStory />,
};

function NoModelsSelectedStory() {
  const [instances, setInstances] = useState<ModelInstance[]>([]);
  return (
    <ChatHeader
      selectedInstances={instances}
      onInstancesChange={setInstances}
      availableModels={mockModels}
    />
  );
}

export const NoModelsSelected: Story = {
  render: () => <NoModelsSelectedStory />,
};

// Mock conversation for export stories
const mockConversation: Conversation = {
  id: "conv-123",
  title: "Discussing API Design",
  models: ["anthropic/claude-3-opus", "openai/gpt-4o"],
  createdAt: new Date("2024-01-15T10:30:00Z"),
  updatedAt: new Date("2024-01-15T11:45:00Z"),
  messages: [
    {
      id: "msg-1",
      role: "user",
      content: "What are the best practices for REST API design?",
      timestamp: new Date("2024-01-15T10:30:00Z"),
    },
    {
      id: "msg-2",
      role: "assistant",
      model: "anthropic/claude-3-opus",
      content:
        "Here are the key best practices for REST API design:\n\n1. Use nouns for resources (e.g., /users, /products)\n2. Use HTTP methods correctly (GET, POST, PUT, DELETE)\n3. Use proper status codes\n4. Version your API\n5. Handle errors consistently",
      timestamp: new Date("2024-01-15T10:31:00Z"),
      usage: {
        inputTokens: 15,
        outputTokens: 85,
        totalTokens: 100,
        cost: 0.0025,
      },
      feedback: {
        rating: "positive",
        selectedAsBest: true,
      },
    },
    {
      id: "msg-3",
      role: "assistant",
      model: "openai/gpt-4o",
      content:
        "REST API design best practices include:\n\n- Resource naming with plural nouns\n- Proper HTTP method usage\n- Consistent error handling\n- Pagination for large datasets\n- Rate limiting",
      timestamp: new Date("2024-01-15T10:31:00Z"),
      usage: {
        inputTokens: 15,
        outputTokens: 75,
        totalTokens: 90,
        cost: 0.002,
      },
    },
  ],
};

function WithExportStory() {
  const [instances, setInstances] = useState<ModelInstance[]>([
    { id: "anthropic/claude-3-opus", modelId: "anthropic/claude-3-opus" },
    { id: "openai/gpt-4o", modelId: "openai/gpt-4o" },
  ]);
  const [disabledInstances, setDisabledInstances] = useState<string[]>([]);
  return (
    <ChatHeader
      selectedInstances={instances}
      onInstancesChange={setInstances}
      availableModels={mockModels}
      totalUsage={makeUsage({
        inputTokens: 30,
        outputTokens: 160,
        totalTokens: 190,
        cost: 0.0045,
      })}
      canClear
      hasMessages
      onClear={() => console.log("Clear clicked")}
      disabledInstances={disabledInstances}
      onDisabledInstancesChange={setDisabledInstances}
      conversation={mockConversation}
    />
  );
}

export const WithExport: Story = {
  render: () => <WithExportStory />,
};

// Story showing attached vector stores indicator
function WithKnowledgeBaseStory() {
  const [instances, setInstances] = useState<ModelInstance[]>([
    { id: "anthropic/claude-3-opus", modelId: "anthropic/claude-3-opus" },
  ]);
  const [disabledInstances, setDisabledInstances] = useState<string[]>([]);
  return (
    <ChatHeader
      selectedInstances={instances}
      onInstancesChange={setInstances}
      availableModels={mockModels}
      totalUsage={makeUsage({
        inputTokens: 1500,
        outputTokens: 3500,
        totalTokens: 5000,
        cost: 0.025,
      })}
      canClear
      hasMessages
      onClear={() => console.log("Clear clicked")}
      disabledInstances={disabledInstances}
      onDisabledInstancesChange={setDisabledInstances}
      vectorStoreIds={["vs-123", "vs-456"]}
    />
  );
}

export const WithKnowledgeBase: Story = {
  render: () => <WithKnowledgeBaseStory />,
};

function WithSingleCollectionStory() {
  const [instances, setInstances] = useState<ModelInstance[]>([
    { id: "anthropic/claude-3-opus", modelId: "anthropic/claude-3-opus" },
    { id: "openai/gpt-4o", modelId: "openai/gpt-4o" },
  ]);
  const [disabledInstances, setDisabledInstances] = useState<string[]>([]);
  return (
    <ChatHeader
      selectedInstances={instances}
      onInstancesChange={setInstances}
      availableModels={mockModels}
      totalUsage={makeUsage({
        inputTokens: 2500,
        outputTokens: 4500,
        totalTokens: 7000,
        cost: 0.045,
      })}
      canClear
      hasMessages
      onClear={() => console.log("Clear clicked")}
      disabledInstances={disabledInstances}
      onDisabledInstancesChange={setDisabledInstances}
      vectorStoreIds={["vs-789"]}
    />
  );
}

export const WithSingleCollection: Story = {
  render: () => <WithSingleCollectionStory />,
};

function WithDuplicateInstancesStory() {
  const [instances, setInstances] = useState<ModelInstance[]>([
    { id: "openai/gpt-4o", modelId: "openai/gpt-4o" },
    { id: "openai/gpt-4o-2", modelId: "openai/gpt-4o", label: "GPT-4o Creative" },
    { id: "openai/gpt-4o-3", modelId: "openai/gpt-4o", label: "GPT-4o Precise" },
  ]);
  const [disabledInstances, setDisabledInstances] = useState<string[]>([]);
  return (
    <ChatHeader
      selectedInstances={instances}
      onInstancesChange={setInstances}
      availableModels={mockModels}
      totalUsage={makeUsage({
        inputTokens: 1500,
        outputTokens: 4500,
        totalTokens: 6000,
        cost: 0.03,
      })}
      canClear
      hasMessages
      onClear={() => console.log("Clear clicked")}
      disabledInstances={disabledInstances}
      onDisabledInstancesChange={setDisabledInstances}
    />
  );
}

export const WithDuplicateInstances: Story = {
  render: () => <WithDuplicateInstancesStory />,
};

function WithLabelEditingStory() {
  const [instances, setInstances] = useState<ModelInstance[]>([
    { id: "anthropic/claude-3-opus", modelId: "anthropic/claude-3-opus" },
    { id: "openai/gpt-4o", modelId: "openai/gpt-4o", label: "GPT-4o Creative" },
  ]);
  const [disabledInstances, setDisabledInstances] = useState<string[]>([]);

  const handleLabelChange = (instanceId: string, label: string) => {
    setInstances((prev) =>
      prev.map((inst) => (inst.id === instanceId ? { ...inst, label: label || undefined } : inst))
    );
  };

  const handleParametersChange = (instanceId: string, params: ModelInstance["parameters"]) => {
    setInstances((prev) =>
      prev.map((inst) => (inst.id === instanceId ? { ...inst, parameters: params } : inst))
    );
  };

  return (
    <ChatHeader
      selectedInstances={instances}
      onInstancesChange={setInstances}
      availableModels={mockModels}
      totalUsage={makeUsage({
        inputTokens: 1000,
        outputTokens: 3000,
        totalTokens: 4000,
        cost: 0.02,
      })}
      canClear
      hasMessages
      onClear={() => console.log("Clear clicked")}
      disabledInstances={disabledInstances}
      onDisabledInstancesChange={setDisabledInstances}
      onInstanceParametersChange={handleParametersChange}
      onInstanceLabelChange={handleLabelChange}
    />
  );
}

export const WithLabelEditing: Story = {
  render: () => <WithLabelEditingStory />,
};

// Story showing mode overhead costs (routing, synthesis, voting, etc.)
function WithModeOverheadStory() {
  const [instances, setInstances] = useState<ModelInstance[]>([
    { id: "anthropic/claude-3-opus", modelId: "anthropic/claude-3-opus" },
    { id: "openai/gpt-4o", modelId: "openai/gpt-4o" },
    { id: "google/gemini-1.5-pro", modelId: "google/gemini-1.5-pro" },
  ]);
  const [disabledInstances, setDisabledInstances] = useState<string[]>([]);
  return (
    <ChatHeader
      selectedInstances={instances}
      onInstancesChange={setInstances}
      availableModels={mockModels}
      totalUsage={makeUsage(
        // Response usage
        {
          inputTokens: 2500,
          outputTokens: 8500,
          totalTokens: 11000,
          cost: 0.085,
        },
        // Mode overhead (router/synthesizer costs)
        {
          inputTokens: 350,
          outputTokens: 150,
          totalTokens: 500,
          cost: 0.0025,
        }
      )}
      canClear
      hasMessages
      onClear={() => console.log("Clear clicked")}
      disabledInstances={disabledInstances}
      onDisabledInstancesChange={setDisabledInstances}
    />
  );
}

export const WithModeOverhead: Story = {
  render: () => <WithModeOverheadStory />,
};

// Story showing the project picker
const mockConversationWithProject: Conversation = {
  ...mockConversation,
  projectId: "proj-1",
  projectName: "Production API",
};

function WithProjectPickerStory() {
  const [instances, setInstances] = useState<ModelInstance[]>([
    { id: "anthropic/claude-3-opus", modelId: "anthropic/claude-3-opus" },
    { id: "openai/gpt-4o", modelId: "openai/gpt-4o" },
  ]);
  const [disabledInstances, setDisabledInstances] = useState<string[]>([]);
  const [conv, setConv] = useState(mockConversationWithProject);
  return (
    <ChatHeader
      selectedInstances={instances}
      onInstancesChange={setInstances}
      availableModels={mockModels}
      totalUsage={makeUsage({
        inputTokens: 30,
        outputTokens: 160,
        totalTokens: 190,
        cost: 0.0045,
      })}
      canClear
      hasMessages
      onClear={() => console.log("Clear clicked")}
      disabledInstances={disabledInstances}
      onDisabledInstancesChange={setDisabledInstances}
      conversation={conv}
      onProjectChange={(projectId, projectName) => {
        setConv((c) => ({ ...c, projectId: projectId ?? undefined, projectName }));
        console.log("Project changed:", projectId, projectName);
      }}
    />
  );
}

export const WithProjectPicker: Story = {
  render: () => <WithProjectPickerStory />,
};
