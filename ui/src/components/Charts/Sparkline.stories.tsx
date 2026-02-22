import type { Meta, StoryObj } from "@storybook/react";
import { Sparkline } from "./Sparkline";

const sparklineData = [12, 18, 15, 22, 19, 25, 31, 28, 35, 42];

const meta: Meta<typeof Sparkline> = {
  title: "Components/Charts/Sparkline",
  component: Sparkline,
  parameters: {
    layout: "padded",
  },
};

export default meta;

type Story = StoryObj<typeof Sparkline>;

export const Default: Story = {
  args: {
    data: sparklineData,
  },
};

export const Large: Story = {
  args: {
    data: sparklineData,
    width: 120,
    height: 40,
  },
};

export const LineOnly: Story = {
  args: {
    data: sparklineData,
    showArea: false,
  },
};

export const CustomColor: Story = {
  args: {
    data: sparklineData,
    color: "#ec4899",
  },
};

export const Downtrend: Story = {
  args: {
    data: [42, 35, 28, 31, 25, 19, 22, 15, 18, 12],
    color: "#ef4444",
  },
};

export const InlineWithText: Story = {
  render: () => (
    <div className="flex items-center gap-8">
      <div className="flex items-center gap-2">
        <span className="text-sm text-muted-foreground">Requests:</span>
        <Sparkline data={sparklineData} />
        <span className="font-mono text-sm font-medium text-success">+15%</span>
      </div>
      <div className="flex items-center gap-2">
        <span className="text-sm text-muted-foreground">Cost:</span>
        <Sparkline data={[42, 35, 28, 31, 25, 19, 22, 15, 18, 12]} color="#ef4444" />
        <span className="font-mono text-sm font-medium text-destructive">-28%</span>
      </div>
    </div>
  ),
};
