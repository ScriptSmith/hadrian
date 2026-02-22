import type { Meta, StoryObj } from "@storybook/react";
import { useState, useEffect } from "react";
import { http, HttpResponse } from "msw";

import type { VectorStoreOwnerType } from "@/api/generated/types.gen";
import { ToolsBar } from "./ToolsBar";
import { useMCPStore } from "@/stores/mcpStore";
import type { MCPConnectionStatus } from "@/services/mcp";

const meta = {
  title: "Components/ToolsBar",
  component: ToolsBar,
  parameters: {
    layout: "centered",
  },
  decorators: [
    (Story) => (
      <div className="p-8 bg-background border rounded-lg">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof ToolsBar>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Interactive wrapper that manages tool state */
function InteractiveToolsBar({
  initialTools = [],
  vectorStoreIds: initialVectorStoreIds,
  onVectorStoreIdsChange,
  vectorStoreOwnerType,
  vectorStoreOwnerId,
  disabled,
}: {
  initialTools?: string[];
  vectorStoreIds?: string[];
  onVectorStoreIdsChange?: (ids: string[]) => void;
  vectorStoreOwnerType?: VectorStoreOwnerType;
  vectorStoreOwnerId?: string;
  disabled?: boolean;
}) {
  const [enabledTools, setEnabledTools] = useState<string[]>(initialTools);
  const [vectorStoreIds, setVectorStoreIds] = useState<string[]>(initialVectorStoreIds || []);

  return (
    <div className="space-y-4">
      <ToolsBar
        enabledTools={enabledTools}
        onEnabledToolsChange={setEnabledTools}
        vectorStoreIds={vectorStoreIds}
        onVectorStoreIdsChange={onVectorStoreIdsChange || setVectorStoreIds}
        vectorStoreOwnerType={vectorStoreOwnerType}
        vectorStoreOwnerId={vectorStoreOwnerId}
        disabled={disabled}
      />
      <div className="text-xs text-muted-foreground space-y-1">
        <div>Enabled tools: {enabledTools.length > 0 ? enabledTools.join(", ") : "none"}</div>
        {vectorStoreIds.length > 0 && <div>Vector stores: {vectorStoreIds.join(", ")}</div>}
      </div>
    </div>
  );
}

export const Default: Story = {
  render: () => <InteractiveToolsBar />,
};

export const WithEnabledTools: Story = {
  render: () => (
    <InteractiveToolsBar initialTools={["code_interpreter", "sql_query", "chart_render"]} />
  ),
};

export const WithVectorStores: Story = {
  render: () => (
    <InteractiveToolsBar
      initialTools={["file_search"]}
      vectorStoreIds={["vs_abc123", "vs_def456"]}
    />
  ),
};

export const AllToolsEnabled: Story = {
  render: () => (
    <InteractiveToolsBar
      initialTools={[
        "file_search",
        "code_interpreter",
        "js_code_interpreter",
        "sql_query",
        "chart_render",
        "html_render",
      ]}
      vectorStoreIds={["vs_abc123"]}
    />
  ),
};

export const Disabled: Story = {
  render: () => <InteractiveToolsBar initialTools={["code_interpreter"]} disabled />,
};

export const InContext: Story = {
  render: () => (
    <div className="flex items-center gap-2 p-2 border rounded-xl bg-card">
      <InteractiveToolsBar initialTools={["sql_query"]} />
      <span className="text-muted-foreground">|</span>
      <button className="px-3 py-1.5 text-sm bg-primary text-primary-foreground rounded-lg">
        Send
      </button>
    </div>
  ),
};

/** Mock vector stores for stories */
const mockVectorStores = [
  {
    id: "vs_001",
    name: "Product Documentation",
    description: "Technical docs and guides",
    status: "completed",
    embedding_model: "text-embedding-3-small",
    file_counts: { total: 15, completed: 15, in_progress: 0, failed: 0, cancelled: 0 },
    created_at: Date.now() / 1000,
  },
  {
    id: "vs_002",
    name: "Customer Support KB",
    description: "FAQ and support articles",
    status: "completed",
    embedding_model: "text-embedding-3-small",
    file_counts: { total: 42, completed: 42, in_progress: 0, failed: 0, cancelled: 0 },
    created_at: Date.now() / 1000,
  },
  {
    id: "vs_003",
    name: "Research Papers",
    description: "Academic papers and citations",
    status: "in_progress",
    embedding_model: "text-embedding-ada-002",
    file_counts: { total: 8, completed: 5, in_progress: 3, failed: 0, cancelled: 0 },
    created_at: Date.now() / 1000,
  },
];

export const WithVectorStoreSelector: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("/api/admin/vector_stores", () => {
          return HttpResponse.json({
            data: mockVectorStores,
            has_more: false,
          });
        }),
      ],
    },
  },
  render: () => (
    <div className="space-y-4">
      <p className="text-sm text-muted-foreground max-w-md">
        Hover over the file_search tool icon to see the vector store selector. Click &quot;Add
        Knowledge&quot; to open the picker dialog.
      </p>
      <InteractiveToolsBar
        initialTools={["file_search"]}
        vectorStoreIds={["vs_001"]}
        vectorStoreOwnerType="user"
        vectorStoreOwnerId="user_123"
      />
    </div>
  ),
};

export const FileSearchWithNoStores: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("/api/admin/vector_stores", () => {
          return HttpResponse.json({
            data: mockVectorStores,
            has_more: false,
          });
        }),
      ],
    },
  },
  render: () => (
    <div className="space-y-4">
      <p className="text-sm text-muted-foreground max-w-md">
        file_search is enabled but no vector stores are selected yet. Hover to see the selector.
      </p>
      <InteractiveToolsBar
        initialTools={["file_search"]}
        vectorStoreIds={[]}
        vectorStoreOwnerType="user"
        vectorStoreOwnerId="user_123"
      />
    </div>
  ),
};

export const SqlQueryWithDataFiles: Story = {
  render: () => (
    <div className="space-y-4">
      <p className="text-sm text-muted-foreground max-w-md">
        Hover over the sql_query tool icon to see the data file upload panel. Upload CSV, Parquet,
        JSON, or SQLite files to query with SQL.
      </p>
      <InteractiveToolsBar initialTools={["sql_query"]} />
    </div>
  ),
};

export const MultipleToolsWithSettings: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("/api/admin/vector_stores", () => {
          return HttpResponse.json({
            data: mockVectorStores,
            has_more: false,
          });
        }),
      ],
    },
  },
  render: () => (
    <div className="space-y-4">
      <p className="text-sm text-muted-foreground max-w-md">
        Multiple tools enabled with their respective settings panels. Hover over each tool to see
        its configuration options.
      </p>
      <InteractiveToolsBar
        initialTools={["file_search", "sql_query", "code_interpreter"]}
        vectorStoreIds={["vs_001", "vs_002"]}
        vectorStoreOwnerType="user"
        vectorStoreOwnerId="user_123"
      />
    </div>
  ),
};

export const DisabledToolStates: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("/api/admin/vector_stores", () => {
          return HttpResponse.json({
            data: mockVectorStores,
            has_more: false,
          });
        }),
      ],
    },
  },
  render: () => (
    <div className="space-y-4">
      <p className="text-sm text-muted-foreground max-w-md">
        Hover over disabled tools to see why they can&apos;t be enabled. Web Search shows &quot;not
        yet available&quot;, File Search shows config requirement when no vector stores are
        attached.
      </p>
      <InteractiveToolsBar
        initialTools={["code_interpreter"]}
        vectorStoreIds={[]}
        vectorStoreOwnerType="user"
        vectorStoreOwnerId="user_123"
      />
    </div>
  ),
};

export const StableOrderingDemo: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("/api/admin/vector_stores", () => {
          return HttpResponse.json({
            data: mockVectorStores,
            has_more: false,
          });
        }),
      ],
    },
  },
  render: () => (
    <div className="space-y-4">
      <p className="text-sm text-muted-foreground max-w-md">
        <strong>Stable ordering:</strong> Tools maintain their position while you&apos;re
        interacting. Enable a tool and notice it doesn&apos;t jump - you can configure it (e.g.,
        select vector stores for file_search) before moving your mouse away. Only after leaving the
        toolbar does it collapse to show just enabled tools.
      </p>
      <InteractiveToolsBar
        vectorStoreIds={[]}
        vectorStoreOwnerType="user"
        vectorStoreOwnerId="user_123"
      />
    </div>
  ),
};

/** Mock models for sub-agent stories */
const mockModels = [
  {
    id: "openai/gpt-4o",
    name: "GPT-4o",
    provider: "openai",
    context_length: 128000,
    capabilities: { tools: true, vision: true },
  },
  {
    id: "openai/gpt-4o-mini",
    name: "GPT-4o Mini",
    provider: "openai",
    context_length: 128000,
    capabilities: { tools: true, vision: true },
  },
  {
    id: "anthropic/claude-3-5-sonnet",
    name: "Claude 3.5 Sonnet",
    provider: "anthropic",
    context_length: 200000,
    capabilities: { tools: true, vision: true },
  },
  {
    id: "anthropic/claude-3-haiku",
    name: "Claude 3 Haiku",
    provider: "anthropic",
    context_length: 200000,
    capabilities: { tools: true, vision: false },
  },
];

/** Interactive wrapper with sub-agent model selection */
function InteractiveToolsBarWithSubAgent({
  initialTools = ["sub_agent"],
  initialModel = null as string | null,
}: {
  initialTools?: string[];
  initialModel?: string | null;
}) {
  const [enabledTools, setEnabledTools] = useState<string[]>(initialTools);
  const [subAgentModel, setSubAgentModel] = useState<string | null>(initialModel);

  return (
    <div className="space-y-4">
      <ToolsBar
        enabledTools={enabledTools}
        onEnabledToolsChange={setEnabledTools}
        availableModels={mockModels}
        subAgentModel={subAgentModel}
        onSubAgentModelChange={setSubAgentModel}
      />
      <div className="text-xs text-muted-foreground space-y-1">
        <div>Enabled tools: {enabledTools.length > 0 ? enabledTools.join(", ") : "none"}</div>
        <div>Sub-agent model: {subAgentModel || "(use current model)"}</div>
      </div>
    </div>
  );
}

export const SubAgentTool: Story = {
  render: () => (
    <div className="space-y-4">
      <p className="text-sm text-muted-foreground max-w-md">
        The sub_agent tool allows delegating investigative tasks to a separate AI agent. Hover over
        the tool to see the model selector for configuring the default sub-agent model.
      </p>
      <InteractiveToolsBarWithSubAgent />
    </div>
  ),
};

export const SubAgentWithSelectedModel: Story = {
  render: () => (
    <div className="space-y-4">
      <p className="text-sm text-muted-foreground max-w-md">
        Sub-agent with a pre-selected model (GPT-4o Mini). This model will be used when the main
        model invokes the sub_agent tool.
      </p>
      <InteractiveToolsBarWithSubAgent initialModel="openai/gpt-4o-mini" />
    </div>
  ),
};

/** Interactive wrapper for sub-agent with other tools */
function SubAgentWithOtherToolsDemo() {
  const [enabledTools, setEnabledTools] = useState<string[]>([
    "sub_agent",
    "code_interpreter",
    "sql_query",
  ]);
  const [subAgentModel, setSubAgentModel] = useState<string | null>("anthropic/claude-3-haiku");
  const [vectorStoreIds, setVectorStoreIds] = useState<string[]>([]);

  return (
    <div className="space-y-4">
      <p className="text-sm text-muted-foreground max-w-md">
        Sub-agent tool combined with other tools. Each tool has its own configuration panel
        accessible on hover.
      </p>
      <ToolsBar
        enabledTools={enabledTools}
        onEnabledToolsChange={setEnabledTools}
        vectorStoreIds={vectorStoreIds}
        onVectorStoreIdsChange={setVectorStoreIds}
        vectorStoreOwnerType="user"
        vectorStoreOwnerId="user_123"
        availableModels={mockModels}
        subAgentModel={subAgentModel}
        onSubAgentModelChange={setSubAgentModel}
      />
      <div className="text-xs text-muted-foreground space-y-1">
        <div>Enabled: {enabledTools.join(", ")}</div>
        <div>Sub-agent model: {subAgentModel || "(use current)"}</div>
      </div>
    </div>
  );
}

export const SubAgentWithOtherTools: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("/api/admin/vector_stores", () => {
          return HttpResponse.json({
            data: mockVectorStores,
            has_more: false,
          });
        }),
      ],
    },
  },
  render: () => <SubAgentWithOtherToolsDemo />,
};

// =============================================================================
// MCP Tool Stories
// =============================================================================

/** Mock MCP server data */
const mockMCPServers = [
  {
    id: "mcp-1",
    name: "GitHub Tools",
    url: "https://mcp.github.com",
    enabled: true,
    status: "connected" as MCPConnectionStatus,
    tools: [
      { name: "github_search", description: "Search GitHub repositories", inputSchema: {} },
      { name: "github_issues", description: "Manage GitHub issues", inputSchema: {} },
    ],
    resources: [],
    prompts: [],
    toolsEnabled: { github_search: true, github_issues: true },
  },
  {
    id: "mcp-2",
    name: "Slack Integration",
    url: "https://mcp.slack.com",
    enabled: true,
    status: "connected" as MCPConnectionStatus,
    tools: [
      { name: "slack_send", description: "Send Slack messages", inputSchema: {} },
      { name: "slack_search", description: "Search Slack messages", inputSchema: {} },
      { name: "slack_channels", description: "List Slack channels", inputSchema: {} },
    ],
    resources: [],
    prompts: [],
    toolsEnabled: { slack_send: true, slack_search: true, slack_channels: true },
  },
];

const mockMCPServersWithError = [
  {
    id: "mcp-1",
    name: "GitHub Tools",
    url: "https://mcp.github.com",
    enabled: true,
    status: "connected" as MCPConnectionStatus,
    tools: [{ name: "github_search", description: "Search GitHub", inputSchema: {} }],
    resources: [],
    prompts: [],
    toolsEnabled: { github_search: true },
  },
  {
    id: "mcp-2",
    name: "Broken Server",
    url: "https://mcp.broken.com",
    enabled: true,
    status: "error" as MCPConnectionStatus,
    error: "Connection refused: ECONNREFUSED",
    tools: [],
    resources: [],
    prompts: [],
    toolsEnabled: {},
  },
];

/** Interactive wrapper with MCP store setup */
function InteractiveToolsBarWithMCP({
  initialTools = ["mcp"],
  servers = [] as typeof mockMCPServers,
}: {
  initialTools?: string[];
  servers?: typeof mockMCPServers;
}) {
  const [enabledTools, setEnabledTools] = useState<string[]>(initialTools);
  const [configOpen, setConfigOpen] = useState(false);

  // Set up MCP store state
  useEffect(() => {
    const store = useMCPStore.getState();
    // Clear existing servers
    store.servers.forEach((s) => store.removeServer(s.id));
    // Add mock servers directly to state (bypassing connect flow)
    useMCPStore.setState({ servers });
    return () => {
      // Clean up on unmount
      useMCPStore.setState({ servers: [] });
    };
  }, [servers]);

  return (
    <div className="space-y-4">
      <ToolsBar
        enabledTools={enabledTools}
        onEnabledToolsChange={setEnabledTools}
        onOpenMCPConfig={() => setConfigOpen(true)}
      />
      <div className="text-xs text-muted-foreground space-y-1">
        <div>Enabled tools: {enabledTools.length > 0 ? enabledTools.join(", ") : "none"}</div>
        <div>MCP config modal: {configOpen ? "open" : "closed"}</div>
      </div>
    </div>
  );
}

export const MCPTool: Story = {
  render: () => (
    <div className="space-y-4">
      <p className="text-sm text-muted-foreground max-w-md">
        The MCP (Model Context Protocol) tool allows connecting to external MCP servers to access
        additional tools and data sources. Hover over the tool to see server status and the
        &quot;Manage Servers&quot; button.
      </p>
      <InteractiveToolsBarWithMCP servers={[]} />
    </div>
  ),
};

export const MCPWithConnectedServers: Story = {
  render: () => (
    <div className="space-y-4">
      <p className="text-sm text-muted-foreground max-w-md">
        MCP with two connected servers providing 5 tools total. Hover to see the server stats and
        available tool count.
      </p>
      <InteractiveToolsBarWithMCP servers={mockMCPServers} />
    </div>
  ),
};

export const MCPWithError: Story = {
  render: () => (
    <div className="space-y-4">
      <p className="text-sm text-muted-foreground max-w-md">
        MCP with one connected server and one with a connection error. The error indicator shows in
        the flyout.
      </p>
      <InteractiveToolsBarWithMCP servers={mockMCPServersWithError} />
    </div>
  ),
};

/** MCP combined with other tools */
function MCPWithOtherToolsDemo() {
  const [enabledTools, setEnabledTools] = useState<string[]>([
    "mcp",
    "code_interpreter",
    "file_search",
  ]);
  const [vectorStoreIds, setVectorStoreIds] = useState<string[]>(["vs_001"]);
  const [configOpen, setConfigOpen] = useState(false);

  // Set up MCP store state
  useEffect(() => {
    useMCPStore.setState({ servers: mockMCPServers });
    return () => {
      useMCPStore.setState({ servers: [] });
    };
  }, []);

  return (
    <div className="space-y-4">
      <p className="text-sm text-muted-foreground max-w-md">
        MCP combined with other tools. Each tool has its own configuration panel. The MCP tools from
        connected servers will be merged with built-in tools when sent to the LLM.
      </p>
      <ToolsBar
        enabledTools={enabledTools}
        onEnabledToolsChange={setEnabledTools}
        vectorStoreIds={vectorStoreIds}
        onVectorStoreIdsChange={setVectorStoreIds}
        vectorStoreOwnerType="user"
        vectorStoreOwnerId="user_123"
        onOpenMCPConfig={() => setConfigOpen(true)}
      />
      <div className="text-xs text-muted-foreground space-y-1">
        <div>Enabled: {enabledTools.join(", ")}</div>
        <div>MCP config: {configOpen ? "open" : "closed"}</div>
      </div>
    </div>
  );
}

export const MCPWithOtherTools: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("/api/admin/vector_stores", () => {
          return HttpResponse.json({
            data: mockVectorStores,
            has_more: false,
          });
        }),
      ],
    },
  },
  render: () => <MCPWithOtherToolsDemo />,
};
