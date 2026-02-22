import type { Meta, StoryObj } from "@storybook/react";
import { ChartArtifact } from "./ChartArtifact";
import type { Artifact } from "@/components/chat-types";

const meta = {
  title: "Chat/Artifacts/ChartArtifact",
  component: ChartArtifact,
  parameters: {
    layout: "padded",
  },
} satisfies Meta<typeof ChartArtifact>;

export default meta;
type Story = StoryObj<typeof meta>;

const makeArtifact = (spec: object): Artifact => ({
  id: "chart-1",
  type: "chart",
  title: "Chart",
  data: { spec },
});

export const BarChart: Story = {
  args: {
    artifact: makeArtifact({
      $schema: "https://vega.github.io/schema/vega-lite/v5.json",
      description: "A simple bar chart",
      data: {
        values: [
          { category: "A", value: 28 },
          { category: "B", value: 55 },
          { category: "C", value: 43 },
          { category: "D", value: 91 },
        ],
      },
      mark: "bar",
      encoding: {
        x: { field: "category", type: "nominal" },
        y: { field: "value", type: "quantitative" },
      },
    }),
  },
};

export const LineChart: Story = {
  args: {
    artifact: makeArtifact({
      $schema: "https://vega.github.io/schema/vega-lite/v5.json",
      description: "A line chart",
      data: {
        values: [
          { x: 0, y: 0 },
          { x: 1, y: 2 },
          { x: 2, y: 5 },
          { x: 3, y: 9 },
          { x: 4, y: 16 },
        ],
      },
      mark: "line",
      encoding: {
        x: { field: "x", type: "quantitative" },
        y: { field: "y", type: "quantitative" },
      },
    }),
  },
};

export const InvalidSpec: Story = {
  args: {
    artifact: {
      id: "chart-invalid",
      type: "chart",
      title: "Invalid Chart",
      data: { spec: { invalid: true } },
    },
  },
};
