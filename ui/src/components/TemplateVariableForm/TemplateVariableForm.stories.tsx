import type { Meta, StoryObj } from "@storybook/react";
import { useState } from "react";

import type { TemplateVariable } from "@/lib/templateVariables";
import { TemplateVariableForm } from "./TemplateVariableForm";

function TemplateVariableFormDemo({
  variables,
  errors,
}: {
  variables: TemplateVariable[];
  errors?: Record<string, string>;
}) {
  const [values, setValues] = useState<Record<string, string>>({});
  return (
    <TemplateVariableForm
      variables={variables}
      values={values}
      onChange={setValues}
      errors={errors}
    />
  );
}

const meta: Meta<typeof TemplateVariableForm> = {
  title: "Components/TemplateVariableForm",
  component: TemplateVariableForm,
  parameters: {
    layout: "padded",
  },
};

export default meta;
type Story = StoryObj<typeof meta>;

export const TextInputs: Story = {
  render: () => (
    <TemplateVariableFormDemo
      variables={[
        {
          name: "name",
          label: "Project Name",
          type: "text",
          required: true,
          placeholder: "my-project",
        },
        {
          name: "description",
          label: "Description",
          type: "text",
          placeholder: "A brief description",
        },
      ]}
    />
  ),
};

export const MixedTypes: Story = {
  render: () => (
    <TemplateVariableFormDemo
      variables={[
        {
          name: "language",
          label: "Language",
          type: "select",
          options: ["Python", "TypeScript", "Go", "Rust"],
          required: true,
        },
        {
          name: "context",
          label: "Additional Context",
          type: "textarea",
          placeholder: "Describe your use case...",
        },
        { name: "author", label: "Author", type: "text", default: "Anonymous" },
      ]}
    />
  ),
};

export const WithErrors: Story = {
  render: () => (
    <TemplateVariableFormDemo
      variables={[
        { name: "name", label: "Project Name", type: "text", required: true },
        {
          name: "language",
          label: "Language",
          type: "select",
          options: ["Python", "TypeScript"],
          required: true,
        },
      ]}
      errors={{ name: "Project Name is required", language: "Language is required" }}
    />
  ),
};
