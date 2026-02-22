import type { Meta, StoryObj } from "@storybook/react";
import { useState } from "react";

import { UserSelect } from "./UserSelect";

const meta: Meta<typeof UserSelect> = {
  title: "Admin/UserSelect",
  component: UserSelect,
  parameters: {
    layout: "padded",
  },
};

export default meta;
type Story = StoryObj<typeof UserSelect>;

const mockUsers = [
  {
    id: "user-1",
    external_id: "alice@acme-corp.com",
    name: "Alice Johnson",
    email: "alice@acme-corp.com",
    created_at: "2024-01-01T00:00:00Z",
    updated_at: "2024-01-01T00:00:00Z",
  },
  {
    id: "user-2",
    external_id: "bob@acme-corp.com",
    name: "Bob Smith",
    email: "bob@acme-corp.com",
    created_at: "2024-01-02T00:00:00Z",
    updated_at: "2024-01-02T00:00:00Z",
  },
  {
    id: "user-3",
    external_id: "svc-ci-pipeline",
    name: null,
    email: null,
    created_at: "2024-01-03T00:00:00Z",
    updated_at: "2024-01-03T00:00:00Z",
  },
];

function DefaultStory() {
  const [value, setValue] = useState<string | null>(null);
  return <UserSelect users={mockUsers} value={value} onChange={setValue} />;
}

export const Default: Story = {
  render: () => <DefaultStory />,
};

function WithSelectedUserStory() {
  const [value, setValue] = useState<string | null>("user-2");
  return <UserSelect users={mockUsers} value={value} onChange={setValue} />;
}

export const WithSelectedUser: Story = {
  render: () => <WithSelectedUserStory />,
};

function NoNoneOptionStory() {
  const [value, setValue] = useState<string | null>("user-1");
  return <UserSelect users={mockUsers} value={value} onChange={setValue} allowNone={false} />;
}

export const NoNoneOption: Story = {
  render: () => <NoNoneOptionStory />,
  parameters: {
    docs: {
      description: {
        story: "When allowNone is false, the 'All users' option is not shown.",
      },
    },
  },
};

function CustomLabelsStory() {
  const [value, setValue] = useState<string | null>(null);
  return (
    <UserSelect
      users={mockUsers}
      value={value}
      onChange={setValue}
      label="Filter by User"
      nonePlaceholder="No user filter"
    />
  );
}

export const CustomLabels: Story = {
  render: () => <CustomLabelsStory />,
};

function DisabledStory() {
  const [value, setValue] = useState<string | null>("user-1");
  return <UserSelect users={mockUsers} value={value} onChange={setValue} disabled />;
}

export const Disabled: Story = {
  render: () => <DisabledStory />,
};

function EmptyUsersStory() {
  const [value, setValue] = useState<string | null>(null);
  return <UserSelect users={[]} value={value} onChange={setValue} />;
}

export const EmptyUsers: Story = {
  render: () => <EmptyUsersStory />,
  parameters: {
    docs: {
      description: {
        story: "When there are no users, only the 'All users' option is shown.",
      },
    },
  },
};
