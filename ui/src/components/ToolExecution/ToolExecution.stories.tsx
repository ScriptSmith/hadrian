import type { Meta, StoryObj } from "@storybook/react";
import { expect, within, userEvent } from "storybook/test";
import type { ToolExecution, ToolExecutionRound, Artifact } from "@/components/chat-types";
import { ToolExecutionBlock } from "./ToolExecutionBlock";
import { ExecutionTimeline } from "./ExecutionTimeline";
import { ExecutionSummaryBar } from "./ExecutionSummaryBar";
import { ToolExecutionStep } from "./ToolExecutionStep";
import { ArtifactThumbnail } from "./ArtifactThumbnail";
import { PreferencesProvider } from "@/preferences/PreferencesProvider";

// Sample artifacts for stories
const codeInputArtifact: Artifact = {
  id: "code-input-1",
  type: "code",
  title: "Python",
  role: "input",
  data: {
    language: "python",
    code: 'import pandas as pd\nimport matplotlib.pyplot as plt\n\ndf = pd.read_csv("data.csv")\nprint(df.head())\nprint(df.describe())',
  },
};

const codeOutputArtifact: Artifact = {
  id: "code-output-1",
  type: "code",
  title: "Output",
  role: "output",
  data: {
    language: "text",
    code: "   name  age  salary\n0  Alice   30   50000\n1    Bob   25   45000",
  },
};

const chartArtifact: Artifact = {
  id: "chart-1",
  type: "chart",
  title: "Sales Chart",
  role: "output",
  data: {
    spec: {
      $schema: "https://vega.github.io/schema/vega-lite/v6.json",
      data: { values: [{ x: 1, y: 2 }] },
      mark: "bar",
    },
  },
};

const tableArtifact: Artifact = {
  id: "table-1",
  type: "table",
  title: "Results",
  role: "output",
  data: {
    columns: [
      { key: "name", label: "Name" },
      { key: "value", label: "Value" },
    ],
    rows: [
      { name: "Total", value: 1234 },
      { name: "Average", value: 56.7 },
    ],
  },
};

// Sample executions
const successfulPythonExecution: ToolExecution = {
  id: "exec-1",
  toolName: "code_interpreter",
  status: "success",
  startTime: Date.now() - 2100,
  endTime: Date.now(),
  duration: 2100,
  input: { code: 'print("Hello")' },
  inputArtifacts: [codeInputArtifact],
  outputArtifacts: [codeOutputArtifact, chartArtifact],
  round: 1,
};

const failedPythonExecution: ToolExecution = {
  id: "exec-2",
  toolName: "code_interpreter",
  status: "error",
  startTime: Date.now() - 1200,
  endTime: Date.now(),
  duration: 1200,
  input: { code: "import seaborn" },
  inputArtifacts: [
    {
      ...codeInputArtifact,
      id: "code-input-2",
      data: { language: "python", code: "import seaborn as sns" },
    },
  ],
  outputArtifacts: [],
  error: "ModuleNotFoundError: No module named 'seaborn'",
  round: 1,
};

const sqlExecution: ToolExecution = {
  id: "exec-3",
  toolName: "sql_query",
  status: "success",
  startTime: Date.now() - 500,
  endTime: Date.now(),
  duration: 500,
  input: { sql: "SELECT * FROM users LIMIT 10" },
  inputArtifacts: [
    {
      id: "sql-input-1",
      type: "code",
      title: "SQL Query",
      role: "input",
      data: { language: "sql", code: "SELECT * FROM users LIMIT 10" },
    },
  ],
  outputArtifacts: [tableArtifact],
  round: 2,
};

const runningExecution: ToolExecution = {
  id: "exec-4",
  toolName: "code_interpreter",
  status: "running",
  startTime: Date.now(),
  input: { code: "# Processing..." },
  inputArtifacts: [],
  outputArtifacts: [],
  round: 1,
};

// Sample rounds
const singleSuccessRound: ToolExecutionRound[] = [
  {
    round: 1,
    executions: [successfulPythonExecution],
    totalDuration: 2100,
  },
];

const multiRoundWithRetry: ToolExecutionRound[] = [
  {
    round: 1,
    executions: [failedPythonExecution],
    hasError: true,
    totalDuration: 1200,
    modelReasoning: "I'll try using matplotlib instead of seaborn...",
  },
  {
    round: 2,
    executions: [successfulPythonExecution],
    totalDuration: 2100,
  },
];

const multiToolRound: ToolExecutionRound[] = [
  {
    round: 1,
    executions: [successfulPythonExecution, sqlExecution],
    totalDuration: 2600,
  },
];

const streamingRound: ToolExecutionRound[] = [
  {
    round: 1,
    executions: [runningExecution],
  },
];

// ============== ToolExecutionBlock Stories ==============

const meta: Meta<typeof ToolExecutionBlock> = {
  title: "Chat/ToolExecutionBlock",
  component: ToolExecutionBlock,
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
type Story = StoryObj<typeof ToolExecutionBlock>;

/** Default collapsed view with output artifacts visible */
export const Collapsed: Story = {
  args: {
    rounds: singleSuccessRound,
  },
};

/** Expanded view showing full timeline */
export const Expanded: Story = {
  args: {
    rounds: singleSuccessRound,
    defaultExpanded: true,
  },
};

/** Multiple rounds showing retry behavior */
export const WithRetries: Story = {
  args: {
    rounds: multiRoundWithRetry,
  },
};

/** Multiple rounds expanded showing model reasoning */
export const WithRetriesExpanded: Story = {
  args: {
    rounds: multiRoundWithRetry,
    defaultExpanded: true,
  },
};

/** Multiple tools executed in same round */
export const MultiTool: Story = {
  args: {
    rounds: multiToolRound,
  },
};

/** Active streaming/execution state */
export const Streaming: Story = {
  args: {
    rounds: streamingRound,
    isStreaming: true,
  },
};

/** Complex scenario: multi-round, multi-tool, with errors */
export const ComplexScenario: Story = {
  args: {
    rounds: [
      {
        round: 1,
        executions: [failedPythonExecution],
        hasError: true,
        totalDuration: 1200,
        modelReasoning: "Seaborn is not available, let me try matplotlib instead...",
      },
      {
        round: 2,
        executions: [successfulPythonExecution, sqlExecution],
        totalDuration: 2600,
        modelReasoning: "Now let me also query the database for related data...",
      },
      {
        round: 3,
        executions: [
          {
            id: "exec-5",
            toolName: "chart_render",
            status: "success",
            startTime: Date.now() - 300,
            endTime: Date.now(),
            duration: 300,
            input: { spec: {} },
            inputArtifacts: [],
            outputArtifacts: [chartArtifact],
            round: 3,
          },
        ],
        totalDuration: 300,
      },
    ],
    defaultExpanded: true,
  },
};

// ============== ExecutionTimeline Stories ==============

export const TimelineSingleTool: Story = {
  render: () => (
    <div className="max-w-2xl">
      <ExecutionTimeline rounds={singleSuccessRound} />
    </div>
  ),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    // Single round doesn't show "Round 1" header
    await expect(canvas.getByText("Python")).toBeInTheDocument();
  },
};

export const TimelineMultiTool: Story = {
  render: () => (
    <div className="max-w-2xl">
      <ExecutionTimeline rounds={multiToolRound} />
    </div>
  ),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByText("Python")).toBeInTheDocument();
    await expect(canvas.getByText("SQL Query")).toBeInTheDocument();
  },
};

export const TimelineWithRetries: Story = {
  render: () => (
    <div className="max-w-2xl">
      <ExecutionTimeline rounds={multiRoundWithRetry} />
    </div>
  ),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    // Multiple rounds show headers
    await expect(canvas.getByText("Round 1")).toBeInTheDocument();
    await expect(canvas.getByText("Round 2")).toBeInTheDocument();
    // Error badge should appear
    await expect(canvas.getByText("failed")).toBeInTheDocument();
    // Model reasoning should appear between rounds
    await expect(canvas.getByText(/matplotlib instead of seaborn/)).toBeInTheDocument();
  },
};

// ============== ExecutionSummaryBar Stories ==============

export const SummaryBarCollapsed: Story = {
  render: () => (
    <div className="max-w-2xl">
      <ExecutionSummaryBar rounds={singleSuccessRound} isExpanded={false} onToggle={() => {}} />
    </div>
  ),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByText(/1 tool/)).toBeInTheDocument();
    await expect(canvas.getByText(/2\.1s/)).toBeInTheDocument();
  },
};

export const SummaryBarExpanded: Story = {
  render: () => (
    <div className="max-w-2xl">
      <ExecutionSummaryBar rounds={singleSuccessRound} isExpanded={true} onToggle={() => {}} />
    </div>
  ),
};

export const SummaryBarWithRetries: Story = {
  render: () => (
    <div className="max-w-2xl">
      <ExecutionSummaryBar rounds={multiRoundWithRetry} isExpanded={false} onToggle={() => {}} />
    </div>
  ),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByText(/1 retry/)).toBeInTheDocument();
  },
};

export const SummaryBarStreaming: Story = {
  render: () => (
    <div className="max-w-2xl">
      <ExecutionSummaryBar
        rounds={streamingRound}
        isExpanded={true}
        isStreaming={true}
        onToggle={() => {}}
      />
    </div>
  ),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByText("running")).toBeInTheDocument();
  },
};

export const SummaryBarMultiTool: Story = {
  render: () => (
    <div className="max-w-2xl">
      <ExecutionSummaryBar rounds={multiToolRound} isExpanded={false} onToggle={() => {}} />
    </div>
  ),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByText(/2 tools/)).toBeInTheDocument();
    // Tool icons should be visible (icons only, no text labels)
  },
};

// ============== ToolExecutionStep Stories ==============

export const StepSuccess: Story = {
  render: () => (
    <div className="max-w-xl pl-4">
      <ToolExecutionStep execution={successfulPythonExecution} />
    </div>
  ),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByText("Python")).toBeInTheDocument();
    await expect(canvas.getByText("2.1s")).toBeInTheDocument();
    // Output artifacts should be visible
    await expect(canvas.getByText("Output")).toBeInTheDocument();
  },
};

export const StepError: Story = {
  render: () => (
    <div className="max-w-xl pl-4">
      <ToolExecutionStep execution={failedPythonExecution} />
    </div>
  ),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByText("Python")).toBeInTheDocument();
    // Error message should be visible
    await expect(canvas.getByText(/ModuleNotFoundError/)).toBeInTheDocument();
  },
};

export const StepRunning: Story = {
  render: () => (
    <div className="max-w-xl pl-4">
      <ToolExecutionStep execution={runningExecution} />
    </div>
  ),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByText("running")).toBeInTheDocument();
  },
};

export const StepPending: Story = {
  render: () => (
    <div className="max-w-xl pl-4">
      <ToolExecutionStep
        execution={{
          ...runningExecution,
          id: "exec-pending",
          status: "pending",
        }}
      />
    </div>
  ),
};

export const StepSQL: Story = {
  render: () => (
    <div className="max-w-xl pl-4">
      <ToolExecutionStep execution={sqlExecution} />
    </div>
  ),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByText("SQL Query")).toBeInTheDocument();
    await expect(canvas.getByText("Results")).toBeInTheDocument();
  },
};

export const StepWithInputExpanded: Story = {
  render: () => (
    <div className="max-w-xl pl-4">
      <ToolExecutionStep execution={successfulPythonExecution} defaultInputExpanded={true} />
    </div>
  ),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    // Code should be visible inline
    await expect(canvas.getByText(/import pandas/)).toBeInTheDocument();
  },
};

export const StepExpandInput: Story = {
  render: () => (
    <div className="max-w-xl pl-4">
      <ToolExecutionStep execution={successfulPythonExecution} />
    </div>
  ),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    // Code preview should be visible inline
    await expect(canvas.getByText(/import pandas/)).toBeInTheDocument();
    // Click to expand full code
    const expandButton = canvas.getByText("expand");
    await userEvent.click(expandButton);
    // Should now show "collapse"
    await expect(canvas.getByText("collapse")).toBeInTheDocument();
  },
};

// ============== ArtifactThumbnail Stories ==============

export const ThumbnailCode: Story = {
  render: () => <ArtifactThumbnail artifact={codeOutputArtifact} />,
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByText("Output")).toBeInTheDocument();
  },
};

export const ThumbnailTable: Story = {
  render: () => <ArtifactThumbnail artifact={tableArtifact} />,
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByText("Results")).toBeInTheDocument();
  },
};

export const ThumbnailChart: Story = {
  render: () => <ArtifactThumbnail artifact={chartArtifact} />,
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByText("Sales Chart")).toBeInTheDocument();
  },
};

export const ThumbnailImage: Story = {
  render: () => (
    <ArtifactThumbnail
      artifact={{
        id: "image-1",
        type: "image",
        title: "Generated Plot",
        role: "output",
        data: "data:image/png;base64,iVBORw0KGgo=",
      }}
    />
  ),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByText("Generated Plot")).toBeInTheDocument();
  },
};

export const ThumbnailHtml: Story = {
  render: () => (
    <ArtifactThumbnail
      artifact={{
        id: "html-1",
        type: "html",
        title: "Interactive Widget",
        role: "output",
        data: "<div>Hello</div>",
      }}
    />
  ),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByText("Interactive Widget")).toBeInTheDocument();
  },
};

export const ThumbnailWithOrigin: Story = {
  render: () => <ArtifactThumbnail artifact={chartArtifact} originLabel="from Python" />,
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    // Origin label is passed but displayed is simplified to just title
    await expect(canvas.getByText("Sales Chart")).toBeInTheDocument();
  },
};

export const ThumbnailNoTitle: Story = {
  render: () => (
    <ArtifactThumbnail
      artifact={{
        id: "code-no-title",
        type: "code",
        role: "output",
        data: { language: "python", code: "print('hello')" },
      }}
    />
  ),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    // Should fall back to type label
    await expect(canvas.getByText("Code")).toBeInTheDocument();
  },
};

// ============== Tool-Specific Stories ==============
// These stories show realistic tool executions at various stages

// --- Python Tool Stories ---

const pythonDataAnalysis: ToolExecution = {
  id: "python-data-1",
  toolName: "code_interpreter",
  status: "success",
  startTime: Date.now() - 3500,
  endTime: Date.now(),
  duration: 3500,
  input: {},
  inputArtifacts: [
    {
      id: "python-input-1",
      type: "code",
      title: "Python",
      role: "input",
      data: {
        language: "python",
        code: `import pandas as pd
import matplotlib.pyplot as plt

# Load and analyze sales data
df = pd.read_csv('sales_2024.csv')
print(f"Total records: {len(df)}")
print(f"Total revenue: \${df['amount'].sum():,.2f}")

# Group by month
monthly = df.groupby('month')['amount'].sum()
print("\\nMonthly breakdown:")
print(monthly)`,
      },
    },
  ],
  outputArtifacts: [
    {
      id: "python-output-1",
      type: "code",
      title: "stdout",
      role: "output",
      data: {
        language: "text",
        code: `Total records: 15234
Total revenue: $2,847,293.45

Monthly breakdown:
month
Jan    234521.00
Feb    198234.00
Mar    312456.00
...`,
      },
    },
  ],
  round: 1,
};

const pythonWithChart: ToolExecution = {
  id: "python-chart-1",
  toolName: "code_interpreter",
  status: "success",
  startTime: Date.now() - 4200,
  endTime: Date.now(),
  duration: 4200,
  input: {},
  inputArtifacts: [
    {
      id: "python-chart-input",
      type: "code",
      title: "Python",
      role: "input",
      data: {
        language: "python",
        code: `import matplotlib.pyplot as plt
import numpy as np

# Generate sample data
months = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun']
revenue = [234, 198, 312, 287, 342, 398]

# Create bar chart
plt.figure(figsize=(10, 6))
plt.bar(months, revenue, color='#3b82f6')
plt.title('Monthly Revenue 2024')
plt.ylabel('Revenue ($K)')
plt.xlabel('Month')
plt.savefig('revenue_chart.png')
print("Chart saved successfully")`,
      },
    },
  ],
  outputArtifacts: [
    {
      id: "python-chart-stdout",
      type: "code",
      title: "stdout",
      role: "output",
      data: { language: "text", code: "Chart saved successfully" },
    },
    {
      id: "python-chart-image",
      type: "image",
      title: "revenue_chart.png",
      role: "output",
      data: "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==",
    },
  ],
  round: 1,
};

const pythonRunning: ToolExecution = {
  id: "python-running-1",
  toolName: "code_interpreter",
  status: "running",
  startTime: Date.now() - 1500,
  input: {},
  inputArtifacts: [
    {
      id: "python-running-input",
      type: "code",
      title: "Python",
      role: "input",
      data: {
        language: "python",
        code: `import pandas as pd
from sklearn.model_selection import train_test_split
from sklearn.ensemble import RandomForestClassifier

# Training a model...
df = pd.read_csv('training_data.csv')
X = df.drop('target', axis=1)
y = df['target']

model = RandomForestClassifier(n_estimators=100)
model.fit(X, y)`,
      },
    },
  ],
  outputArtifacts: [],
  round: 1,
};

const pythonError: ToolExecution = {
  id: "python-error-1",
  toolName: "code_interpreter",
  status: "error",
  startTime: Date.now() - 800,
  endTime: Date.now(),
  duration: 800,
  input: {},
  inputArtifacts: [
    {
      id: "python-error-input",
      type: "code",
      title: "Python",
      role: "input",
      data: {
        language: "python",
        code: `import tensorflow as tf
model = tf.keras.models.load_model('my_model.h5')`,
      },
    },
  ],
  outputArtifacts: [],
  error: "ModuleNotFoundError: No module named 'tensorflow'",
  round: 1,
};

/** Python: Data analysis with stdout output */
export const PythonDataAnalysis: Story = {
  args: {
    rounds: [{ round: 1, executions: [pythonDataAnalysis], totalDuration: 3500 }],
  },
};

/** Python: Running state with code visible */
export const PythonRunning: Story = {
  args: {
    rounds: [{ round: 1, executions: [pythonRunning] }],
    isStreaming: true,
  },
};

/** Python: Error with traceback */
export const PythonError: Story = {
  args: {
    rounds: [{ round: 1, executions: [pythonError], hasError: true, totalDuration: 800 }],
  },
};

/** Python: Chart generation with image output */
export const PythonWithChart: Story = {
  args: {
    rounds: [{ round: 1, executions: [pythonWithChart], totalDuration: 4200 }],
  },
};

// --- SQL Tool Stories ---

const sqlSimpleQuery: ToolExecution = {
  id: "sql-simple-1",
  toolName: "sql_query",
  status: "success",
  startTime: Date.now() - 150,
  endTime: Date.now(),
  duration: 150,
  input: {},
  inputArtifacts: [
    {
      id: "sql-simple-input",
      type: "code",
      title: "SQL",
      role: "input",
      data: {
        language: "sql",
        code: `SELECT
  customer_name,
  SUM(order_total) as total_spent,
  COUNT(*) as order_count
FROM orders
GROUP BY customer_name
ORDER BY total_spent DESC
LIMIT 10`,
      },
    },
  ],
  outputArtifacts: [
    {
      id: "sql-simple-output",
      type: "table",
      title: "Query Results",
      role: "output",
      data: {
        columns: [
          { key: "customer_name", label: "Customer" },
          { key: "total_spent", label: "Total Spent" },
          { key: "order_count", label: "Orders" },
        ],
        rows: [
          { customer_name: "Acme Corp", total_spent: 125430.0, order_count: 47 },
          { customer_name: "TechStart Inc", total_spent: 98234.5, order_count: 32 },
          { customer_name: "Global Systems", total_spent: 87123.0, order_count: 28 },
          { customer_name: "DataFlow LLC", total_spent: 76543.25, order_count: 21 },
          { customer_name: "CloudNine", total_spent: 65432.0, order_count: 19 },
        ],
      },
    },
  ],
  round: 1,
};

const sqlComplexJoin: ToolExecution = {
  id: "sql-complex-1",
  toolName: "sql_query",
  status: "success",
  startTime: Date.now() - 420,
  endTime: Date.now(),
  duration: 420,
  input: {},
  inputArtifacts: [
    {
      id: "sql-complex-input",
      type: "code",
      title: "SQL",
      role: "input",
      data: {
        language: "sql",
        code: `WITH monthly_revenue AS (
  SELECT
    DATE_TRUNC('month', order_date) as month,
    SUM(amount) as revenue
  FROM orders o
  JOIN products p ON o.product_id = p.id
  WHERE order_date >= '2024-01-01'
  GROUP BY 1
),
growth AS (
  SELECT
    month,
    revenue,
    LAG(revenue) OVER (ORDER BY month) as prev_revenue,
    (revenue - LAG(revenue) OVER (ORDER BY month)) /
      NULLIF(LAG(revenue) OVER (ORDER BY month), 0) * 100 as growth_pct
  FROM monthly_revenue
)
SELECT * FROM growth ORDER BY month`,
      },
    },
  ],
  outputArtifacts: [
    {
      id: "sql-complex-output",
      type: "table",
      title: "Monthly Growth",
      role: "output",
      data: {
        columns: [
          { key: "month", label: "Month" },
          { key: "revenue", label: "Revenue" },
          { key: "growth_pct", label: "Growth %" },
        ],
        rows: [
          { month: "2024-01", revenue: 234521, growth_pct: null },
          { month: "2024-02", revenue: 198234, growth_pct: -15.5 },
          { month: "2024-03", revenue: 312456, growth_pct: 57.6 },
          { month: "2024-04", revenue: 287123, growth_pct: -8.1 },
        ],
      },
    },
  ],
  round: 1,
};

const sqlError: ToolExecution = {
  id: "sql-error-1",
  toolName: "sql_query",
  status: "error",
  startTime: Date.now() - 50,
  endTime: Date.now(),
  duration: 50,
  input: {},
  inputArtifacts: [
    {
      id: "sql-error-input",
      type: "code",
      title: "SQL",
      role: "input",
      data: {
        language: "sql",
        code: `SELECT * FROM nonexistent_table WHERE id = 1`,
      },
    },
  ],
  outputArtifacts: [],
  error: "Table 'nonexistent_table' does not exist",
  round: 1,
};

/** SQL: Simple aggregation query */
export const SQLSimpleQuery: Story = {
  args: {
    rounds: [{ round: 1, executions: [sqlSimpleQuery], totalDuration: 150 }],
  },
};

/** SQL: Complex query with CTEs */
export const SQLComplexQuery: Story = {
  args: {
    rounds: [{ round: 1, executions: [sqlComplexJoin], totalDuration: 420 }],
  },
};

/** SQL: Query error */
export const SQLError: Story = {
  args: {
    rounds: [{ round: 1, executions: [sqlError], hasError: true, totalDuration: 50 }],
  },
};

// --- JavaScript Tool Stories ---

const jsCalculation: ToolExecution = {
  id: "js-calc-1",
  toolName: "js_code_interpreter",
  status: "success",
  startTime: Date.now() - 45,
  endTime: Date.now(),
  duration: 45,
  input: {},
  inputArtifacts: [
    {
      id: "js-calc-input",
      type: "code",
      title: "JavaScript",
      role: "input",
      data: {
        language: "javascript",
        code: `// Calculate compound interest
const principal = 10000;
const rate = 0.05;
const years = 10;
const n = 12; // monthly compounding

const amount = principal * Math.pow(1 + rate/n, n * years);
console.log(\`Initial: $\${principal.toLocaleString()}\`);
console.log(\`Final: $\${amount.toLocaleString('en-US', {maximumFractionDigits: 2})}\`);
console.log(\`Interest earned: $\${(amount - principal).toLocaleString('en-US', {maximumFractionDigits: 2})}\`);`,
      },
    },
  ],
  outputArtifacts: [
    {
      id: "js-calc-output",
      type: "code",
      title: "console",
      role: "output",
      data: {
        language: "text",
        code: `Initial: $10,000
Final: $16,470.09
Interest earned: $6,470.09`,
      },
    },
  ],
  round: 1,
};

const jsDataProcessing: ToolExecution = {
  id: "js-data-1",
  toolName: "js_code_interpreter",
  status: "success",
  startTime: Date.now() - 120,
  endTime: Date.now(),
  duration: 120,
  input: {},
  inputArtifacts: [
    {
      id: "js-data-input",
      type: "code",
      title: "JavaScript",
      role: "input",
      data: {
        language: "javascript",
        code: `const data = [
  { name: 'Alice', score: 85 },
  { name: 'Bob', score: 92 },
  { name: 'Charlie', score: 78 },
  { name: 'Diana', score: 95 },
  { name: 'Eve', score: 88 }
];

const average = data.reduce((sum, d) => sum + d.score, 0) / data.length;
const sorted = [...data].sort((a, b) => b.score - a.score);

console.log('Rankings:');
sorted.forEach((d, i) => console.log(\`\${i + 1}. \${d.name}: \${d.score}\`));
console.log(\`\\nClass average: \${average.toFixed(1)}\`);`,
      },
    },
  ],
  outputArtifacts: [
    {
      id: "js-data-output",
      type: "code",
      title: "console",
      role: "output",
      data: {
        language: "text",
        code: `Rankings:
1. Diana: 95
2. Bob: 92
3. Eve: 88
4. Alice: 85
5. Charlie: 78

Class average: 87.6`,
      },
    },
  ],
  round: 1,
};

/** JavaScript: Simple calculation */
export const JavaScriptCalculation: Story = {
  args: {
    rounds: [{ round: 1, executions: [jsCalculation], totalDuration: 45 }],
  },
};

/** JavaScript: Data processing */
export const JavaScriptDataProcessing: Story = {
  args: {
    rounds: [{ round: 1, executions: [jsDataProcessing], totalDuration: 120 }],
  },
};

// --- Chart Tool Stories ---

const chartBarSimple: ToolExecution = {
  id: "chart-bar-1",
  toolName: "chart_render",
  status: "success",
  startTime: Date.now() - 80,
  endTime: Date.now(),
  duration: 80,
  input: {},
  inputArtifacts: [
    {
      id: "chart-bar-input",
      type: "code",
      title: "Vega-Lite Spec",
      role: "input",
      data: {
        language: "json",
        code: `{
  "$schema": "https://vega.github.io/schema/vega-lite/v6.json",
  "data": {
    "values": [
      {"category": "A", "value": 28},
      {"category": "B", "value": 55},
      {"category": "C", "value": 43}
    ]
  },
  "mark": "bar",
  "encoding": {
    "x": {"field": "category", "type": "nominal"},
    "y": {"field": "value", "type": "quantitative"}
  }
}`,
      },
    },
  ],
  outputArtifacts: [
    {
      id: "chart-bar-output",
      type: "chart",
      title: "Bar Chart",
      role: "output",
      data: {
        spec: {
          $schema: "https://vega.github.io/schema/vega-lite/v6.json",
          data: {
            values: [
              { category: "A", value: 28 },
              { category: "B", value: 55 },
              { category: "C", value: 43 },
            ],
          },
          mark: "bar",
          encoding: {
            x: { field: "category", type: "nominal" },
            y: { field: "value", type: "quantitative" },
          },
        },
      },
    },
  ],
  round: 1,
};

const chartLineTimeSeries: ToolExecution = {
  id: "chart-line-1",
  toolName: "chart_render",
  status: "success",
  startTime: Date.now() - 95,
  endTime: Date.now(),
  duration: 95,
  input: {},
  inputArtifacts: [
    {
      id: "chart-line-input",
      type: "code",
      title: "Vega-Lite Spec",
      role: "input",
      data: {
        language: "json",
        code: `{
  "$schema": "https://vega.github.io/schema/vega-lite/v6.json",
  "title": "Stock Price Over Time",
  "data": { "values": [...] },
  "mark": "line",
  "encoding": {
    "x": {"field": "date", "type": "temporal"},
    "y": {"field": "price", "type": "quantitative"}
  }
}`,
      },
    },
  ],
  outputArtifacts: [
    {
      id: "chart-line-output",
      type: "chart",
      title: "Stock Price",
      role: "output",
      data: {
        spec: {
          $schema: "https://vega.github.io/schema/vega-lite/v6.json",
          title: "Stock Price Over Time",
          data: {
            values: [
              { date: "2024-01-01", price: 150 },
              { date: "2024-02-01", price: 165 },
              { date: "2024-03-01", price: 158 },
              { date: "2024-04-01", price: 172 },
              { date: "2024-05-01", price: 180 },
            ],
          },
          mark: "line",
          encoding: {
            x: { field: "date", type: "temporal" },
            y: { field: "price", type: "quantitative" },
          },
        },
      },
    },
  ],
  round: 1,
};

/** Chart: Simple bar chart */
export const ChartBarSimple: Story = {
  args: {
    rounds: [{ round: 1, executions: [chartBarSimple], totalDuration: 80 }],
  },
};

/** Chart: Time series line chart */
export const ChartTimeSeries: Story = {
  args: {
    rounds: [{ round: 1, executions: [chartLineTimeSeries], totalDuration: 95 }],
  },
};

// --- Multi-Tool Workflow Stories ---

/** Complete workflow: Python analysis → SQL query → Chart */
export const MultiToolWorkflow: Story = {
  args: {
    rounds: [
      {
        round: 1,
        executions: [pythonDataAnalysis],
        totalDuration: 3500,
        modelReasoning: "Let me also query the database for more detailed breakdown...",
      },
      {
        round: 2,
        executions: [sqlSimpleQuery],
        totalDuration: 150,
        modelReasoning: "Now I'll visualize this data...",
      },
      {
        round: 3,
        executions: [chartBarSimple],
        totalDuration: 80,
      },
    ],
    defaultExpanded: true,
  },
};

/** Error recovery: Failed import → successful alternative */
export const ErrorRecoveryWorkflow: Story = {
  args: {
    rounds: [
      {
        round: 1,
        executions: [pythonError],
        hasError: true,
        totalDuration: 800,
        modelReasoning:
          "TensorFlow is not available. Let me use scikit-learn instead for this classification task...",
      },
      {
        round: 2,
        executions: [pythonDataAnalysis],
        totalDuration: 3500,
      },
    ],
    defaultExpanded: true,
  },
};

// --- Sub-Agent Tool Stories ---

const subAgentRunning: ToolExecution = {
  id: "sub-agent-running-1",
  toolName: "sub_agent",
  status: "running",
  startTime: Date.now() - 3000,
  input: {
    task: "Research the latest developments in quantum computing and summarize the key breakthroughs from 2024.",
    model: "openai/gpt-4o",
  },
  inputArtifacts: [],
  outputArtifacts: [],
  round: 1,
};

const subAgentCompleted: ToolExecution = {
  id: "sub-agent-completed-1",
  toolName: "sub_agent",
  status: "success",
  startTime: Date.now() - 8500,
  endTime: Date.now(),
  duration: 8500,
  input: {
    task: "Analyze the pros and cons of different state management solutions in React applications.",
    model: "anthropic/claude-3-5-sonnet",
  },
  inputArtifacts: [],
  outputArtifacts: [
    {
      id: "agent-artifact-1",
      type: "agent",
      title: "Sub-Agent (claude-3-5-sonnet)",
      role: "output",
      data: {
        task: "Analyze the pros and cons of different state management solutions in React applications.",
        model: "anthropic/claude-3-5-sonnet",
        internal: `Let me analyze the major state management options for React...

**Redux:**
- Pros: Predictable state, great DevTools, huge ecosystem
- Cons: Boilerplate heavy, learning curve, overkill for small apps
- Best for: Large applications with complex state logic

**Zustand:**
- Pros: Minimal boilerplate, simple API, no providers needed
- Cons: Less opinionated, smaller ecosystem
- Best for: Medium apps that want simplicity

**React Context + useReducer:**
- Pros: Built-in, no dependencies, familiar patterns
- Cons: Can cause unnecessary re-renders, not optimized for frequent updates
- Best for: Small apps or passing data through many levels

**Jotai/Recoil (Atomic):**
- Pros: Fine-grained updates, minimal re-renders, composable
- Cons: Different mental model, newer ecosystem
- Best for: Apps with many independent pieces of state

**TanStack Query (Server State):**
- Pros: Handles caching, loading states, background updates
- Cons: Only for server state, adds complexity
- Best for: Apps with significant API data fetching

I'll now curate these findings into a concise summary...`,
        output: `## React State Management Comparison

| Solution | Best For | Trade-offs |
|----------|----------|------------|
| **Redux** | Large, complex apps | More boilerplate, steeper learning curve |
| **Zustand** | Medium apps | Simple API, less ecosystem support |
| **Context + useReducer** | Small apps | Built-in but can cause re-render issues |
| **Jotai/Recoil** | Fine-grained updates | Atomic model requires adjustment |
| **TanStack Query** | Server state | Specialized for API data only |

**Recommendation:** Start with Zustand for most apps. Add TanStack Query for server state. Only reach for Redux when you need its ecosystem.`,
        usage: {
          inputTokens: 156,
          outputTokens: 423,
          totalTokens: 579,
          cost: 0.0089,
        },
      },
    },
  ],
  round: 1,
};

const subAgentError: ToolExecution = {
  id: "sub-agent-error-1",
  toolName: "sub_agent",
  status: "error",
  startTime: Date.now() - 2000,
  endTime: Date.now(),
  duration: 2000,
  input: {
    task: "Investigate a very complex topic",
    model: "nonexistent/model",
  },
  inputArtifacts: [],
  outputArtifacts: [],
  error: "Model 'nonexistent/model' not found or not available",
  round: 1,
};

const subAgentWithCodeInterpreter: ToolExecution = {
  id: "sub-agent-research-1",
  toolName: "sub_agent",
  status: "success",
  startTime: Date.now() - 12000,
  endTime: Date.now(),
  duration: 12000,
  input: {
    task: "Research and explain the CAP theorem in distributed systems with practical examples.",
  },
  inputArtifacts: [],
  outputArtifacts: [
    {
      id: "agent-artifact-2",
      type: "agent",
      title: "Sub-Agent (gpt-4o)",
      role: "output",
      data: {
        task: "Research and explain the CAP theorem in distributed systems with practical examples.",
        model: "openai/gpt-4o",
        internal: `The CAP theorem, proposed by Eric Brewer in 2000, states that a distributed system can only provide two of three guarantees simultaneously:

1. **Consistency (C)**: Every read receives the most recent write or an error
2. **Availability (A)**: Every request receives a response (without guarantee it's the most recent)
3. **Partition Tolerance (P)**: System continues despite network partitions

Since network partitions are unavoidable in distributed systems, the real choice is between CP and AP systems.

**CP Systems (Consistency + Partition Tolerance):**
- MongoDB (with appropriate settings)
- HBase
- Redis Cluster
- Trade-off: May refuse requests during partitions

**AP Systems (Availability + Partition Tolerance):**
- Cassandra
- CouchDB
- DynamoDB
- Trade-off: May return stale data

**Real-world examples:**
- Banking: CP preferred (consistency critical)
- Social media: AP preferred (availability more important)
- E-commerce: Often hybrid approaches

Let me synthesize this into a clear explanation...`,
        output: `## CAP Theorem Explained

The CAP theorem states distributed systems must choose between:
- **CP** (Consistency + Partition Tolerance): Always correct data, may be unavailable
- **AP** (Availability + Partition Tolerance): Always responds, may have stale data

### Practical Examples

| Use Case | Choice | Why |
|----------|--------|-----|
| Banking transactions | CP | Cannot show wrong balance |
| Social media feeds | AP | Better to show old posts than nothing |
| Shopping cart | AP | Availability drives sales |
| Inventory counts | CP | Overselling is costly |

Most modern systems use hybrid approaches, choosing CP or AP per operation.`,
        usage: {
          inputTokens: 89,
          outputTokens: 312,
          totalTokens: 401,
          cost: 0.0062,
        },
      },
    },
  ],
  round: 1,
};

/** Sub-agent: Running state */
export const SubAgentRunning: Story = {
  args: {
    rounds: [{ round: 1, executions: [subAgentRunning] }],
    isStreaming: true,
  },
};

/** Sub-agent: Completed with agent artifact */
export const SubAgentCompleted: Story = {
  args: {
    rounds: [{ round: 1, executions: [subAgentCompleted], totalDuration: 8500 }],
  },
};

/** Sub-agent: Error state */
export const SubAgentError: Story = {
  args: {
    rounds: [{ round: 1, executions: [subAgentError], hasError: true, totalDuration: 2000 }],
  },
};

/** Sub-agent: Research task with detailed output */
export const SubAgentResearch: Story = {
  args: {
    rounds: [{ round: 1, executions: [subAgentWithCodeInterpreter], totalDuration: 12000 }],
    defaultExpanded: true,
  },
};

/** Multi-tool workflow including sub-agent for research */
export const WorkflowWithSubAgent: Story = {
  args: {
    rounds: [
      {
        round: 1,
        executions: [subAgentWithCodeInterpreter],
        totalDuration: 12000,
        modelReasoning: "Now that I understand the CAP theorem, let me create a visualization...",
      },
      {
        round: 2,
        executions: [chartBarSimple],
        totalDuration: 80,
      },
    ],
    defaultExpanded: true,
  },
};
