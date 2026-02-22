import type { Meta, StoryObj } from "@storybook/react";
import { Textarea } from "./Textarea";

const meta: Meta<typeof Textarea> = {
  title: "UI/Textarea",
  component: Textarea,
  parameters: {
    layout: "centered",
  },

  argTypes: {
    error: {
      control: "boolean",
    },
    autoResize: {
      control: "boolean",
    },
    maxHeight: {
      control: "number",
    },
    disabled: {
      control: "boolean",
    },
  },
};

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  args: {
    placeholder: "Enter your message...",
  },
};

export const WithValue: Story = {
  args: {
    value: "This is some text content in the textarea.",
    "aria-label": "Example textarea",
  },
};

export const Error: Story = {
  args: {
    placeholder: "Enter your message...",
    error: true,
  },
};

export const AutoResize: Story = {
  args: {
    placeholder: "This textarea will grow as you type...",
    autoResize: true,
    maxHeight: 200,
  },
};

export const Disabled: Story = {
  args: {
    value: "This textarea is disabled",
    disabled: true,
    "aria-label": "Disabled textarea",
  },
};

export const WithMaxHeight: Story = {
  args: {
    placeholder: "Limited height textarea...",
    autoResize: true,
    maxHeight: 100,
  },
};
