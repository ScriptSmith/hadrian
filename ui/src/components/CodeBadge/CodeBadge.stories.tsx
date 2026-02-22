import type { Meta, StoryObj } from "@storybook/react";

import { CodeBadge } from "./CodeBadge";

const meta: Meta<typeof CodeBadge> = {
  title: "Components/CodeBadge",
  component: CodeBadge,
  parameters: {
    layout: "centered",
  },
};

export default meta;
type Story = StoryObj<typeof CodeBadge>;

export const Default: Story = {
  args: {
    children: "my-slug",
  },
};

export const WithEllipsis: Story = {
  args: {
    children: "sk-abc123...",
  },
};

export const LongText: Story = {
  args: {
    children: "a-very-long-slug-that-might-need-truncation",
    truncate: true,
  },
  decorators: [
    (Story) => (
      <div className="w-32">
        <Story />
      </div>
    ),
  ],
};

export const InContext: Story = {
  render: () => (
    <div className="space-y-2">
      <div className="flex items-center gap-2">
        <span className="font-medium">Organization:</span>
        <CodeBadge>acme-corp</CodeBadge>
      </div>
      <div className="flex items-center gap-2">
        <span className="font-medium">Key Prefix:</span>
        <CodeBadge>sk-abc123...</CodeBadge>
      </div>
      <div className="flex items-center gap-2">
        <span className="font-medium">External ID:</span>
        <CodeBadge>user_2abc3def4ghi</CodeBadge>
      </div>
    </div>
  ),
};

export const TableExample: Story = {
  render: () => (
    <table className="w-full">
      <thead>
        <tr className="border-b">
          <th className="px-4 py-2 text-left">Name</th>
          <th className="px-4 py-2 text-left">Slug</th>
        </tr>
      </thead>
      <tbody>
        <tr className="border-b">
          <td className="px-4 py-2">Acme Corp</td>
          <td className="px-4 py-2">
            <CodeBadge>acme-corp</CodeBadge>
          </td>
        </tr>
        <tr className="border-b">
          <td className="px-4 py-2">Globex</td>
          <td className="px-4 py-2">
            <CodeBadge>globex</CodeBadge>
          </td>
        </tr>
      </tbody>
    </table>
  ),
};
