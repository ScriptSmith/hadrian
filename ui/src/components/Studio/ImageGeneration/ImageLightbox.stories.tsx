import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react";
import { ImageLightbox, type LightboxImage } from "./ImageLightbox";

const sampleImage =
  "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='600' height='600' viewBox='0 0 600 600'%3E%3Crect fill='%23334155' width='600' height='600'/%3E%3Ctext fill='%2394a3b8' font-family='sans-serif' font-size='20' x='50%25' y='50%25' text-anchor='middle' dominant-baseline='middle'%3EGenerated Image%3C/text%3E%3C/svg%3E";

const sampleImages: LightboxImage[] = [
  {
    imageData: sampleImage,
    prompt: "A sunset over mountains",
    revisedPrompt:
      "A breathtaking sunset with vibrant orange and pink hues illuminating snow-capped mountain peaks against a dramatic sky with wispy clouds stretching across the horizon",
    modelLabel: "DALL-E 3",
  },
  {
    imageData: sampleImage,
    prompt: "A futuristic city",
    modelLabel: "GPT Image 1",
  },
  {
    imageData: sampleImage,
    prompt: "An underwater scene",
    revisedPrompt: "A vivid underwater coral reef scene with tropical fish",
    modelLabel: "DALL-E 3",
  },
];

function LightboxWrapper({ images }: { images: LightboxImage[] }) {
  const [index, setIndex] = useState(0);
  return (
    <ImageLightbox images={images} currentIndex={index} onClose={() => {}} onNavigate={setIndex} />
  );
}

const meta = {
  title: "Studio/ImageLightbox",
  component: LightboxWrapper,
  parameters: {
    layout: "fullscreen",
  },
} satisfies Meta<typeof LightboxWrapper>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  args: {
    images: sampleImages,
  },
};

export const SingleImage: Story = {
  args: {
    images: [sampleImages[0]],
  },
};
