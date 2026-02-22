import type { Meta, StoryObj } from "@storybook/react";
import { fn } from "storybook/test";
import { ScimConfigFormModal } from "./ScimConfigFormModal";
import type { OrgScimConfig, Team } from "@/api/generated/types.gen";

const mockTeams: Team[] = [
  {
    id: "team-1",
    org_id: "org-1",
    name: "Engineering",
    slug: "engineering",
    created_at: "2024-01-01T00:00:00Z",
    updated_at: "2024-01-01T00:00:00Z",
  },
  {
    id: "team-2",
    org_id: "org-1",
    name: "Platform",
    slug: "platform",
    created_at: "2024-01-01T00:00:00Z",
    updated_at: "2024-01-01T00:00:00Z",
  },
];

const mockExistingConfig: OrgScimConfig = {
  id: "scim-config-1",
  org_id: "org-1",
  enabled: true,
  token_prefix: "scim_abc",
  token_last_used_at: "2024-06-15T10:30:00Z",
  create_users: true,
  sync_display_name: true,
  default_team_id: "team-1",
  default_org_role: "member",
  default_team_role: "member",
  deactivate_deletes_user: false,
  revoke_api_keys_on_deactivate: true,
  created_at: "2024-01-01T00:00:00Z",
  updated_at: "2024-06-15T10:30:00Z",
};

const meta: Meta<typeof ScimConfigFormModal> = {
  title: "Admin/ScimConfigFormModal",
  component: ScimConfigFormModal,
  decorators: [
    (Story) => (
      <div style={{ minHeight: "600px" }}>
        <Story />
      </div>
    ),
  ],
  args: {
    open: true,
    onClose: fn(),
    onCreateSubmit: fn(),
    onUpdateSubmit: fn(),
    isLoading: false,
    editingConfig: null,
    teams: mockTeams,
  },
};

export default meta;
type Story = StoryObj<typeof ScimConfigFormModal>;

export const CreateMode: Story = {
  args: {
    editingConfig: null,
  },
};

export const EditMode: Story = {
  args: {
    editingConfig: mockExistingConfig,
  },
};

export const EditModeDisabled: Story = {
  args: {
    editingConfig: {
      ...mockExistingConfig,
      enabled: false,
    },
  },
};

export const Loading: Story = {
  args: {
    isLoading: true,
  },
};

export const NoTeams: Story = {
  args: {
    teams: [],
    editingConfig: null,
  },
};

export const AggressiveDeprovisioning: Story = {
  args: {
    editingConfig: {
      ...mockExistingConfig,
      deactivate_deletes_user: true,
      revoke_api_keys_on_deactivate: true,
    },
  },
};
