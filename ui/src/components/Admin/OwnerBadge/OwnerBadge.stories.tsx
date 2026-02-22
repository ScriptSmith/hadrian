import type { Meta, StoryObj } from "@storybook/react";

import { OwnerBadge } from "./OwnerBadge";

const meta: Meta<typeof OwnerBadge> = {
  title: "Admin/OwnerBadge",
  component: OwnerBadge,
  parameters: {
    layout: "centered",
  },
};

export default meta;
type Story = StoryObj<typeof OwnerBadge>;

export const Global: Story = {
  args: {
    owner: { type: "global" },
  },
};

export const Organization: Story = {
  args: {
    owner: { type: "organization", org_id: "abc12345-def6-7890-ghij-klmnopqrstuv" },
  },
};

export const OrganizationWithId: Story = {
  args: {
    owner: { type: "organization", org_id: "abc12345-def6-7890-ghij-klmnopqrstuv" },
    showId: true,
  },
};

export const Team: Story = {
  args: {
    owner: { type: "team", team_id: "team1234-def6-7890-ghij-klmnopqrstuv" },
  },
};

export const TeamWithId: Story = {
  args: {
    owner: { type: "team", team_id: "team1234-def6-7890-ghij-klmnopqrstuv" },
    showId: true,
  },
};

export const Project: Story = {
  args: {
    owner: { type: "project", project_id: "proj1234-def6-7890-ghij-klmnopqrstuv" },
  },
};

export const ProjectWithId: Story = {
  args: {
    owner: { type: "project", project_id: "proj1234-def6-7890-ghij-klmnopqrstuv" },
    showId: true,
  },
};

export const User: Story = {
  args: {
    owner: { type: "user", user_id: "user5678-def6-7890-ghij-klmnopqrstuv" },
  },
};

export const UserWithId: Story = {
  args: {
    owner: { type: "user", user_id: "user5678-def6-7890-ghij-klmnopqrstuv" },
    showId: true,
  },
};

export const AllTypes: Story = {
  render: () => (
    <div className="flex gap-2">
      <OwnerBadge owner={{ type: "global" }} />
      <OwnerBadge owner={{ type: "organization", org_id: "abc12345" }} />
      <OwnerBadge owner={{ type: "team", team_id: "team1234" }} />
      <OwnerBadge owner={{ type: "project", project_id: "proj1234" }} />
      <OwnerBadge owner={{ type: "user", user_id: "user5678" }} />
    </div>
  ),
};

export const TableContext: Story = {
  render: () => (
    <table className="w-full">
      <thead>
        <tr className="border-b">
          <th className="px-4 py-2 text-left">Name</th>
          <th className="px-4 py-2 text-left">Owner</th>
        </tr>
      </thead>
      <tbody>
        <tr className="border-b">
          <td className="px-4 py-2">Default Pricing</td>
          <td className="px-4 py-2">
            <OwnerBadge owner={{ type: "global" }} />
          </td>
        </tr>
        <tr className="border-b">
          <td className="px-4 py-2">Acme API Key</td>
          <td className="px-4 py-2">
            <OwnerBadge owner={{ type: "organization", org_id: "abc12345" }} showId />
          </td>
        </tr>
        <tr className="border-b">
          <td className="px-4 py-2">Dev Project Key</td>
          <td className="px-4 py-2">
            <OwnerBadge owner={{ type: "project", project_id: "proj1234" }} showId />
          </td>
        </tr>
      </tbody>
    </table>
  ),
};
