import type { Meta, StoryObj } from "@storybook/react";
import { expect, within, userEvent } from "storybook/test";

import { useState } from "react";
import { Artifact, ArtifactList } from "./Artifact";
import { ArtifactModal } from "./ArtifactModal";
import type { Artifact as ArtifactType } from "@/components/chat-types";
import { PreferencesProvider } from "@/preferences/PreferencesProvider";

const meta: Meta<typeof Artifact> = {
  title: "Chat/Artifact",
  component: Artifact,
  parameters: {
    layout: "padded",
  },
  decorators: [
    (Story) => (
      <PreferencesProvider>
        <div className="max-w-2xl">
          <Story />
        </div>
      </PreferencesProvider>
    ),
  ],
};

export default meta;
type Story = StoryObj<typeof Artifact>;

// Sample artifacts for stories
const codeArtifact: ArtifactType = {
  id: "code-1",
  type: "code",
  title: "Python Output",
  data: {
    language: "python",
    code: `def fibonacci(n):
    """Generate Fibonacci sequence up to n terms."""
    a, b = 0, 1
    result = []
    for _ in range(n):
        result.append(a)
        a, b = b, a + b
    return result

# Generate first 10 Fibonacci numbers
print(fibonacci(10))
# Output: [0, 1, 1, 2, 3, 5, 8, 13, 21, 34]`,
  },
};

const tableArtifact: ArtifactType = {
  id: "table-1",
  type: "table",
  title: "Sales Data Q4 2024",
  data: {
    columns: [
      { key: "product", label: "Product" },
      { key: "units", label: "Units Sold" },
      { key: "revenue", label: "Revenue ($)" },
      { key: "growth", label: "Growth (%)" },
    ],
    rows: [
      { product: "Widget A", units: 1250, revenue: 45000, growth: 12.5 },
      { product: "Widget B", units: 890, revenue: 32100, growth: -3.2 },
      { product: "Gadget X", units: 2100, revenue: 84000, growth: 28.7 },
      { product: "Gadget Y", units: 567, revenue: 22680, growth: 5.1 },
      { product: "Device Z", units: 3400, revenue: 136000, growth: 45.3 },
    ],
  },
};

const imageArtifact: ArtifactType = {
  id: "image-1",
  type: "image",
  title: "Generated Chart",
  mimeType: "image/svg+xml",
  data: `data:image/svg+xml;base64,${btoa(`
    <svg xmlns="http://www.w3.org/2000/svg" width="400" height="200" viewBox="0 0 400 200">
      <rect width="400" height="200" fill="#f0f0f0"/>
      <text x="200" y="100" text-anchor="middle" font-family="sans-serif" font-size="16" fill="#333">
        Sample Chart Placeholder
      </text>
      <rect x="50" y="120" width="60" height="50" fill="#4CAF50"/>
      <rect x="130" y="100" width="60" height="70" fill="#2196F3"/>
      <rect x="210" y="80" width="60" height="90" fill="#FF9800"/>
      <rect x="290" y="60" width="60" height="110" fill="#9C27B0"/>
    </svg>
  `)}`,
};

const chartArtifact: ArtifactType = {
  id: "chart-1",
  type: "chart",
  title: "Monthly Revenue",
  data: {
    spec: {
      $schema: "https://vega.github.io/schema/vega-lite/v6.json",
      description: "A simple bar chart with embedded data.",
      data: {
        values: [
          { month: "Jan", revenue: 28000 },
          { month: "Feb", revenue: 35000 },
          { month: "Mar", revenue: 42000 },
          { month: "Apr", revenue: 38000 },
          { month: "May", revenue: 51000 },
          { month: "Jun", revenue: 49000 },
        ],
      },
      mark: "bar",
      encoding: {
        x: { field: "month", type: "ordinal" },
        y: { field: "revenue", type: "quantitative" },
      },
    },
  },
};

const htmlArtifact: ArtifactType = {
  id: "html-1",
  type: "html",
  title: "Interactive Widget",
  data: `
    <div style="padding: 20px; text-align: center;">
      <h2 style="color: #333; margin-bottom: 16px;">Interactive Counter</h2>
      <div id="counter" style="font-size: 48px; font-weight: bold; color: #2196F3;">0</div>
      <button onclick="document.getElementById('counter').textContent = parseInt(document.getElementById('counter').textContent) + 1"
              style="margin-top: 16px; padding: 8px 24px; font-size: 16px; cursor: pointer; background: #4CAF50; color: white; border: none; border-radius: 4px;">
        Increment
      </button>
    </div>
  `,
};

const agentArtifact: ArtifactType = {
  id: "agent-1",
  type: "agent",
  title: "Sub-Agent (gpt-4o)",
  data: {
    task: "Research the latest developments in quantum computing and summarize the key breakthroughs from 2024.",
    model: "openai/gpt-4o",
    internal: `Let me investigate the latest quantum computing developments from 2024.

First, I'll consider the major players and their announcements:

**Google Quantum AI:**
- They've been working on error correction and recently published results showing logical qubit error rates are now lower than physical qubit rates. This is a significant milestone because it proves that adding more qubits for error correction actually helps rather than hurts.

**IBM:**
- Their roadmap has them at 1,000+ qubits now with the Condor processor. They've been focusing on fault-tolerant computing demonstrations.

**Practical Applications:**
- I'm seeing reports of quantum advantage being demonstrated for real commercial problems, particularly in logistics optimization. This is different from the artificial benchmarks we've seen before.
- Drug discovery is another area where quantum ML is showing promise, with some pharma companies reporting 100x speedups for certain molecular simulations.

**Hardware Innovations:**
- There have been interesting developments in room-temperature operation, though these are still experimental
- Photonic approaches are gaining traction with higher qubit counts

Let me also consider what's still hype vs reality...`,
    output: `## Quantum Computing Breakthroughs in 2024

### 1. Error Correction Advances
- Google achieved logical qubit error rates below physical qubit rates
- IBM demonstrated fault-tolerant computing on 1,000+ qubit processor

### 2. Practical Applications
- First commercial quantum advantage for logistics optimization
- 100x speedup for certain drug discovery simulations

### 3. Hardware Progress
- Room-temperature superconducting qubits (experimental)
- Photonic quantum computers reaching 1,000 qubits`,
    usage: {
      inputTokens: 312,
      outputTokens: 489,
      totalTokens: 801,
      cost: 0.0164,
    },
  },
};

const agentArtifactWithoutUsage: ArtifactType = {
  id: "agent-2",
  type: "agent",
  title: "Sub-Agent (claude-3-sonnet)",
  data: {
    task: "Explain the difference between TCP and UDP protocols",
    model: "anthropic/claude-3-sonnet",
    internal: `Let me think through the key differences between TCP and UDP...

TCP (Transmission Control Protocol):
- Connection-oriented: requires a handshake before data transfer
- Reliable: guarantees delivery through acknowledgments and retransmission
- Ordered: packets arrive in sequence
- Flow control: adjusts speed based on receiver capacity
- Overhead: more data is sent for reliability features

UDP (User Datagram Protocol):
- Connectionless: no handshake needed
- Unreliable: no guarantee packets arrive
- Unordered: packets may arrive out of sequence
- No flow control: sends at whatever rate
- Low overhead: minimal header, faster

Use cases to consider:
- TCP makes sense when you need every byte to arrive correctly (web, email, files)
- UDP makes sense when speed matters more than completeness (streaming, gaming, DNS)`,
    output: `**TCP** - Connection-oriented, reliable, ordered delivery. Use for: web browsing, email, file transfers.

**UDP** - Connectionless, fast, no delivery guarantee. Use for: video streaming, gaming, VoIP.`,
  },
};

/** Code artifact with syntax highlighting */
export const Code: Story = {
  args: {
    artifact: codeArtifact,
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // Verify the artifact header shows correct type
    await expect(canvas.getByText("Python Output")).toBeInTheDocument();

    // Verify code content is visible
    await expect(canvas.getByText(/def fibonacci/)).toBeInTheDocument();

    // Verify language badge
    await expect(canvas.getByText("python")).toBeInTheDocument();
  },
};

/** Table artifact with sortable columns */
export const Table: Story = {
  args: {
    artifact: tableArtifact,
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // Verify table header
    await expect(canvas.getByText("Sales Data Q4 2024")).toBeInTheDocument();

    // Verify row count
    await expect(canvas.getByText("5 rows")).toBeInTheDocument();

    // Verify column headers exist
    await expect(canvas.getByRole("button", { name: /Product/ })).toBeInTheDocument();
    await expect(canvas.getByRole("button", { name: /Units Sold/ })).toBeInTheDocument();

    // Click to sort by Units Sold
    const unitsHeader = canvas.getByRole("button", { name: /Units Sold/ });
    await userEvent.click(unitsHeader);

    // Click again to reverse sort
    await userEvent.click(unitsHeader);
  },
};

/** Image artifact with zoom and download */
export const Image: Story = {
  args: {
    artifact: imageArtifact,
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // Verify the artifact header
    await expect(canvas.getByText("Generated Chart")).toBeInTheDocument();

    // Verify image is rendered
    const img = canvas.getByRole("img");
    await expect(img).toBeInTheDocument();
  },
};

/** Chart artifact rendered with Vega-Lite */
export const Chart: Story = {
  args: {
    artifact: chartArtifact,
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // Verify the artifact header
    await expect(canvas.getByText("Monthly Revenue")).toBeInTheDocument();

    // Wait for chart to render (vega-embed creates an SVG)
    await new Promise((resolve) => setTimeout(resolve, 1000));

    // Note: "Open in Vega Editor" button is rendered by vega-embed in its action dropdown menu
    // and is not easily testable with @testing-library. We verify the chart renders correctly instead.

    // Click to view spec
    const viewSpecButton = canvas.getByRole("button", { name: /View Spec/ });
    await userEvent.click(viewSpecButton);

    // Verify spec is now visible - look for vega-lite schema URL
    await expect(canvas.getByText(/vega\.github\.io\/schema\/vega-lite/)).toBeInTheDocument();

    // Click to hide spec
    await userEvent.click(canvas.getByRole("button", { name: /Hide Spec/ }));
  },
};

/** Line chart */
export const LineChart: Story = {
  args: {
    artifact: {
      id: "chart-line",
      type: "chart",
      title: "Stock Price Over Time",
      data: {
        spec: {
          $schema: "https://vega.github.io/schema/vega-lite/v6.json",
          description: "Stock price line chart",
          data: {
            values: [
              { date: "2024-01", price: 150 },
              { date: "2024-02", price: 165 },
              { date: "2024-03", price: 158 },
              { date: "2024-04", price: 172 },
              { date: "2024-05", price: 189 },
              { date: "2024-06", price: 195 },
            ],
          },
          mark: { type: "line", point: true },
          encoding: {
            x: { field: "date", type: "ordinal", title: "Month" },
            y: { field: "price", type: "quantitative", title: "Price ($)" },
          },
        },
      },
    },
  },
};

/** Scatter plot */
export const ScatterPlot: Story = {
  args: {
    artifact: {
      id: "chart-scatter",
      type: "chart",
      title: "Height vs Weight",
      data: {
        spec: {
          $schema: "https://vega.github.io/schema/vega-lite/v6.json",
          description: "Height vs Weight scatter plot",
          data: {
            values: [
              { height: 165, weight: 68, gender: "F" },
              { height: 170, weight: 72, gender: "M" },
              { height: 158, weight: 55, gender: "F" },
              { height: 180, weight: 85, gender: "M" },
              { height: 175, weight: 78, gender: "M" },
              { height: 162, weight: 60, gender: "F" },
              { height: 185, weight: 92, gender: "M" },
              { height: 168, weight: 65, gender: "F" },
            ],
          },
          mark: "point",
          encoding: {
            x: { field: "height", type: "quantitative", title: "Height (cm)" },
            y: { field: "weight", type: "quantitative", title: "Weight (kg)" },
            color: { field: "gender", type: "nominal", title: "Gender" },
          },
        },
      },
    },
  },
};

/** Pie/Arc chart */
export const PieChart: Story = {
  args: {
    artifact: {
      id: "chart-pie",
      type: "chart",
      title: "Market Share",
      data: {
        spec: {
          $schema: "https://vega.github.io/schema/vega-lite/v6.json",
          description: "Market share pie chart",
          data: {
            values: [
              { company: "Company A", share: 35 },
              { company: "Company B", share: 28 },
              { company: "Company C", share: 20 },
              { company: "Others", share: 17 },
            ],
          },
          mark: { type: "arc", innerRadius: 50 },
          encoding: {
            theta: { field: "share", type: "quantitative" },
            color: { field: "company", type: "nominal", title: "Company" },
          },
        },
      },
    },
  },
};

/** Area chart */
export const AreaChart: Story = {
  args: {
    artifact: {
      id: "chart-area",
      type: "chart",
      title: "Website Traffic",
      data: {
        spec: {
          $schema: "https://vega.github.io/schema/vega-lite/v6.json",
          description: "Website traffic area chart",
          data: {
            values: [
              { hour: 0, visitors: 120 },
              { hour: 4, visitors: 80 },
              { hour: 8, visitors: 450 },
              { hour: 12, visitors: 780 },
              { hour: 16, visitors: 650 },
              { hour: 20, visitors: 380 },
              { hour: 24, visitors: 150 },
            ],
          },
          mark: { type: "area", opacity: 0.7 },
          encoding: {
            x: { field: "hour", type: "quantitative", title: "Hour of Day" },
            y: { field: "visitors", type: "quantitative", title: "Visitors" },
          },
        },
      },
    },
  },
};

/** Chart with invalid spec (error state) */
export const ChartError: Story = {
  args: {
    artifact: {
      id: "chart-error",
      type: "chart",
      title: "Invalid Chart",
      data: {
        spec: {
          // Invalid spec - missing required fields
          mark: "bar",
          // No data or encoding
        },
      },
    },
  },
};

/** HTML artifact with preview and source toggle */
export const Html: Story = {
  args: {
    artifact: htmlArtifact,
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // Verify the artifact header
    await expect(canvas.getByText("Interactive Widget")).toBeInTheDocument();

    // Verify preview/source toggle exists
    await expect(canvas.getByRole("button", { name: /Preview/ })).toBeInTheDocument();
    await expect(canvas.getByRole("button", { name: /Source/ })).toBeInTheDocument();

    // Click to view source
    await userEvent.click(canvas.getByRole("button", { name: /Source/ }));

    // Verify source code is visible
    await expect(canvas.getByText(/Interactive Counter/)).toBeInTheDocument();

    // Switch back to preview
    await userEvent.click(canvas.getByRole("button", { name: /Preview/ }));
  },
};

/** Sub-agent artifact with task, internal reasoning, output, and usage stats */
export const Agent: Story = {
  args: {
    artifact: agentArtifact,
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // Verify the artifact header
    await expect(canvas.getByText("Sub-Agent (gpt-4o)")).toBeInTheDocument();

    // Verify model name in sub-header
    await expect(canvas.getByText("gpt-4o")).toBeInTheDocument();

    // Verify usage stats are shown (cost 0.0164 is formatted as $0.0164)
    await expect(canvas.getByText(/801 tokens/)).toBeInTheDocument();
    await expect(canvas.getByText(/\$0\.0164/)).toBeInTheDocument();

    // Verify all three sections exist
    await expect(canvas.getByText("Task")).toBeInTheDocument();
    await expect(canvas.getByText("Internal")).toBeInTheDocument();
    await expect(canvas.getByText("Output")).toBeInTheDocument();

    // Output should always be visible
    await expect(canvas.getByText(/Error Correction Advances/)).toBeInTheDocument();

    // Click to expand task
    await userEvent.click(canvas.getByText("Task"));
    await expect(canvas.getByText(/Research the latest developments/)).toBeInTheDocument();

    // Click to expand internal reasoning
    await userEvent.click(canvas.getByText("Internal"));
    await expect(canvas.getByText(/Let me investigate/)).toBeInTheDocument();
  },
};

/** Sub-agent artifact without usage data */
export const AgentWithoutUsage: Story = {
  args: {
    artifact: agentArtifactWithoutUsage,
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // Verify the artifact renders without usage
    await expect(canvas.getByText("Sub-Agent (claude-3-sonnet)")).toBeInTheDocument();
    await expect(canvas.getByText("claude-3-sonnet")).toBeInTheDocument();

    // No usage stats should be present
    expect(canvas.queryByText(/tokens/)).not.toBeInTheDocument();

    // Verify structure: Task, Internal, Output sections
    await expect(canvas.getByText("Task")).toBeInTheDocument();
    await expect(canvas.getByText("Internal")).toBeInTheDocument();
    await expect(canvas.getByText("Output")).toBeInTheDocument();
  },
};

/** Sub-agent in modal view */
export const ModalWithAgent: StoryObj<typeof ArtifactModal> = {
  render: function ModalWithAgentRender() {
    const [open, setOpen] = useState(true);
    return (
      <>
        <button
          onClick={() => setOpen(true)}
          className="px-4 py-2 bg-primary text-primary-foreground rounded"
        >
          Open Agent Modal
        </button>
        <ArtifactModal artifact={agentArtifact} open={open} onClose={() => setOpen(false)} />
      </>
    );
  },
  play: async () => {
    const body = within(document.body);
    await expect(body.getByText("Sub-Agent (gpt-4o)")).toBeInTheDocument();
  },
};

/** Multiple artifacts in a list */
export const ArtifactListStory: StoryObj<typeof ArtifactList> = {
  render: () => (
    <ArtifactList artifacts={[codeArtifact, tableArtifact, imageArtifact]} className="space-y-4" />
  ),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // Verify all artifacts are rendered
    await expect(canvas.getByText("Python Output")).toBeInTheDocument();
    await expect(canvas.getByText("Sales Data Q4 2024")).toBeInTheDocument();
    await expect(canvas.getByText("Generated Chart")).toBeInTheDocument();
  },
};

/** Empty artifact list */
export const EmptyList: StoryObj<typeof ArtifactList> = {
  render: () => <ArtifactList artifacts={[]} />,
};

/** Unknown artifact type fallback */
export const UnknownType: Story = {
  args: {
    artifact: {
      id: "unknown-1",
      type: "unknown" as ArtifactType["type"],
      data: { foo: "bar" },
    },
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // Verify unknown type message
    await expect(canvas.getByText(/Unknown artifact type/)).toBeInTheDocument();
  },
};

/** Code artifact without language specified */
export const CodeWithoutLanguage: Story = {
  args: {
    artifact: {
      id: "code-2",
      type: "code",
      data: {
        code: "console.log('Hello, world!');",
      },
    },
  },
};

/** Large table with many rows */
export const LargeTable: Story = {
  args: {
    artifact: {
      id: "table-large",
      type: "table",
      title: "Large Dataset",
      data: {
        columns: [
          { key: "id", label: "ID" },
          { key: "name", label: "Name" },
          { key: "value", label: "Value" },
          { key: "status", label: "Status" },
        ],
        rows: Array.from({ length: 50 }, (_, i) => ({
          id: i + 1,
          name: `Item ${i + 1}`,
          value: Math.round(Math.random() * 10000) / 100,
          status: ["Active", "Pending", "Completed"][Math.floor(Math.random() * 3)],
        })),
      },
    },
  },
};

// ============ ArtifactModal Stories ============

/** Interactive ArtifactModal with code artifact */
export const ModalWithCode: StoryObj<typeof ArtifactModal> = {
  render: function ModalWithCodeRender() {
    const [open, setOpen] = useState(true);
    return (
      <>
        <button
          onClick={() => setOpen(true)}
          className="px-4 py-2 bg-primary text-primary-foreground rounded"
        >
          Open Code Modal
        </button>
        <ArtifactModal artifact={codeArtifact} open={open} onClose={() => setOpen(false)} />
      </>
    );
  },
  play: async () => {
    // Modal renders in a portal to document.body
    const body = within(document.body);

    // Modal should be open by default
    await expect(body.getByText("Python Output")).toBeInTheDocument();

    // Close button should exist
    await expect(body.getByRole("button", { name: /Close/ })).toBeInTheDocument();
  },
};

/** Interactive ArtifactModal with table artifact */
export const ModalWithTable: StoryObj<typeof ArtifactModal> = {
  render: function ModalWithTableRender() {
    const [open, setOpen] = useState(true);
    return (
      <>
        <button
          onClick={() => setOpen(true)}
          className="px-4 py-2 bg-primary text-primary-foreground rounded"
        >
          Open Table Modal
        </button>
        <ArtifactModal artifact={tableArtifact} open={open} onClose={() => setOpen(false)} />
      </>
    );
  },
  play: async () => {
    const body = within(document.body);
    await expect(body.getByText("Sales Data Q4 2024")).toBeInTheDocument();
  },
};

/** Interactive ArtifactModal with chart artifact */
export const ModalWithChart: StoryObj<typeof ArtifactModal> = {
  render: function ModalWithChartRender() {
    const [open, setOpen] = useState(true);
    return (
      <>
        <button
          onClick={() => setOpen(true)}
          className="px-4 py-2 bg-primary text-primary-foreground rounded"
        >
          Open Chart Modal
        </button>
        <ArtifactModal artifact={chartArtifact} open={open} onClose={() => setOpen(false)} />
      </>
    );
  },
  play: async () => {
    const body = within(document.body);
    await expect(body.getByText("Monthly Revenue")).toBeInTheDocument();
  },
};

/** Interactive ArtifactModal with image artifact */
export const ModalWithImage: StoryObj<typeof ArtifactModal> = {
  render: function ModalWithImageRender() {
    const [open, setOpen] = useState(true);
    return (
      <>
        <button
          onClick={() => setOpen(true)}
          className="px-4 py-2 bg-primary text-primary-foreground rounded"
        >
          Open Image Modal
        </button>
        <ArtifactModal artifact={imageArtifact} open={open} onClose={() => setOpen(false)} />
      </>
    );
  },
  play: async () => {
    const body = within(document.body);
    await expect(body.getByText("Generated Chart")).toBeInTheDocument();
  },
};

/** Interactive ArtifactModal with HTML artifact */
export const ModalWithHtml: StoryObj<typeof ArtifactModal> = {
  render: function ModalWithHtmlRender() {
    const [open, setOpen] = useState(true);
    return (
      <>
        <button
          onClick={() => setOpen(true)}
          className="px-4 py-2 bg-primary text-primary-foreground rounded"
        >
          Open HTML Modal
        </button>
        <ArtifactModal artifact={htmlArtifact} open={open} onClose={() => setOpen(false)} />
      </>
    );
  },
  play: async () => {
    const body = within(document.body);
    await expect(body.getByText("Interactive Widget")).toBeInTheDocument();
  },
};

/** ArtifactModal with input role badge */
export const ModalWithInputRole: StoryObj<typeof ArtifactModal> = {
  render: function ModalWithInputRoleRender() {
    const [open, setOpen] = useState(true);
    const inputArtifact: ArtifactType = {
      ...codeArtifact,
      id: "code-input",
      title: "Python Code",
      role: "input",
    };
    return (
      <>
        <button
          onClick={() => setOpen(true)}
          className="px-4 py-2 bg-primary text-primary-foreground rounded"
        >
          Open Input Modal
        </button>
        <ArtifactModal artifact={inputArtifact} open={open} onClose={() => setOpen(false)} />
      </>
    );
  },
  play: async () => {
    const body = within(document.body);
    await expect(body.getByText("Python Code")).toBeInTheDocument();
    await expect(body.getByText("Input")).toBeInTheDocument();
  },
};

/** ArtifactModal close interaction */
export const ModalCloseInteraction: StoryObj<typeof ArtifactModal> = {
  render: function ModalCloseInteractionRender() {
    const [open, setOpen] = useState(true);
    return (
      <>
        <button
          onClick={() => setOpen(true)}
          className="px-4 py-2 bg-primary text-primary-foreground rounded"
        >
          Open Modal
        </button>
        <ArtifactModal artifact={codeArtifact} open={open} onClose={() => setOpen(false)} />
      </>
    );
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const body = within(document.body);

    // Modal should be open initially (in portal)
    await expect(body.getByText("Python Output")).toBeInTheDocument();

    // Click close button (in portal)
    await userEvent.click(body.getByRole("button", { name: /Close/ }));

    // Wait a bit for the modal to close
    await new Promise((resolve) => setTimeout(resolve, 100));

    // Modal content should no longer be visible (modal closed)
    // Note: The open button should still exist (in canvas)
    await expect(canvas.getByText("Open Modal")).toBeInTheDocument();
  },
};
