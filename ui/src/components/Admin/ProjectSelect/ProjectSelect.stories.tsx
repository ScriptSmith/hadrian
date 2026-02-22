import type { Meta, StoryObj } from "@storybook/react";
import { useState } from "react";

import { ProjectSelect } from "./ProjectSelect";

const meta: Meta<typeof ProjectSelect> = {
  title: "Admin/ProjectSelect",
  component: ProjectSelect,
  parameters: {
    layout: "padded",
  },
};

export default meta;
type Story = StoryObj<typeof ProjectSelect>;

const mockProjects = [
  {
    id: "proj-1",
    org_id: "org-1",
    slug: "production-api",
    name: "Production API",
    created_at: "2024-01-01T00:00:00Z",
    updated_at: "2024-01-01T00:00:00Z",
  },
  {
    id: "proj-2",
    org_id: "org-1",
    slug: "staging-api",
    name: "Staging API",
    created_at: "2024-01-02T00:00:00Z",
    updated_at: "2024-01-02T00:00:00Z",
  },
  {
    id: "proj-3",
    org_id: "org-1",
    slug: "internal-tools",
    name: "Internal Tools",
    created_at: "2024-01-03T00:00:00Z",
    updated_at: "2024-01-03T00:00:00Z",
  },
];

function DefaultStory() {
  const [value, setValue] = useState<string | null>(null);
  return <ProjectSelect projects={mockProjects} value={value} onChange={setValue} />;
}

export const Default: Story = {
  render: () => <DefaultStory />,
};

function WithSelectedProjectStory() {
  const [value, setValue] = useState<string | null>("staging-api");
  return <ProjectSelect projects={mockProjects} value={value} onChange={setValue} />;
}

export const WithSelectedProject: Story = {
  render: () => <WithSelectedProjectStory />,
};

function NoNoneOptionStory() {
  const [value, setValue] = useState<string | null>("production-api");
  return (
    <ProjectSelect projects={mockProjects} value={value} onChange={setValue} allowNone={false} />
  );
}

export const NoNoneOption: Story = {
  render: () => <NoNoneOptionStory />,
  parameters: {
    docs: {
      description: {
        story: "When allowNone is false, the 'All projects' option is not shown.",
      },
    },
  },
};

function CustomLabelsStory() {
  const [value, setValue] = useState<string | null>(null);
  return (
    <ProjectSelect
      projects={mockProjects}
      value={value}
      onChange={setValue}
      label="Filter by Project"
      nonePlaceholder="No project filter"
    />
  );
}

export const CustomLabels: Story = {
  render: () => <CustomLabelsStory />,
};

function DisabledStory() {
  const [value, setValue] = useState<string | null>("production-api");
  return <ProjectSelect projects={mockProjects} value={value} onChange={setValue} disabled />;
}

export const Disabled: Story = {
  render: () => <DisabledStory />,
};

function EmptyProjectsStory() {
  const [value, setValue] = useState<string | null>(null);
  return <ProjectSelect projects={[]} value={value} onChange={setValue} />;
}

export const EmptyProjects: Story = {
  render: () => <EmptyProjectsStory />,
  parameters: {
    docs: {
      description: {
        story: "When there are no projects, only the 'All projects' option is shown.",
      },
    },
  },
};
