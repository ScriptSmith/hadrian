import type { Meta, StoryObj } from "@storybook/react";
import { useState } from "react";

import type { TemplateVariable } from "@/lib/templateVariables";
import { TemplateVariableEditor } from "./TemplateVariableEditor";

function TemplateVariableEditorDemo({ initial = [] }: { initial?: TemplateVariable[] }) {
  const [variables, setVariables] = useState<TemplateVariable[]>(initial);
  return <TemplateVariableEditor variables={variables} onChange={setVariables} />;
}

const meta: Meta<typeof TemplateVariableEditor> = {
  title: "Components/TemplateVariableEditor",
  component: TemplateVariableEditor,
  parameters: {
    layout: "padded",
  },
};

export default meta;
type Story = StoryObj<typeof meta>;

export const Empty: Story = {
  render: () => <TemplateVariableEditorDemo />,
};

export const WithVariables: Story = {
  render: () => (
    <TemplateVariableEditorDemo
      initial={[
        {
          name: "language",
          label: "Language",
          type: "select",
          options: ["Python", "TypeScript", "Go"],
          required: true,
        },
        {
          name: "context",
          label: "Additional Context",
          type: "textarea",
          placeholder: "Describe your use case...",
        },
      ]}
    />
  ),
};

export const SingleTextVariable: Story = {
  render: () => (
    <TemplateVariableEditorDemo
      initial={[
        {
          name: "name",
          label: "Project Name",
          type: "text",
          required: true,
          placeholder: "my-project",
        },
      ]}
    />
  ),
};
