import type { Meta, StoryObj } from "@storybook/react";
import { useState } from "react";

import { OrganizationSelect } from "./OrganizationSelect";

const meta: Meta<typeof OrganizationSelect> = {
  title: "Admin/OrganizationSelect",
  component: OrganizationSelect,
  parameters: {
    layout: "padded",
  },
};

export default meta;
type Story = StoryObj<typeof OrganizationSelect>;

const mockOrganizations = [
  {
    id: "1",
    slug: "acme-corp",
    name: "Acme Corp",
    created_at: "2024-01-01",
    updated_at: "2024-01-01",
  },
  {
    id: "2",
    slug: "globex",
    name: "Globex Corporation",
    created_at: "2024-01-02",
    updated_at: "2024-01-02",
  },
  { id: "3", slug: "initech", name: "Initech", created_at: "2024-01-03", updated_at: "2024-01-03" },
];

function DefaultStory() {
  const [value, setValue] = useState<string | null>(null);
  return <OrganizationSelect organizations={mockOrganizations} value={value} onChange={setValue} />;
}

export const Default: Story = {
  render: () => <DefaultStory />,
};

function CustomLabelStory() {
  const [value, setValue] = useState<string | null>(null);
  return (
    <OrganizationSelect
      organizations={mockOrganizations}
      value={value}
      onChange={setValue}
      label="Organization"
    />
  );
}

export const CustomLabel: Story = {
  render: () => <CustomLabelStory />,
};

function SingleOrganizationStory() {
  const [value, setValue] = useState<string | null>(null);
  return (
    <OrganizationSelect organizations={[mockOrganizations[0]]} value={value} onChange={setValue} />
  );
}

export const SingleOrganization: Story = {
  render: () => <SingleOrganizationStory />,
  parameters: {
    docs: {
      description: {
        story: "When there is only one organization, the component renders nothing.",
      },
    },
  },
};
