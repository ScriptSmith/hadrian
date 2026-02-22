import type { Meta, StoryObj } from "@storybook/react";
import { ImageCard } from "./ImageCard";

const meta = {
  title: "Studio/ImageCard",
  component: ImageCard,
  parameters: {
    layout: "centered",
  },
  decorators: [
    (Story) => (
      <div className="w-[280px]">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof ImageCard>;

export default meta;
type Story = StoryObj<typeof meta>;

const sampleImage =
  "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='400' height='400' viewBox='0 0 400 400'%3E%3Crect fill='%23334155' width='400' height='400'/%3E%3Ctext fill='%2394a3b8' font-family='sans-serif' font-size='16' x='50%25' y='50%25' text-anchor='middle' dominant-baseline='middle'%3EGenerated Image%3C/text%3E%3C/svg%3E";

export const Default: Story = {
  args: {
    imageData: sampleImage,
    prompt: "A beautiful sunset over snow-capped mountains",
    createdAt: Date.now(),
  },
};

export const WithRevisedPrompt: Story = {
  args: {
    imageData: sampleImage,
    prompt: "A beautiful sunset over snow-capped mountains",
    revisedPrompt:
      "A breathtaking sunset with vibrant orange and pink hues illuminating snow-capped mountain peaks against a dramatic sky",
    createdAt: Date.now(),
  },
};
