import type { Meta, StoryObj } from "@storybook/react";

import { CircuitBreakerBadge } from "./CircuitBreakerBadge";

const meta: Meta<typeof CircuitBreakerBadge> = {
  title: "Admin/CircuitBreakerBadge",
  component: CircuitBreakerBadge,
  parameters: {
    layout: "centered",
  },
};

export default meta;
type Story = StoryObj<typeof CircuitBreakerBadge>;

export const Closed: Story = {
  args: {
    state: "closed",
  },
};

export const Open: Story = {
  args: {
    state: "open",
  },
};

export const HalfOpen: Story = {
  args: {
    state: "half_open",
  },
};

export const WithoutIcon: Story = {
  args: {
    state: "closed",
    showIcon: false,
  },
};

export const AllStates: Story = {
  render: () => (
    <div className="flex gap-2">
      <CircuitBreakerBadge state="closed" />
      <CircuitBreakerBadge state="open" />
      <CircuitBreakerBadge state="half_open" />
    </div>
  ),
};

export const TableContext: Story = {
  render: () => (
    <table className="w-full">
      <thead>
        <tr className="border-b">
          <th className="px-4 py-2 text-left">Provider</th>
          <th className="px-4 py-2 text-left">Circuit State</th>
          <th className="px-4 py-2 text-left">Failures</th>
        </tr>
      </thead>
      <tbody>
        <tr className="border-b">
          <td className="px-4 py-2">openai</td>
          <td className="px-4 py-2">
            <CircuitBreakerBadge state="closed" />
          </td>
          <td className="px-4 py-2">0</td>
        </tr>
        <tr className="border-b">
          <td className="px-4 py-2">anthropic</td>
          <td className="px-4 py-2">
            <CircuitBreakerBadge state="open" />
          </td>
          <td className="px-4 py-2">5</td>
        </tr>
        <tr className="border-b">
          <td className="px-4 py-2">azure-openai</td>
          <td className="px-4 py-2">
            <CircuitBreakerBadge state="half_open" />
          </td>
          <td className="px-4 py-2">3</td>
        </tr>
      </tbody>
    </table>
  ),
};
