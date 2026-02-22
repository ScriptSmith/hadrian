import type { Meta, StoryObj } from "@storybook/react";
import { Settings, User, LogOut, Trash2, Edit, MoreHorizontal } from "lucide-react";
import {
  Dropdown,
  DropdownTrigger,
  DropdownContent,
  DropdownItem,
  DropdownSeparator,
  DropdownLabel,
} from "./Dropdown";

const meta: Meta<typeof Dropdown> = {
  title: "UI/Dropdown",
  component: Dropdown,
  parameters: {
    layout: "centered",
  },
};

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  render: () => (
    <Dropdown>
      <DropdownTrigger>Options</DropdownTrigger>
      <DropdownContent>
        <DropdownItem>Profile</DropdownItem>
        <DropdownItem>Settings</DropdownItem>
        <DropdownItem>Help</DropdownItem>
      </DropdownContent>
    </Dropdown>
  ),
};

export const WithIcons: Story = {
  render: () => (
    <Dropdown>
      <DropdownTrigger>My Account</DropdownTrigger>
      <DropdownContent>
        <DropdownItem>
          <User className="mr-2 h-4 w-4" />
          Profile
        </DropdownItem>
        <DropdownItem>
          <Settings className="mr-2 h-4 w-4" />
          Settings
        </DropdownItem>
        <DropdownSeparator />
        <DropdownItem>
          <LogOut className="mr-2 h-4 w-4" />
          Log out
        </DropdownItem>
      </DropdownContent>
    </Dropdown>
  ),
};

export const WithLabels: Story = {
  render: () => (
    <Dropdown>
      <DropdownTrigger>Filter</DropdownTrigger>
      <DropdownContent>
        <DropdownLabel>Status</DropdownLabel>
        <DropdownItem>Active</DropdownItem>
        <DropdownItem>Inactive</DropdownItem>
        <DropdownSeparator />
        <DropdownLabel>Role</DropdownLabel>
        <DropdownItem>Admin</DropdownItem>
        <DropdownItem>User</DropdownItem>
        <DropdownItem>Viewer</DropdownItem>
      </DropdownContent>
    </Dropdown>
  ),
};

export const WithSelection: Story = {
  render: () => (
    <Dropdown>
      <DropdownTrigger>Sort by</DropdownTrigger>
      <DropdownContent>
        <DropdownItem selected>Name</DropdownItem>
        <DropdownItem>Date Created</DropdownItem>
        <DropdownItem>Last Modified</DropdownItem>
      </DropdownContent>
    </Dropdown>
  ),
};

export const AlignEnd: Story = {
  render: () => (
    <div className="flex justify-end">
      <Dropdown>
        <DropdownTrigger>Actions</DropdownTrigger>
        <DropdownContent align="end">
          <DropdownItem>View</DropdownItem>
          <DropdownItem>Edit</DropdownItem>
          <DropdownItem>Delete</DropdownItem>
        </DropdownContent>
      </Dropdown>
    </div>
  ),
};

export const ActionsMenu: Story = {
  render: () => (
    <Dropdown>
      <DropdownTrigger variant="ghost" className="h-8 w-8 p-0" aria-label="Actions">
        <MoreHorizontal className="h-4.5 w-4.5" />
      </DropdownTrigger>
      <DropdownContent align="end">
        <DropdownItem>
          <Edit className="mr-2 h-4 w-4" />
          Edit
        </DropdownItem>
        <DropdownItem>
          <Trash2 className="mr-2 h-4 w-4 text-destructive" />
          <span className="text-destructive">Delete</span>
        </DropdownItem>
      </DropdownContent>
    </Dropdown>
  ),
};

export const DisabledItems: Story = {
  render: () => (
    <Dropdown>
      <DropdownTrigger>Options</DropdownTrigger>
      <DropdownContent>
        <DropdownItem>Available</DropdownItem>
        <DropdownItem disabled>Disabled</DropdownItem>
        <DropdownItem>Another Option</DropdownItem>
      </DropdownContent>
    </Dropdown>
  ),
};

export const InContext: Story = {
  render: () => (
    <div className="flex items-center justify-between rounded-lg border p-4" style={{ width: 400 }}>
      <div>
        <div className="font-medium">API Key</div>
        <div className="text-sm text-muted-foreground">gw_live_abc123...</div>
      </div>
      <Dropdown>
        <DropdownTrigger className="h-8 w-8 p-0" aria-label="Actions">
          <MoreHorizontal className="h-4 w-4" />
        </DropdownTrigger>
        <DropdownContent align="end">
          <DropdownItem>
            <Edit className="mr-2 h-4 w-4" />
            Rename
          </DropdownItem>
          <DropdownItem>Copy Key</DropdownItem>
          <DropdownSeparator />
          <DropdownItem>
            <Trash2 className="mr-2 h-4 w-4 text-destructive" />
            <span className="text-destructive">Revoke</span>
          </DropdownItem>
        </DropdownContent>
      </Dropdown>
    </div>
  ),
};
