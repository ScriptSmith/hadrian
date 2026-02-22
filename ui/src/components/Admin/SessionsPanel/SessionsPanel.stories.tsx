import type { Meta, StoryObj } from "@storybook/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { SessionsPanel } from "./SessionsPanel";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: false,
    },
  },
});

const meta: Meta<typeof SessionsPanel> = {
  title: "Admin/SessionsPanel",
  component: SessionsPanel,
  parameters: {
    layout: "padded",
  },
  tags: ["autodocs"],
  decorators: [
    (Story) => (
      <QueryClientProvider client={queryClient}>
        <div className="max-w-2xl">
          <Story />
        </div>
      </QueryClientProvider>
    ),
  ],
};

export default meta;
type Story = StoryObj<typeof meta>;

const baseSessions = [
  {
    id: "550e8400-e29b-41d4-a716-446655440001",
    created_at: new Date(Date.now() - 2 * 24 * 60 * 60 * 1000).toISOString(),
    expires_at: new Date(Date.now() + 5 * 24 * 60 * 60 * 1000).toISOString(),
    last_activity: new Date(Date.now() - 30 * 60 * 1000).toISOString(),
    device: {
      device_description: "Chrome 120 on Windows 11",
      ip_address: "192.168.1.100",
    },
  },
  {
    id: "550e8400-e29b-41d4-a716-446655440002",
    created_at: new Date(Date.now() - 1 * 24 * 60 * 60 * 1000).toISOString(),
    expires_at: new Date(Date.now() + 6 * 24 * 60 * 60 * 1000).toISOString(),
    last_activity: new Date(Date.now() - 5 * 60 * 1000).toISOString(),
    device: {
      device_description: "Safari on macOS Sonoma",
      ip_address: "10.0.0.50",
    },
  },
  {
    id: "550e8400-e29b-41d4-a716-446655440003",
    created_at: new Date(Date.now() - 7 * 24 * 60 * 60 * 1000).toISOString(),
    expires_at: new Date(Date.now() + 1 * 24 * 60 * 60 * 1000).toISOString(),
    last_activity: new Date(Date.now() - 2 * 24 * 60 * 60 * 1000).toISOString(),
    device: {
      device_description: "Firefox on Ubuntu",
      ip_address: "172.16.0.1",
    },
  },
];

export const WithSessions: Story = {
  args: {
    userId: "550e8400-e29b-41d4-a716-446655440000",
    sessions: {
      data: baseSessions,
      enhanced_enabled: true,
    },
  },
};

export const SingleSession: Story = {
  args: {
    userId: "550e8400-e29b-41d4-a716-446655440000",
    sessions: {
      data: [baseSessions[0]],
      enhanced_enabled: true,
    },
  },
};

export const EmptySessions: Story = {
  args: {
    userId: "550e8400-e29b-41d4-a716-446655440000",
    sessions: {
      data: [],
      enhanced_enabled: true,
    },
  },
};

export const EnhancedDisabled: Story = {
  args: {
    userId: "550e8400-e29b-41d4-a716-446655440000",
    sessions: {
      data: [],
      enhanced_enabled: false,
    },
  },
};

export const ManySessions: Story = {
  args: {
    userId: "550e8400-e29b-41d4-a716-446655440000",
    sessions: {
      data: [
        ...baseSessions,
        {
          id: "550e8400-e29b-41d4-a716-446655440004",
          created_at: new Date(Date.now() - 3 * 24 * 60 * 60 * 1000).toISOString(),
          expires_at: new Date(Date.now() + 4 * 24 * 60 * 60 * 1000).toISOString(),
          last_activity: new Date(Date.now() - 1 * 60 * 60 * 1000).toISOString(),
          device: {
            device_description: "Edge on Windows 11",
            ip_address: "192.168.1.101",
          },
        },
        {
          id: "550e8400-e29b-41d4-a716-446655440005",
          created_at: new Date(Date.now() - 4 * 24 * 60 * 60 * 1000).toISOString(),
          expires_at: new Date(Date.now() + 3 * 24 * 60 * 60 * 1000).toISOString(),
          last_activity: null,
          device: {
            device_description: "Safari on iOS 17 (iPhone)",
            ip_address: "192.168.1.50",
          },
        },
      ],
      enhanced_enabled: true,
    },
  },
};
