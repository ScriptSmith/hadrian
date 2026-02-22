import type { Meta, StoryObj } from "@storybook/react";

import { Input } from "@/components/Input/Input";
import { FormField } from "./FormField";

const meta: Meta<typeof FormField> = {
  title: "Components/FormField",
  component: FormField,
  parameters: {
    layout: "padded",
  },
};

export default meta;
type Story = StoryObj<typeof FormField>;

export const Default: Story = {
  args: {
    label: "Email",
    htmlFor: "email",
    children: <Input id="email" type="email" placeholder="john@example.com" />,
  },
};

export const WithHelpText: Story = {
  args: {
    label: "Slug",
    htmlFor: "slug",
    helpText: "Used in URLs and API paths",
    children: <Input id="slug" placeholder="my-project" />,
  },
};

export const Required: Story = {
  args: {
    label: "Name",
    htmlFor: "name",
    required: true,
    children: <Input id="name" placeholder="Enter name" />,
  },
};

export const WithError: Story = {
  args: {
    label: "Email",
    htmlFor: "email",
    error: "Please enter a valid email address",
    children: <Input id="email" type="email" placeholder="john@example.com" />,
  },
};

export const RequiredWithHelpText: Story = {
  args: {
    label: "API Key",
    htmlFor: "api-key",
    required: true,
    helpText: "Your API key will only be shown once",
    children: <Input id="api-key" placeholder="sk-..." />,
  },
};

export const FormExample: Story = {
  render: () => (
    <div className="max-w-md space-y-4">
      <FormField label="Name" htmlFor="name" required>
        <Input id="name" placeholder="My Organization" />
      </FormField>
      <FormField label="Slug" htmlFor="slug" required helpText="Used in URLs and API paths">
        <Input id="slug" placeholder="my-organization" />
      </FormField>
      <FormField label="Description" htmlFor="description">
        <Input id="description" placeholder="Optional description" />
      </FormField>
    </div>
  ),
};
