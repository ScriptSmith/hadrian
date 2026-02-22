import type { Meta, StoryObj } from "@storybook/react";
import { PieChart } from "./PieChart";

const modelDistributionData = [
  { name: "gpt-4o", value: 450 },
  { name: "claude-3-opus", value: 280 },
  { name: "gpt-3.5-turbo", value: 180 },
  { name: "claude-3-sonnet", value: 90 },
];

const formatCurrency = (value: number) => `$${value.toFixed(2)}`;

const meta: Meta<typeof PieChart> = {
  title: "Components/Charts/PieChart",
  component: PieChart,
  parameters: {
    layout: "padded",
  },
};

export default meta;

type Story = StoryObj<typeof PieChart>;

export const Default: Story = {
  args: {
    data: modelDistributionData,
    height: 250,
    formatter: formatCurrency,
  },
};

export const WithLabels: Story = {
  args: {
    data: modelDistributionData,
    height: 300,
    formatter: formatCurrency,
    showLabel: true,
    outerRadius: 100,
  },
};

export const Solid: Story = {
  args: {
    data: modelDistributionData,
    height: 250,
    formatter: formatCurrency,
    innerRadius: 0,
  },
  name: "Solid (No Inner Radius)",
};
