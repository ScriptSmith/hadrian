import type { Meta, StoryObj } from "@storybook/react";
import { fn } from "storybook/test";
import { SessionCard } from "./SessionCard";

const meta: Meta<typeof SessionCard> = {
  title: "Admin/SessionCard",
  component: SessionCard,
  parameters: {
    layout: "padded",
  },
  tags: ["autodocs"],
  decorators: [
    (Story) => (
      <div className="max-w-md">
        <Story />
      </div>
    ),
  ],
};

export default meta;
type Story = StoryObj<typeof meta>;

const baseSession = {
  id: "550e8400-e29b-41d4-a716-446655440000",
  created_at: new Date(Date.now() - 2 * 24 * 60 * 60 * 1000).toISOString(), // 2 days ago
  expires_at: new Date(Date.now() + 5 * 24 * 60 * 60 * 1000).toISOString(), // 5 days from now
  last_activity: new Date(Date.now() - 30 * 60 * 1000).toISOString(), // 30 minutes ago
};

export const FullDeviceInfo: Story = {
  args: {
    session: {
      ...baseSession,
      device: {
        device_description: "Chrome 120 on Windows 11",
        ip_address: "192.168.1.100",
        user_agent:
          "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
        device_id: "abc123def456",
      },
    },
    onRevoke: fn(),
  },
};

export const MinimalDeviceInfo: Story = {
  args: {
    session: {
      ...baseSession,
      device: {
        device_description: "Unknown Browser",
      },
    },
    onRevoke: fn(),
  },
};

export const NoDeviceInfo: Story = {
  args: {
    session: {
      ...baseSession,
      device: null,
    },
    onRevoke: fn(),
  },
};

export const RecentlyActive: Story = {
  args: {
    session: {
      ...baseSession,
      last_activity: new Date(Date.now() - 30 * 1000).toISOString(), // 30 seconds ago
      device: {
        device_description: "Safari on macOS Sonoma",
        ip_address: "10.0.0.50",
      },
    },
    onRevoke: fn(),
  },
};

export const NoLastActivity: Story = {
  args: {
    session: {
      id: "550e8400-e29b-41d4-a716-446655440000",
      created_at: new Date(Date.now() - 2 * 24 * 60 * 60 * 1000).toISOString(),
      expires_at: new Date(Date.now() + 5 * 24 * 60 * 60 * 1000).toISOString(),
      last_activity: null,
      device: {
        device_description: "Firefox on Ubuntu",
        ip_address: "172.16.0.1",
      },
    },
    onRevoke: fn(),
  },
};

export const Revoking: Story = {
  args: {
    session: {
      ...baseSession,
      device: {
        device_description: "Chrome 120 on Windows 11",
        ip_address: "192.168.1.100",
      },
    },
    onRevoke: fn(),
    isRevoking: true,
  },
};

export const NoRevokeAction: Story = {
  args: {
    session: {
      ...baseSession,
      device: {
        device_description: "Edge on Windows 11",
        ip_address: "10.10.10.10",
      },
    },
  },
};

export const MobileDevice: Story = {
  args: {
    session: {
      ...baseSession,
      device: {
        device_description: "Safari on iOS 17 (iPhone)",
        ip_address: "192.168.1.50",
        user_agent:
          "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1",
      },
    },
    onRevoke: fn(),
  },
};
