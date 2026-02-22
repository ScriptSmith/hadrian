import type { Meta, StoryObj } from "@storybook/react";
import { useState, useEffect } from "react";

import { MCPConfigModal } from "./MCPConfigModal";
import { useMCPStore } from "@/stores/mcpStore";
import type { MCPConnectionStatus, MCPToolDefinition } from "@/services/mcp";
import { Button } from "@/components/Button/Button";

const meta = {
  title: "Components/MCPConfigModal",
  component: MCPConfigModal,
  parameters: {
    layout: "centered",
  },
} satisfies Meta<typeof MCPConfigModal>;

export default meta;
type Story = StoryObj<typeof meta>;

// =============================================================================
// Mock Data
// =============================================================================

const mockTools: MCPToolDefinition[] = [
  {
    name: "github_search",
    description: "Search GitHub repositories and code",
    inputSchema: { type: "object", properties: { query: { type: "string" } } },
  },
  {
    name: "github_issues",
    description: "Create, list, and manage GitHub issues",
    inputSchema: { type: "object", properties: { repo: { type: "string" } } },
  },
  {
    name: "github_pr",
    description: "Create and review pull requests",
    inputSchema: { type: "object", properties: { repo: { type: "string" } } },
  },
];

const mockSlackTools: MCPToolDefinition[] = [
  {
    name: "slack_send",
    description: "Send messages to Slack channels",
    inputSchema: {
      type: "object",
      properties: { channel: { type: "string" }, message: { type: "string" } },
    },
  },
  {
    name: "slack_search",
    description: "Search Slack messages and files",
    inputSchema: { type: "object", properties: { query: { type: "string" } } },
  },
];

interface MockServer {
  id: string;
  name: string;
  url: string;
  enabled: boolean;
  status: MCPConnectionStatus;
  error?: string;
  tools: MCPToolDefinition[];
  resources: never[];
  prompts: never[];
  toolsEnabled: Record<string, boolean>;
}

const createMockServers = (overrides: Partial<MockServer>[] = []): MockServer[] => {
  const defaults: MockServer[] = [
    {
      id: "mcp-github",
      name: "GitHub Tools",
      url: "https://mcp.github.com",
      enabled: true,
      status: "connected",
      tools: mockTools,
      resources: [],
      prompts: [],
      toolsEnabled: { github_search: true, github_issues: true, github_pr: true },
    },
    {
      id: "mcp-slack",
      name: "Slack Integration",
      url: "https://mcp.slack.com",
      enabled: true,
      status: "connected",
      tools: mockSlackTools,
      resources: [],
      prompts: [],
      toolsEnabled: { slack_send: true, slack_search: true },
    },
  ];

  return overrides.length > 0
    ? overrides.map((o, i) => ({ ...defaults[i % defaults.length], ...o }))
    : defaults;
};

// =============================================================================
// Story Helpers
// =============================================================================

/** Interactive wrapper that opens the modal */
function ModalWrapper({
  initialServers = [],
  initialOpen = true,
}: {
  initialServers?: MockServer[];
  initialOpen?: boolean;
}) {
  const [open, setOpen] = useState(initialOpen);

  // Set up mock servers in the store
  useEffect(() => {
    useMCPStore.setState({ servers: initialServers });
    return () => {
      useMCPStore.setState({ servers: [] });
    };
  }, [initialServers]);

  return (
    <div className="space-y-4">
      <Button onClick={() => setOpen(true)}>Open MCP Config</Button>
      <MCPConfigModal open={open} onClose={() => setOpen(false)} />
    </div>
  );
}

// =============================================================================
// Stories
// =============================================================================

export const Default: Story = {
  render: () => <ModalWrapper initialServers={createMockServers()} />,
};

export const Empty: Story = {
  render: () => (
    <div className="space-y-4">
      <p className="text-sm text-muted-foreground max-w-md">
        Empty state when no MCP servers are configured. Shows a hint to add servers.
      </p>
      <ModalWrapper initialServers={[]} />
    </div>
  ),
};

export const WithConnectedServers: Story = {
  render: () => (
    <div className="space-y-4">
      <p className="text-sm text-muted-foreground max-w-md">
        Two connected servers with 5 tools total. Expand each server to see and toggle individual
        tools.
      </p>
      <ModalWrapper initialServers={createMockServers()} />
    </div>
  ),
};

export const WithDisconnectedServer: Story = {
  render: () => (
    <div className="space-y-4">
      <p className="text-sm text-muted-foreground max-w-md">
        One connected server and one disconnected. The disconnected server shows no tools.
      </p>
      <ModalWrapper
        initialServers={createMockServers([
          { status: "connected", tools: mockTools },
          {
            id: "mcp-notion",
            name: "Notion API",
            url: "https://mcp.notion.so",
            status: "disconnected",
            tools: [],
          },
        ])}
      />
    </div>
  ),
};

export const WithConnectionError: Story = {
  render: () => (
    <div className="space-y-4">
      <p className="text-sm text-muted-foreground max-w-md">
        One server with a connection error. The error message is displayed below the server info.
      </p>
      <ModalWrapper
        initialServers={createMockServers([
          { status: "connected", tools: mockTools },
          {
            id: "mcp-broken",
            name: "Broken Server",
            url: "https://mcp.broken.com",
            status: "error",
            error: "Connection refused: ECONNREFUSED 127.0.0.1:8080",
            tools: [],
          },
        ])}
      />
    </div>
  ),
};

export const WithConnectingServer: Story = {
  render: () => (
    <div className="space-y-4">
      <p className="text-sm text-muted-foreground max-w-md">
        One server in &quot;connecting&quot; state with a loading spinner.
      </p>
      <ModalWrapper
        initialServers={createMockServers([
          { status: "connected", tools: mockTools },
          {
            id: "mcp-connecting",
            name: "Connecting Server",
            url: "https://mcp.slow.com",
            status: "connecting",
            tools: [],
          },
        ])}
      />
    </div>
  ),
};

export const WithDisabledServer: Story = {
  render: () => (
    <div className="space-y-4">
      <p className="text-sm text-muted-foreground max-w-md">
        A server that is disabled. Disabled servers don&apos;t show connect button and their tools
        are not available.
      </p>
      <ModalWrapper
        initialServers={createMockServers([
          { status: "connected", tools: mockTools },
          {
            id: "mcp-disabled",
            name: "Disabled Server",
            url: "https://mcp.disabled.com",
            enabled: false,
            status: "disconnected",
            tools: [],
          },
        ])}
      />
    </div>
  ),
};

export const WithManyTools: Story = {
  render: () => (
    <div className="space-y-4">
      <p className="text-sm text-muted-foreground max-w-md">
        A server with many tools. Demonstrates scrolling behavior in the tools list.
      </p>
      <ModalWrapper
        initialServers={[
          {
            id: "mcp-comprehensive",
            name: "Comprehensive Tools",
            url: "https://mcp.comprehensive.com",
            enabled: true,
            status: "connected",
            tools: [
              { name: "file_read", description: "Read file contents", inputSchema: {} },
              { name: "file_write", description: "Write to files", inputSchema: {} },
              { name: "file_delete", description: "Delete files", inputSchema: {} },
              { name: "directory_list", description: "List directory contents", inputSchema: {} },
              { name: "http_get", description: "Make HTTP GET requests", inputSchema: {} },
              { name: "http_post", description: "Make HTTP POST requests", inputSchema: {} },
              { name: "database_query", description: "Execute SQL queries", inputSchema: {} },
              { name: "cache_get", description: "Get cached values", inputSchema: {} },
              { name: "cache_set", description: "Set cache values", inputSchema: {} },
              { name: "email_send", description: "Send email messages", inputSchema: {} },
            ],
            resources: [],
            prompts: [],
            toolsEnabled: {},
          },
        ]}
      />
    </div>
  ),
};

export const Closed: Story = {
  render: () => (
    <div className="space-y-4">
      <p className="text-sm text-muted-foreground max-w-md">
        Modal in closed state. Click the button to open it.
      </p>
      <ModalWrapper initialServers={createMockServers()} initialOpen={false} />
    </div>
  ),
};
