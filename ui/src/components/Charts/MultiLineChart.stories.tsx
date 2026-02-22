import type { Meta, StoryObj } from "@storybook/react";
import { MultiLineChart } from "./MultiLineChart";

const latencyData = [
  { time: "09:00", p50: 120, p95: 180, p99: 250 },
  { time: "10:00", p50: 115, p95: 190, p99: 280 },
  { time: "11:00", p50: 130, p95: 200, p99: 310 },
  { time: "12:00", p50: 145, p95: 220, p99: 340 },
  { time: "13:00", p50: 135, p95: 210, p99: 290 },
  { time: "14:00", p50: 125, p95: 185, p99: 260 },
  { time: "15:00", p50: 118, p95: 175, p99: 245 },
];

const errorRateData = [
  { time: "09:00", errorRate: 0.5 },
  { time: "10:00", errorRate: 0.8 },
  { time: "11:00", errorRate: 1.2 },
  { time: "12:00", errorRate: 2.5 },
  { time: "13:00", errorRate: 1.8 },
  { time: "14:00", errorRate: 0.9 },
  { time: "15:00", errorRate: 0.4 },
];

const mixedData = [
  { time: "09:00", requests: 1200, errors: 6 },
  { time: "10:00", requests: 1500, errors: 12 },
  { time: "11:00", requests: 1800, errors: 22 },
  { time: "12:00", requests: 2200, errors: 55 },
  { time: "13:00", requests: 1900, errors: 34 },
  { time: "14:00", requests: 1600, errors: 14 },
  { time: "15:00", requests: 1400, errors: 6 },
];

const sparseData = [
  { time: "09:00", p50: 120, p95: null, p99: 250 },
  { time: "10:00", p50: 115, p95: 190, p99: null },
  { time: "11:00", p50: null, p95: 200, p99: 310 },
  { time: "12:00", p50: 145, p95: null, p99: null },
  { time: "13:00", p50: 135, p95: 210, p99: 290 },
  { time: "14:00", p50: null, p95: 185, p99: 260 },
  { time: "15:00", p50: 118, p95: 175, p99: 245 },
];

const formatMs = (value: number) => `${value}ms`;
const formatPercent = (value: number) => `${value.toFixed(1)}%`;

const meta: Meta<typeof MultiLineChart> = {
  title: "Components/Charts/MultiLineChart",
  component: MultiLineChart,
  parameters: {
    layout: "padded",
  },
};

export default meta;

type Story = StoryObj<typeof MultiLineChart>;

export const LatencyPercentiles: Story = {
  args: {
    data: latencyData,
    xKey: "time",
    series: [
      { dataKey: "p50", name: "P50" },
      { dataKey: "p95", name: "P95" },
      { dataKey: "p99", name: "P99" },
    ],
    height: 250,
    formatter: formatMs,
  },
};

export const SingleSeries: Story = {
  args: {
    data: errorRateData,
    xKey: "time",
    series: [{ dataKey: "errorRate", name: "Error Rate", color: "#ef4444" }],
    height: 250,
    formatter: formatPercent,
  },
};

export const CustomColors: Story = {
  args: {
    data: latencyData,
    xKey: "time",
    series: [
      { dataKey: "p50", name: "P50", color: "#22c55e" },
      { dataKey: "p95", name: "P95", color: "#f59e0b" },
      { dataKey: "p99", name: "P99", color: "#ef4444" },
    ],
    height: 250,
    formatter: formatMs,
  },
};

export const RequestsAndErrors: Story = {
  args: {
    data: mixedData,
    xKey: "time",
    series: [
      { dataKey: "requests", name: "Requests", color: "#3b82f6" },
      { dataKey: "errors", name: "Errors", color: "#ef4444" },
    ],
    height: 250,
  },
};

export const NoGrid: Story = {
  args: {
    data: latencyData,
    xKey: "time",
    series: [
      { dataKey: "p50", name: "P50" },
      { dataKey: "p95", name: "P95" },
      { dataKey: "p99", name: "P99" },
    ],
    height: 250,
    formatter: formatMs,
    showGrid: false,
  },
};

export const NoLegend: Story = {
  args: {
    data: latencyData,
    xKey: "time",
    series: [
      { dataKey: "p50", name: "P50" },
      { dataKey: "p95", name: "P95" },
      { dataKey: "p99", name: "P99" },
    ],
    height: 250,
    formatter: formatMs,
    showLegend: false,
  },
};

export const SparseData: Story = {
  args: {
    data: sparseData,
    xKey: "time",
    series: [
      { dataKey: "p50", name: "P50" },
      { dataKey: "p95", name: "P95" },
      { dataKey: "p99", name: "P99" },
    ],
    height: 250,
    formatter: formatMs,
  },
};
