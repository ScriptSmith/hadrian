import type { Meta, StoryObj } from "@storybook/react";
import { ToolExecutionBlock } from "./ToolExecutionBlock";
import type { ToolExecutionRound, ToolExecution, Artifact } from "@/components/chat-types";

const meta = {
  title: "Chat/ToolExecution/ToolExecutionBlock",
  component: ToolExecutionBlock,
  parameters: {
    layout: "padded",
  },
} satisfies Meta<typeof ToolExecutionBlock>;

export default meta;
type Story = StoryObj<typeof meta>;

const makeCodeArtifact = (code: string): Artifact => ({
  id: `code-${Math.random().toString(36).slice(2)}`,
  type: "code",
  data: { language: "python", code },
  role: "input",
});

const makeChartArtifact = (): Artifact => ({
  id: `chart-${Math.random().toString(36).slice(2)}`,
  type: "chart",
  title: "Generated Chart",
  data: {
    spec: {
      $schema: "https://vega.github.io/schema/vega-lite/v5.json",
      data: { values: [{ x: 1, y: 2 }] },
      mark: "point",
      encoding: { x: { field: "x" }, y: { field: "y" } },
    },
  },
  role: "output",
});

const makeExecution = (
  toolName: string,
  status: ToolExecution["status"],
  inputCode?: string,
  outputArtifact?: Artifact,
  duration?: number
): ToolExecution => ({
  id: `exec-${Math.random().toString(36).slice(2)}`,
  toolName,
  status,
  startTime: Date.now() - (duration || 0),
  endTime: status !== "running" ? Date.now() : undefined,
  duration,
  input: {},
  inputArtifacts: inputCode ? [makeCodeArtifact(inputCode)] : [],
  outputArtifacts: outputArtifact ? [outputArtifact] : [],
  round: 1,
});

const makeRound = (
  round: number,
  executions: ToolExecution[],
  modelReasoning?: string
): ToolExecutionRound => ({
  round,
  executions,
  modelReasoning,
  totalDuration: executions.reduce((sum, e) => sum + (e.duration || 0), 0),
});

export const Collapsed: Story = {
  args: {
    rounds: [
      makeRound(1, [
        makeExecution(
          "code_interpreter",
          "success",
          "import matplotlib",
          makeChartArtifact(),
          1500
        ),
      ]),
    ],
    defaultExpanded: false,
  },
};

export const Expanded: Story = {
  args: {
    rounds: [
      makeRound(1, [
        makeExecution(
          "code_interpreter",
          "success",
          "import matplotlib",
          makeChartArtifact(),
          1500
        ),
      ]),
    ],
    defaultExpanded: true,
  },
};

export const Streaming: Story = {
  args: {
    rounds: [makeRound(1, [makeExecution("code_interpreter", "running", "# Running analysis...")])],
    isStreaming: true,
  },
};

export const WithDisplaySelection: Story = {
  args: {
    rounds: [
      makeRound(1, [
        makeExecution(
          "code_interpreter",
          "success",
          "plt.plot([1,2,3])",
          { ...makeChartArtifact(), id: "chart-selected" },
          1200
        ),
      ]),
    ],
    displaySelection: {
      artifactIds: ["chart-selected"],
      layout: "inline",
    },
  },
};

export const MultipleRounds: Story = {
  args: {
    rounds: [
      makeRound(
        1,
        [makeExecution("code_interpreter", "success", "# Round 1", undefined, 500)],
        "Let me refine this..."
      ),
      makeRound(2, [
        makeExecution("code_interpreter", "success", "# Round 2", makeChartArtifact(), 800),
      ]),
    ],
  },
};
