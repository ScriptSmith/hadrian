import type { Meta, StoryObj } from "@storybook/react";
import { LineChart } from "./LineChart";

const timeSeriesData = [
  { date: "2024-01", cost: 120.5 },
  { date: "2024-02", cost: 180.25 },
  { date: "2024-03", cost: 150.0 },
  { date: "2024-04", cost: 220.75 },
  { date: "2024-05", cost: 190.3 },
  { date: "2024-06", cost: 250.0 },
  { date: "2024-07", cost: 310.5 },
];

const formatCurrency = (value: number) => `$${value.toFixed(2)}`;

const meta: Meta<typeof LineChart> = {
  title: "Components/Charts/LineChart",
  component: LineChart,
  parameters: {
    layout: "padded",
  },
};

export default meta;

type Story = StoryObj<typeof LineChart>;

export const Default: Story = {
  args: {
    data: timeSeriesData,
    xKey: "date",
    yKey: "cost",
    height: 250,
    formatter: formatCurrency,
  },
};

export const WithArea: Story = {
  args: {
    data: timeSeriesData,
    xKey: "date",
    yKey: "cost",
    height: 250,
    formatter: formatCurrency,
    showArea: true,
  },
};

export const NoGrid: Story = {
  args: {
    data: timeSeriesData,
    xKey: "date",
    yKey: "cost",
    height: 250,
    formatter: formatCurrency,
    showGrid: false,
  },
};

export const CustomColor: Story = {
  args: {
    data: timeSeriesData,
    xKey: "date",
    yKey: "cost",
    height: 250,
    formatter: formatCurrency,
    color: "#6366f1",
  },
};
