import type { Meta, StoryObj } from "@storybook/react";
import { ChartLegend } from "./ChartLegend";

const modelDistributionData = [
  { name: "gpt-4o", value: 450 },
  { name: "claude-3-opus", value: 280 },
  { name: "gpt-3.5-turbo", value: 180 },
  { name: "claude-3-sonnet", value: 90 },
];

const formatCurrency = (value: number) => `$${value.toFixed(2)}`;

const meta: Meta<typeof ChartLegend> = {
  title: "Components/Charts/ChartLegend",
  component: ChartLegend,
  parameters: {
    layout: "padded",
  },
};

export default meta;

type Story = StoryObj<typeof ChartLegend>;

export const Default: Story = {
  args: {
    items: modelDistributionData.map((d) => ({ name: d.name, value: d.value })),
    formatter: formatCurrency,
  },
};

export const WithCustomColors: Story = {
  args: {
    items: [
      { name: "Success", value: 95, color: "#10b981" },
      { name: "Warning", value: 3, color: "#f59e0b" },
      { name: "Error", value: 2, color: "#ef4444" },
    ],
    formatter: (v) => `${v}%`,
  },
};

export const WithoutValues: Story = {
  args: {
    items: [{ name: "Series A" }, { name: "Series B" }, { name: "Series C" }],
  },
};
