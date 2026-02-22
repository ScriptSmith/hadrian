import type { Meta, StoryObj } from "@storybook/react";
import { useState } from "react";

import type { ReasoningConfig as ReasoningConfigType } from "@/components/chat-types";
import { DEFAULT_REASONING_CONFIG } from "@/components/chat-types";

import { ReasoningConfig } from "./ReasoningConfig";

const meta: Meta<typeof ReasoningConfig> = {
  title: "Components/ReasoningConfig",
  component: ReasoningConfig,
  parameters: {
    layout: "centered",
  },
};

export default meta;
type Story = StoryObj<typeof ReasoningConfig>;

function ReasoningConfigWrapper({
  initialConfig = DEFAULT_REASONING_CONFIG,
  disabled = false,
}: {
  initialConfig?: ReasoningConfigType;
  disabled?: boolean;
}) {
  const [config, setConfig] = useState<ReasoningConfigType>(initialConfig);

  const handleChange = (partial: Partial<ReasoningConfigType>) => {
    setConfig((prev) => ({ ...prev, ...partial }));
  };

  return (
    <div className="space-y-4">
      <ReasoningConfig config={config} onConfigChange={handleChange} disabled={disabled} />
      <div className="text-xs text-muted-foreground">Config: {JSON.stringify(config)}</div>
    </div>
  );
}

export const Default: Story = {
  render: () => <ReasoningConfigWrapper />,
};

export const Disabled: Story = {
  render: () => <ReasoningConfigWrapper disabled />,
};

export const ReasoningOff: Story = {
  render: () => <ReasoningConfigWrapper initialConfig={{ enabled: false, effort: "none" }} />,
};

export const HighEffort: Story = {
  render: () => <ReasoningConfigWrapper initialConfig={{ enabled: true, effort: "high" }} />,
};

export const MinimalEffort: Story = {
  render: () => <ReasoningConfigWrapper initialConfig={{ enabled: true, effort: "minimal" }} />,
};
