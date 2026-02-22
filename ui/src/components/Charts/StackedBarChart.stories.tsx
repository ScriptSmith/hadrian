import type { Meta, StoryObj } from "@storybook/react";
import { StackedBarChart } from "./StackedBarChart";

const costByModel = [
  { date: "12/01", "gpt-4o": 1.2, "claude-opus": 0.8, "gpt-4o-mini": 0.3 },
  { date: "12/02", "gpt-4o": 1.5, "claude-opus": 0.9, "gpt-4o-mini": 0.4 },
  { date: "12/03", "gpt-4o": 1.1, "claude-opus": 1.2, "gpt-4o-mini": 0.2 },
  { date: "12/04", "gpt-4o": 1.8, "claude-opus": 0.7, "gpt-4o-mini": 0.5 },
  { date: "12/05", "gpt-4o": 1.3, "claude-opus": 1.1, "gpt-4o-mini": 0.3 },
  { date: "12/06", "gpt-4o": 0.9, "claude-opus": 0.6, "gpt-4o-mini": 0.2 },
  { date: "12/07", "gpt-4o": 1.6, "claude-opus": 1.0, "gpt-4o-mini": 0.4 },
];

const requestsByProvider = [
  { date: "12/01", openai: 450, anthropic: 320 },
  { date: "12/02", openai: 520, anthropic: 380 },
  { date: "12/03", openai: 480, anthropic: 410 },
  { date: "12/04", openai: 600, anthropic: 350 },
  { date: "12/05", openai: 550, anthropic: 390 },
  { date: "12/06", openai: 400, anthropic: 280 },
  { date: "12/07", openai: 580, anthropic: 370 },
];

const formatCurrency = (value: number) => `$${value.toFixed(2)}`;

const meta: Meta<typeof StackedBarChart> = {
  title: "Components/Charts/StackedBarChart",
  component: StackedBarChart,
  parameters: {
    layout: "padded",
  },
};

export default meta;

type Story = StoryObj<typeof StackedBarChart>;

export const CostByModel: Story = {
  args: {
    data: costByModel,
    xKey: "date",
    series: [
      { dataKey: "gpt-4o", name: "GPT-4o" },
      { dataKey: "claude-opus", name: "Claude Opus" },
      { dataKey: "gpt-4o-mini", name: "GPT-4o Mini" },
    ],
    height: 250,
    formatter: formatCurrency,
  },
};

export const RequestsByProvider: Story = {
  args: {
    data: requestsByProvider,
    xKey: "date",
    series: [
      { dataKey: "openai", name: "OpenAI" },
      { dataKey: "anthropic", name: "Anthropic" },
    ],
    height: 250,
  },
};

export const CustomColors: Story = {
  args: {
    data: costByModel,
    xKey: "date",
    series: [
      { dataKey: "gpt-4o", name: "GPT-4o", color: "#3b82f6" },
      { dataKey: "claude-opus", name: "Claude Opus", color: "#8b5cf6" },
      { dataKey: "gpt-4o-mini", name: "GPT-4o Mini", color: "#10b981" },
    ],
    height: 250,
    formatter: formatCurrency,
  },
};

export const NoLegend: Story = {
  args: {
    data: requestsByProvider,
    xKey: "date",
    series: [
      { dataKey: "openai", name: "OpenAI" },
      { dataKey: "anthropic", name: "Anthropic" },
    ],
    height: 250,
    showLegend: false,
  },
};
