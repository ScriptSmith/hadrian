import type { Meta, StoryObj } from "@storybook/react";

import { ConnectionStatusIndicator } from "./ConnectionStatusIndicator";

const meta: Meta<typeof ConnectionStatusIndicator> = {
  title: "Admin/ConnectionStatusIndicator",
  component: ConnectionStatusIndicator,
  parameters: {
    layout: "centered",
  },
};

export default meta;
type Story = StoryObj<typeof ConnectionStatusIndicator>;

export const Connected: Story = {
  args: {
    status: "connected",
  },
};

export const Connecting: Story = {
  args: {
    status: "connecting",
  },
};

export const Reconnecting: Story = {
  args: {
    status: "reconnecting",
  },
};

export const DisconnectedHidden: Story = {
  name: "Disconnected (Hidden by Default)",
  args: {
    status: "disconnected",
  },
};

export const DisconnectedVisible: Story = {
  name: "Disconnected (Visible)",
  args: {
    status: "disconnected",
    showDisconnected: true,
  },
};

export const ErrorState: Story = {
  args: {
    status: "error",
    error: "Connection failed: server unreachable",
    showDisconnected: true,
  },
};

export const AllStatuses: Story = {
  render: () => (
    <div className="flex flex-col gap-4">
      <div className="flex items-center gap-4">
        <span className="w-24 text-sm text-muted-foreground">Connected:</span>
        <ConnectionStatusIndicator status="connected" />
      </div>
      <div className="flex items-center gap-4">
        <span className="w-24 text-sm text-muted-foreground">Connecting:</span>
        <ConnectionStatusIndicator status="connecting" />
      </div>
      <div className="flex items-center gap-4">
        <span className="w-24 text-sm text-muted-foreground">Reconnecting:</span>
        <ConnectionStatusIndicator status="reconnecting" />
      </div>
      <div className="flex items-center gap-4">
        <span className="w-24 text-sm text-muted-foreground">Disconnected:</span>
        <ConnectionStatusIndicator status="disconnected" showDisconnected />
      </div>
      <div className="flex items-center gap-4">
        <span className="w-24 text-sm text-muted-foreground">Error:</span>
        <ConnectionStatusIndicator status="error" error="Connection lost" showDisconnected />
      </div>
    </div>
  ),
};

export const PageHeaderContext: Story = {
  render: () => (
    <div className="rounded-lg border bg-card p-6">
      <div className="flex items-center gap-3">
        <h1 className="text-2xl font-semibold">Provider Health</h1>
        <ConnectionStatusIndicator status="connected" />
      </div>
      <p className="mt-1 text-muted-foreground">
        Monitor provider availability and circuit breaker states
      </p>
    </div>
  ),
};

export const ReconnectingInHeader: Story = {
  render: () => (
    <div className="rounded-lg border bg-card p-6">
      <div className="flex items-center gap-3">
        <h1 className="text-2xl font-semibold">Provider Health</h1>
        <ConnectionStatusIndicator status="reconnecting" />
      </div>
      <p className="mt-1 text-muted-foreground">
        Monitor provider availability and circuit breaker states
      </p>
    </div>
  ),
};
