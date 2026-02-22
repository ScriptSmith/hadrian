import type { Meta, StoryObj } from "@storybook/react";

import { ApiKeyStatusBadge, EnabledStatusBadge, SimpleStatusBadge } from "./StatusBadge";

const meta: Meta = {
  title: "Admin/StatusBadge",
  parameters: {
    layout: "centered",
  },
};

export default meta;

export const ApiKeyActive: StoryObj<typeof ApiKeyStatusBadge> = {
  render: () => <ApiKeyStatusBadge />,
};

export const ApiKeyRevoked: StoryObj<typeof ApiKeyStatusBadge> = {
  render: () => <ApiKeyStatusBadge revokedAt="2024-01-15T10:30:00Z" />,
};

export const ApiKeyExpired: StoryObj<typeof ApiKeyStatusBadge> = {
  render: () => <ApiKeyStatusBadge expiresAt="2023-01-15T10:30:00Z" />,
};

export const Enabled: StoryObj<typeof EnabledStatusBadge> = {
  render: () => <EnabledStatusBadge isEnabled={true} />,
};

export const Disabled: StoryObj<typeof EnabledStatusBadge> = {
  render: () => <EnabledStatusBadge isEnabled={false} />,
};

export const EnabledWithoutIcon: StoryObj<typeof EnabledStatusBadge> = {
  render: () => <EnabledStatusBadge isEnabled={true} showIcon={false} />,
};

export const SimpleStatuses: StoryObj<typeof SimpleStatusBadge> = {
  render: () => (
    <div className="flex gap-2">
      <SimpleStatusBadge status="active" />
      <SimpleStatusBadge status="inactive" />
      <SimpleStatusBadge status="success" />
      <SimpleStatusBadge status="error" />
      <SimpleStatusBadge status="warning" />
      <SimpleStatusBadge status="pending" />
    </div>
  ),
};

export const CustomLabels: StoryObj<typeof SimpleStatusBadge> = {
  render: () => (
    <div className="flex gap-2">
      <SimpleStatusBadge status="active" label="Online" />
      <SimpleStatusBadge status="inactive" label="Offline" />
      <SimpleStatusBadge status="pending" label="Processing" />
    </div>
  ),
};

export const TableContext: StoryObj = {
  render: () => (
    <table className="w-full">
      <thead>
        <tr className="border-b">
          <th className="px-4 py-2 text-left">Name</th>
          <th className="px-4 py-2 text-left">Status</th>
        </tr>
      </thead>
      <tbody>
        <tr className="border-b">
          <td className="px-4 py-2">Production Key</td>
          <td className="px-4 py-2">
            <ApiKeyStatusBadge />
          </td>
        </tr>
        <tr className="border-b">
          <td className="px-4 py-2">Old Key</td>
          <td className="px-4 py-2">
            <ApiKeyStatusBadge revokedAt="2024-01-15T10:30:00Z" />
          </td>
        </tr>
        <tr className="border-b">
          <td className="px-4 py-2">Trial Key</td>
          <td className="px-4 py-2">
            <ApiKeyStatusBadge expiresAt="2023-12-31T00:00:00Z" />
          </td>
        </tr>
      </tbody>
    </table>
  ),
};

export const ProviderTable: StoryObj = {
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
            <EnabledStatusBadge isEnabled={true} />
          </td>
        </tr>
        <tr className="border-b">
          <td className="px-4 py-2">Anthropic</td>
          <td className="px-4 py-2">
            <EnabledStatusBadge isEnabled={true} />
          </td>
        </tr>
        <tr className="border-b">
          <td className="px-4 py-2">Local Model</td>
          <td className="px-4 py-2">
            <EnabledStatusBadge isEnabled={false} />
          </td>
        </tr>
      </tbody>
    </table>
  ),
};
