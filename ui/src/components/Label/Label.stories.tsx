import type { Meta, StoryObj } from "@storybook/react";
import { Label } from "./Label";

const meta: Meta<typeof Label> = {
  title: "Components/Label",
  component: Label,
  parameters: {
    layout: "centered",
  },

  decorators: [
    (Story) => (
      <div className="p-4">
        <Story />
      </div>
    ),
  ],
};

export default meta;
type Story = StoryObj<typeof Label>;

/**
 * Default label for form inputs.
 */
export const Default: Story = {
  args: {
    children: "Email address",
  },
};

/**
 * Label with an associated input element.
 */
export const WithInput: Story = {
  render: () => (
    <div className="space-y-2">
      <Label htmlFor="email">Email address</Label>
      <input
        id="email"
        type="email"
        className="border rounded px-3 py-1 text-sm"
        placeholder="you@example.com"
      />
    </div>
  ),
};

/**
 * Required field indicator pattern.
 */
export const Required: Story = {
  render: () => (
    <div className="space-y-2">
      <Label htmlFor="name">
        Full name <span className="text-destructive">*</span>
      </Label>
      <input id="name" type="text" className="border rounded px-3 py-1 text-sm" />
    </div>
  ),
};

/**
 * Disabled label styling.
 */
export const WithDisabledInput: Story = {
  render: () => (
    <div className="space-y-2">
      <Label htmlFor="disabled" className="peer-disabled:opacity-70">
        Disabled field
      </Label>
      <input
        id="disabled"
        type="text"
        className="peer border rounded px-3 py-1 text-sm"
        disabled
        placeholder="Cannot edit"
      />
    </div>
  ),
};
