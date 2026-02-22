import type { Meta, StoryObj } from "@storybook/react";
import { useState } from "react";
import { ImageOptionsForm, type ImageOptions } from "./ImageOptionsForm";

const meta = {
  title: "Studio/ImageOptionsForm",
  component: ImageOptionsForm,
  parameters: {
    layout: "centered",
  },
  decorators: [
    (Story) => (
      <div className="w-[350px]">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof ImageOptionsForm>;

export default meta;
type Story = StoryObj<typeof ImageOptionsForm>;

const defaultOptions: ImageOptions = {
  n: 1,
  size: "1024x1024",
  quality: "auto",
  style: "vivid",
  outputFormat: "png",
  background: "auto",
};

function DefaultStory() {
  const [options, setOptions] = useState<ImageOptions>(defaultOptions);
  return <ImageOptionsForm options={options} onChange={setOptions} />;
}

export const Default: Story = {
  render: () => <DefaultStory />,
};

function MultipleCountStory() {
  const [options, setOptions] = useState<ImageOptions>({
    ...defaultOptions,
    n: 2,
  });
  return <ImageOptionsForm options={options} onChange={setOptions} />;
}

export const MultipleCount: Story = {
  render: () => <MultipleCountStory />,
};
