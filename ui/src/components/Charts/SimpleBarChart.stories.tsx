import type { Meta, StoryObj } from "@storybook/react";
import { SimpleBarChart } from "./SimpleBarChart";

const barChartData = [
  { label: "2024-12-01", value: 45.2 },
  { label: "2024-12-02", value: 62.8 },
  { label: "2024-12-03", value: 38.5 },
  { label: "2024-12-04", value: 91.2 },
  { label: "2024-12-05", value: 55.0 },
];

const formatCurrency = (value: number) => `$${value.toFixed(2)}`;

const meta: Meta<typeof SimpleBarChart> = {
  title: "Components/Charts/SimpleBarChart",
  component: SimpleBarChart,
  parameters: {
    layout: "padded",
  },
};

export default meta;

type Story = StoryObj<typeof SimpleBarChart>;

export const Default: Story = {
  args: {
    data: barChartData,
    formatter: formatCurrency,
  },
};

export const CustomColor: Story = {
  args: {
    data: barChartData,
    formatter: formatCurrency,
    color: "bg-indigo-500",
  },
};

export const WithMaxValue: Story = {
  args: {
    data: barChartData,
    formatter: formatCurrency,
    maxValue: 150,
  },
};

export const ManyItems: Story = {
  args: {
    data: [
      { label: "gpt-4o", value: 450 },
      { label: "claude-3-opus", value: 280 },
      { label: "gpt-3.5-turbo", value: 180 },
      { label: "claude-3-sonnet", value: 90 },
      { label: "gemini-pro", value: 75 },
      { label: "llama-3-70b", value: 45 },
    ],
    formatter: formatCurrency,
  },
};
