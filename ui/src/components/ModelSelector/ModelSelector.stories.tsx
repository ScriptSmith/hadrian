import type { Meta, StoryObj } from "@storybook/react";
import { useState } from "react";
import { ModelSelector, type ModelInfo } from "./ModelSelector";
import { TooltipProvider } from "../Tooltip/Tooltip";
import { PreferencesProvider } from "@/preferences/PreferencesProvider";
import type { ModelInstance } from "@/components/chat-types";

const meta: Meta<typeof ModelSelector> = {
  title: "Chat/ModelSelector",
  component: ModelSelector,
  parameters: {
    layout: "centered",
  },

  decorators: [
    (Story) => (
      <PreferencesProvider>
        <TooltipProvider>
          <div style={{ width: 700 }}>
            <Story />
          </div>
        </TooltipProvider>
      </PreferencesProvider>
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
    id: "openai/gpt-4-turbo",
    owned_by: "openai",
    context_length: 128000,
    pricing: { prompt: "10", completion: "30" },
  },
  {
    id: "openai/gpt-4o",
    owned_by: "openai",
    context_length: 128000,
    pricing: { prompt: "5", completion: "15" },
  },
];

function DefaultStory() {
  const [instances, setInstances] = useState<ModelInstance[]>([]);
  return (
    <ModelSelector
      selectedInstances={instances}
      onInstancesChange={setInstances}
      availableModels={mockModels}
    />
  );
}

export const Default: Story = {
  render: () => <DefaultStory />,
};

function WithSelectedModelsStory() {
  const [instances, setInstances] = useState<ModelInstance[]>([
    { id: "anthropic/claude-3-opus", modelId: "anthropic/claude-3-opus" },
    { id: "openai/gpt-4o", modelId: "openai/gpt-4o" },
  ]);
  return (
    <ModelSelector
      selectedInstances={instances}
      onInstancesChange={setInstances}
      availableModels={mockModels}
    />
  );
}

export const WithSelectedModels: Story = {
  render: () => <WithSelectedModelsStory />,
};

function MaxModelsStory() {
  const [instances, setInstances] = useState<ModelInstance[]>([
    { id: "anthropic/claude-3-opus", modelId: "anthropic/claude-3-opus" },
  ]);
  return (
    <ModelSelector
      selectedInstances={instances}
      onInstancesChange={setInstances}
      availableModels={mockModels}
      maxModels={2}
    />
  );
}

export const MaxModels: Story = {
  render: () => <MaxModelsStory />,
};

function WithDisabledInstancesStory() {
  const [instances, setInstances] = useState<ModelInstance[]>([
    { id: "anthropic/claude-3-opus", modelId: "anthropic/claude-3-opus" },
    { id: "openai/gpt-4o", modelId: "openai/gpt-4o" },
    { id: "anthropic/claude-3-sonnet", modelId: "anthropic/claude-3-sonnet" },
  ]);
  const [disabledInstances, setDisabledInstances] = useState<string[]>(["openai/gpt-4o"]);

  return (
    <ModelSelector
      selectedInstances={instances}
      onInstancesChange={setInstances}
      availableModels={mockModels}
      disabledInstances={disabledInstances}
      onDisabledInstancesChange={setDisabledInstances}
      hasMessages={true}
    />
  );
}

export const WithDisabledInstances: Story = {
  render: () => <WithDisabledInstancesStory />,
};

function InConversationStory() {
  const [instances, setInstances] = useState<ModelInstance[]>([
    { id: "anthropic/claude-3-opus", modelId: "anthropic/claude-3-opus" },
    { id: "openai/gpt-4o", modelId: "openai/gpt-4o" },
  ]);
  const [disabledInstances, setDisabledInstances] = useState<string[]>([]);

  return (
    <ModelSelector
      selectedInstances={instances}
      onInstancesChange={setInstances}
      availableModels={mockModels}
      disabledInstances={disabledInstances}
      onDisabledInstancesChange={setDisabledInstances}
      hasMessages={true}
    />
  );
}

export const InConversation: Story = {
  render: () => <InConversationStory />,
};

function DraggableInstancesStory() {
  const [instances, setInstances] = useState<ModelInstance[]>([
    { id: "anthropic/claude-3-opus", modelId: "anthropic/claude-3-opus" },
    { id: "openai/gpt-4o", modelId: "openai/gpt-4o" },
    { id: "anthropic/claude-3-sonnet", modelId: "anthropic/claude-3-sonnet" },
    { id: "openai/gpt-4-turbo", modelId: "openai/gpt-4-turbo" },
  ]);

  return (
    <div className="space-y-4">
      <p className="text-sm text-muted-foreground">
        Drag the grip handle to reorder models. Current order:{" "}
        {instances.map((i) => i.modelId.split("/")[1]).join(", ")}
      </p>
      <ModelSelector
        selectedInstances={instances}
        onInstancesChange={setInstances}
        availableModels={mockModels}
      />
    </div>
  );
}

export const DraggableInstances: Story = {
  render: () => <DraggableInstancesStory />,
};

function WithDuplicateInstancesStory() {
  const [instances, setInstances] = useState<ModelInstance[]>([
    { id: "openai/gpt-4o", modelId: "openai/gpt-4o" },
    { id: "openai/gpt-4o-2", modelId: "openai/gpt-4o", label: "GPT-4o Creative" },
    { id: "openai/gpt-4o-3", modelId: "openai/gpt-4o", label: "GPT-4o Precise" },
  ]);

  return (
    <div className="space-y-4">
      <p className="text-sm text-muted-foreground">
        Multiple instances of the same model with different labels. Click the + button on a selected
        model in the picker to add another instance.
      </p>
      <ModelSelector
        selectedInstances={instances}
        onInstancesChange={setInstances}
        availableModels={mockModels}
      />
    </div>
  );
}

export const WithDuplicateInstances: Story = {
  render: () => <WithDuplicateInstancesStory />,
};

function WithLabelEditingStory() {
  const [instances, setInstances] = useState<ModelInstance[]>([
    { id: "openai/gpt-4o", modelId: "openai/gpt-4o" },
    { id: "anthropic/claude-3-opus", modelId: "anthropic/claude-3-opus", label: "Claude Creative" },
  ]);

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
    <div className="space-y-4">
      <p className="text-sm text-muted-foreground">
        Click the settings icon on a model chip to edit its label and parameters. The second model
        has a custom label &quot;Claude Creative&quot;.
      </p>
      <ModelSelector
        selectedInstances={instances}
        onInstancesChange={setInstances}
        availableModels={mockModels}
        onInstanceParametersChange={handleParametersChange}
        onInstanceLabelChange={handleLabelChange}
      />
    </div>
  );
}

export const WithLabelEditing: Story = {
  render: () => <WithLabelEditingStory />,
};

function WithDuplicateSettingsStory() {
  const [instances, setInstances] = useState<ModelInstance[]>([
    {
      id: "openai/gpt-4o",
      modelId: "openai/gpt-4o",
      label: "GPT-4o Creative",
      parameters: { temperature: 0.9, maxTokens: 2048 },
    },
    {
      id: "anthropic/claude-3-opus",
      modelId: "anthropic/claude-3-opus",
      parameters: { temperature: 0.3 },
    },
  ]);

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
    <div className="space-y-4">
      <p className="text-sm text-muted-foreground">
        Click the copy icon on any model chip to duplicate it with all its settings. The first model
        has custom temperature (0.9) and max tokens (2048).
      </p>
      <ModelSelector
        selectedInstances={instances}
        onInstancesChange={setInstances}
        availableModels={mockModels}
        onInstanceParametersChange={handleParametersChange}
        onInstanceLabelChange={handleLabelChange}
      />
      <div className="text-xs text-muted-foreground">
        Current instances:{" "}
        {instances
          .map((i) => `${i.label || i.modelId} (temp=${i.parameters?.temperature ?? "default"})`)
          .join(", ")}
      </div>
    </div>
  );
}

export const WithDuplicateSettings: Story = {
  render: () => <WithDuplicateSettingsStory />,
};

function LoadingStory() {
  const [instances, setInstances] = useState<ModelInstance[]>([]);
  return (
    <ModelSelector
      selectedInstances={instances}
      onInstancesChange={setInstances}
      availableModels={[]}
      isLoading={true}
    />
  );
}

export const Loading: Story = {
  render: () => <LoadingStory />,
};

function NoModelsAvailableStory() {
  const [instances, setInstances] = useState<ModelInstance[]>([]);
  return (
    <ModelSelector
      selectedInstances={instances}
      onInstancesChange={setInstances}
      availableModels={[]}
      isLoading={false}
    />
  );
}

export const NoModelsAvailable: Story = {
  render: () => <NoModelsAvailableStory />,
};
