import type { Meta, StoryObj } from "@storybook/react";
import { useState } from "react";

import { TeamSelect } from "./TeamSelect";

const meta: Meta<typeof TeamSelect> = {
  title: "Admin/TeamSelect",
  component: TeamSelect,
  parameters: {
    layout: "padded",
  },
};

export default meta;
type Story = StoryObj<typeof TeamSelect>;

const mockTeams = [
  {
    id: "team-1",
    org_id: "org-1",
    slug: "engineering",
    name: "Engineering",
    created_at: "2024-01-01T00:00:00Z",
    updated_at: "2024-01-01T00:00:00Z",
  },
  {
    id: "team-2",
    org_id: "org-1",
    slug: "design",
    name: "Design",
    created_at: "2024-01-02T00:00:00Z",
    updated_at: "2024-01-02T00:00:00Z",
  },
  {
    id: "team-3",
    org_id: "org-1",
    slug: "marketing",
    name: "Marketing",
    created_at: "2024-01-03T00:00:00Z",
    updated_at: "2024-01-03T00:00:00Z",
  },
];

function DefaultStory() {
  const [value, setValue] = useState<string | null>(null);
  return <TeamSelect teams={mockTeams} value={value} onChange={setValue} />;
}

export const Default: Story = {
  render: () => <DefaultStory />,
};

function WithSelectedTeamStory() {
  const [value, setValue] = useState<string | null>("team-2");
  return <TeamSelect teams={mockTeams} value={value} onChange={setValue} />;
}

export const WithSelectedTeam: Story = {
  render: () => <WithSelectedTeamStory />,
};

function NoNoneOptionStory() {
  const [value, setValue] = useState<string | null>("team-1");
  return <TeamSelect teams={mockTeams} value={value} onChange={setValue} allowNone={false} />;
}

export const NoNoneOption: Story = {
  render: () => <NoNoneOptionStory />,
  parameters: {
    docs: {
      description: {
        story: "When allowNone is false, the 'None' option is not shown.",
      },
    },
  },
};

function CustomLabelsStory() {
  const [value, setValue] = useState<string | null>(null);
  return (
    <TeamSelect
      teams={mockTeams}
      value={value}
      onChange={setValue}
      label="Assign to Team"
      nonePlaceholder="No team (project-level only)"
    />
  );
}

export const CustomLabels: Story = {
  render: () => <CustomLabelsStory />,
};

function DisabledStory() {
  const [value, setValue] = useState<string | null>("team-1");
  return <TeamSelect teams={mockTeams} value={value} onChange={setValue} disabled />;
}

export const Disabled: Story = {
  render: () => <DisabledStory />,
};

function EmptyTeamsStory() {
  const [value, setValue] = useState<string | null>(null);
  return <TeamSelect teams={[]} value={value} onChange={setValue} />;
}

export const EmptyTeams: Story = {
  render: () => <EmptyTeamsStory />,
  parameters: {
    docs: {
      description: {
        story: "When there are no teams, only the 'None' option is shown.",
      },
    },
  },
};
