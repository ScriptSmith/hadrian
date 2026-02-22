import type { Meta, StoryObj } from "@storybook/react";
import { Brain, Wrench, Eye, Braces, Scale } from "lucide-react";

import { TooltipProvider } from "@/components/Tooltip/Tooltip";

import { CapabilityBadge } from "./CapabilityBadge";

const meta = {
  title: "Components/ModelPicker/CapabilityBadge",
  component: CapabilityBadge,
  parameters: {
    layout: "centered",
  },
  decorators: [
    (Story) => (
      <TooltipProvider>
        <Story />
      </TooltipProvider>
    ),
  ],
} satisfies Meta<typeof CapabilityBadge>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Reasoning: Story = {
  args: {
    icon: Brain,
    label: "Reasoning",
    color: "purple",
  },
};

export const ToolCalling: Story = {
  args: {
    icon: Wrench,
    label: "Tool Calling",
    color: "green",
  },
};

export const Vision: Story = {
  args: {
    icon: Eye,
    label: "Vision",
    color: "cyan",
  },
};

export const StructuredOutput: Story = {
  args: {
    icon: Braces,
    label: "Structured Output (JSON)",
    color: "amber",
  },
};

export const OpenWeights: Story = {
  args: {
    icon: Scale,
    label: "Open Weights",
    color: "indigo",
  },
};

/** All capability badges displayed together */
export const AllBadges: Story = {
  render: () => (
    <div className="flex items-center gap-2">
      <CapabilityBadge icon={Brain} label="Reasoning" color="purple" />
      <CapabilityBadge icon={Wrench} label="Tool Calling" color="green" />
      <CapabilityBadge icon={Eye} label="Vision" color="cyan" />
      <CapabilityBadge icon={Braces} label="Structured Output (JSON)" color="amber" />
      <CapabilityBadge icon={Scale} label="Open Weights" color="indigo" />
    </div>
  ),
};
