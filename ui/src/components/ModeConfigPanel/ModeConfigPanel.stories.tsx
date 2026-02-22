import type { Meta, StoryObj } from "@storybook/react";
import { useState } from "react";

import type { ModeConfig, ModelInstance } from "@/components/chat-types";

import { ModeConfigPanel } from "./ModeConfigPanel";

const meta: Meta<typeof ModeConfigPanel> = {
  title: "Chat/ModeConfigPanel",
  component: ModeConfigPanel,

  decorators: [
    (Story) => (
      <div className="p-4">
        <Story />
      </div>
    ),
  ],
};

export default meta;
type Story = StoryObj<typeof ModeConfigPanel>;

// Standard instances with unique IDs
const availableInstances: ModelInstance[] = [
  { id: "claude-3-opus", modelId: "anthropic/claude-3-opus" },
  { id: "gpt-4-turbo", modelId: "openai/gpt-4-turbo" },
  { id: "gemini-1.5-pro", modelId: "google/gemini-1.5-pro" },
  { id: "llama-3.1-70b", modelId: "meta/llama-3.1-70b" },
];

// Instances with duplicates of the same model (to test multi-instance scenarios)
const multiInstanceInstances: ModelInstance[] = [
  { id: "gpt-4-creative", modelId: "openai/gpt-4-turbo", label: "GPT-4 (Creative)" },
  { id: "gpt-4-precise", modelId: "openai/gpt-4-turbo", label: "GPT-4 (Precise)" },
  { id: "claude-3-opus", modelId: "anthropic/claude-3-opus" },
  { id: "gemini-1.5-pro", modelId: "google/gemini-1.5-pro" },
];

function RoutedConfigDemo() {
  const [config, setConfig] = useState<ModeConfig>({});
  return (
    <ModeConfigPanel
      mode="routed"
      config={config}
      onConfigChange={setConfig}
      availableInstances={availableInstances}
    />
  );
}

export const RoutedMode: Story = {
  render: () => <RoutedConfigDemo />,
};

function ChainedConfigDemo() {
  const [config, setConfig] = useState<ModeConfig>({});
  return (
    <ModeConfigPanel
      mode="chained"
      config={config}
      onConfigChange={setConfig}
      availableInstances={availableInstances}
    />
  );
}

export const ChainedMode: Story = {
  render: () => <ChainedConfigDemo />,
};

function SynthesizedConfigDemo() {
  const [config, setConfig] = useState<ModeConfig>({});
  return (
    <ModeConfigPanel
      mode="synthesized"
      config={config}
      onConfigChange={setConfig}
      availableInstances={availableInstances}
    />
  );
}

export const SynthesizedMode: Story = {
  render: () => <SynthesizedConfigDemo />,
};

function SynthesizedMultiInstanceDemo() {
  const [config, setConfig] = useState<ModeConfig>({});
  return (
    <ModeConfigPanel
      mode="synthesized"
      config={config}
      onConfigChange={setConfig}
      availableInstances={multiInstanceInstances}
    />
  );
}

export const SynthesizedModeMultiInstance: Story = {
  name: "Synthesized Mode (Multi-Instance)",
  render: () => <SynthesizedMultiInstanceDemo />,
};

function RoutedMultiInstanceDemo() {
  const [config, setConfig] = useState<ModeConfig>({});
  return (
    <ModeConfigPanel
      mode="routed"
      config={config}
      onConfigChange={setConfig}
      availableInstances={multiInstanceInstances}
    />
  );
}

export const RoutedModeMultiInstance: Story = {
  name: "Routed Mode (Multi-Instance)",
  render: () => <RoutedMultiInstanceDemo />,
};

function CritiquedMultiInstanceDemo() {
  const [config, setConfig] = useState<ModeConfig>({});
  return (
    <ModeConfigPanel
      mode="critiqued"
      config={config}
      onConfigChange={setConfig}
      availableInstances={multiInstanceInstances}
    />
  );
}

export const CritiquedModeMultiInstance: Story = {
  name: "Critiqued Mode (Multi-Instance)",
  render: () => <CritiquedMultiInstanceDemo />,
};

function DebatedMultiInstanceDemo() {
  const [config, setConfig] = useState<ModeConfig>({});
  return (
    <ModeConfigPanel
      mode="debated"
      config={config}
      onConfigChange={setConfig}
      availableInstances={multiInstanceInstances}
    />
  );
}

export const DebatedModeMultiInstance: Story = {
  name: "Debated Mode (Multi-Instance)",
  render: () => <DebatedMultiInstanceDemo />,
};

function CouncilMultiInstanceDemo() {
  const [config, setConfig] = useState<ModeConfig>({});
  return (
    <ModeConfigPanel
      mode="council"
      config={config}
      onConfigChange={setConfig}
      availableInstances={multiInstanceInstances}
    />
  );
}

export const CouncilModeMultiInstance: Story = {
  name: "Council Mode (Multi-Instance)",
  render: () => <CouncilMultiInstanceDemo />,
};

function HierarchicalMultiInstanceDemo() {
  const [config, setConfig] = useState<ModeConfig>({});
  return (
    <ModeConfigPanel
      mode="hierarchical"
      config={config}
      onConfigChange={setConfig}
      availableInstances={multiInstanceInstances}
    />
  );
}

export const HierarchicalModeMultiInstance: Story = {
  name: "Hierarchical Mode (Multi-Instance)",
  render: () => <HierarchicalMultiInstanceDemo />,
};

function ConfidenceWeightedMultiInstanceDemo() {
  const [config, setConfig] = useState<ModeConfig>({});
  return (
    <ModeConfigPanel
      mode="confidence-weighted"
      config={config}
      onConfigChange={setConfig}
      availableInstances={multiInstanceInstances}
    />
  );
}

export const ConfidenceWeightedModeMultiInstance: Story = {
  name: "Confidence-Weighted Mode (Multi-Instance)",
  render: () => <ConfidenceWeightedMultiInstanceDemo />,
};

function ElectedConfigDemo() {
  const [config, setConfig] = useState<ModeConfig>({});
  return (
    <ModeConfigPanel
      mode="elected"
      config={config}
      onConfigChange={setConfig}
      availableInstances={availableInstances}
    />
  );
}

export const ElectedMode: Story = {
  render: () => <ElectedConfigDemo />,
};

export const MultipleMode: Story = {
  args: {
    mode: "multiple",
    config: {},
    onConfigChange: () => {},
    availableInstances,
  },
};

function DisabledDemo() {
  const [config, setConfig] = useState<ModeConfig>({});
  return (
    <ModeConfigPanel
      mode="routed"
      config={config}
      onConfigChange={setConfig}
      availableInstances={availableInstances}
      disabled
    />
  );
}

export const Disabled: Story = {
  render: () => <DisabledDemo />,
};
