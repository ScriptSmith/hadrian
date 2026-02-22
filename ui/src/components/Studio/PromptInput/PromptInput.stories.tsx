import type { Meta, StoryObj } from "@storybook/react";
import { fn } from "storybook/test";
import { useState } from "react";
import { PromptInput } from "./PromptInput";

const meta = {
  title: "Studio/PromptInput",
  component: PromptInput,
  parameters: {
    layout: "centered",
  },
  decorators: [
    (Story) => (
      <div className="w-[500px]">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof PromptInput>;

export default meta;
type Story = StoryObj<typeof PromptInput>;

function DefaultStory() {
  const [value, setValue] = useState("");
  return (
    <PromptInput
      value={value}
      onChange={setValue}
      onSubmit={fn()}
      placeholder="Enter your prompt..."
    />
  );
}

export const Default: Story = {
  render: () => <DefaultStory />,
};

function WithTextStory() {
  const [value, setValue] = useState("A beautiful sunset over snow-capped mountains");
  return (
    <PromptInput
      value={value}
      onChange={setValue}
      onSubmit={fn()}
      placeholder="Enter your prompt..."
    />
  );
}

export const WithText: Story = {
  render: () => <WithTextStory />,
};

function WithMaxLengthStory() {
  const [value, setValue] = useState("This prompt has a character limit of 200 characters");
  return (
    <PromptInput
      value={value}
      onChange={setValue}
      onSubmit={fn()}
      placeholder="Enter your prompt..."
      maxLength={200}
    />
  );
}

export const WithMaxLength: Story = {
  render: () => <WithMaxLengthStory />,
};

function DisabledStory() {
  const [value, setValue] = useState("This input is disabled");
  return (
    <PromptInput
      value={value}
      onChange={setValue}
      onSubmit={fn()}
      placeholder="Enter your prompt..."
      disabled
    />
  );
}

export const Disabled: Story = {
  render: () => <DisabledStory />,
};
