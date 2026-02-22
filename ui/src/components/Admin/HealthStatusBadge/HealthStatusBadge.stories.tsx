import type { Meta, StoryObj } from "@storybook/react";

import { HealthStatusBadge } from "./HealthStatusBadge";

const meta: Meta<typeof HealthStatusBadge> = {
  title: "Admin/HealthStatusBadge",
  component: HealthStatusBadge,
  parameters: {
    layout: "centered",
  },
};

export default meta;
type Story = StoryObj<typeof HealthStatusBadge>;

export const Healthy: Story = {
  args: {
    status: "healthy",
  },
};

export const Unhealthy: Story = {
  args: {
    status: "unhealthy",
  },
};

export const Unknown: Story = {
  args: {
    status: "unknown",
  },
};

export const WithoutIcon: Story = {
  args: {
    status: "healthy",
    showIcon: false,
  },
};

export const AllStatuses: Story = {
  render: () => (
    <div className="flex gap-2">
      <HealthStatusBadge status="healthy" />
      <HealthStatusBadge status="unhealthy" />
      <HealthStatusBadge status="unknown" />
    </div>
  ),
};

export const TableContext: Story = {
  render: () => (
    <table className="w-full">
      <thead>
        <tr className="border-b">
          <th className="px-4 py-2 text-left">Provider</th>
          <th className="px-4 py-2 text-left">Status</th>
        </tr>
      </thead>
      <tbody>
        <tr className="border-b">
          <td className="px-4 py-2">OpenAI</td>
          <td className="px-4 py-2">
            <HealthStatusBadge status="healthy" />
          </td>
        </tr>
        <tr className="border-b">
          <td className="px-4 py-2">Anthropic</td>
          <td className="px-4 py-2">
            <HealthStatusBadge status="unhealthy" />
          </td>
        </tr>
        <tr className="border-b">
          <td className="px-4 py-2">Local Model</td>
          <td className="px-4 py-2">
            <HealthStatusBadge status="unknown" />
          </td>
        </tr>
      </tbody>
    </table>
  ),
};
