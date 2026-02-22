import type { Meta, StoryObj } from "@storybook/react";
import { fn } from "storybook/test";
import { ImageGallery } from "./ImageGallery";
import type { ImageHistoryEntry } from "@/pages/studio/types";

const meta = {
  title: "Studio/ImageGallery",
  component: ImageGallery,
  parameters: {
    layout: "centered",
  },
  decorators: [
    (Story) => (
      <div className="w-[700px]">
        <Story />
      </div>
    ),
  ],
  args: {
    onDelete: fn(),
  },
} satisfies Meta<typeof ImageGallery>;

export default meta;
type Story = StoryObj<typeof meta>;

const createSampleImage = (color: string, label: string) =>
  `data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='400' height='400' viewBox='0 0 400 400'%3E%3Crect fill='%23${color}' width='400' height='400'/%3E%3Ctext fill='%23ffffff' font-family='sans-serif' font-size='16' x='50%25' y='50%25' text-anchor='middle' dominant-baseline='middle'%3E${label}%3C/text%3E%3C/svg%3E`;

const sampleEntries: ImageHistoryEntry[] = [
  {
    id: "img-1",
    prompt: "A beautiful sunset over mountains",
    options: { size: "1024x1024", quality: "hd", style: "vivid", outputFormat: "png", n: 1 },
    results: [
      {
        instanceId: "dall-e-3",
        modelId: "dall-e-3",
        images: [
          {
            imageData: createSampleImage("334155", "Image 1"),
            revisedPrompt: "A breathtaking sunset with vibrant colors",
          },
        ],
      },
    ],
    createdAt: Date.now(),
  },
  {
    id: "img-2",
    prompt: "A futuristic city at night",
    options: { size: "1024x1024", quality: "standard" },
    results: [
      {
        instanceId: "dall-e-3",
        modelId: "dall-e-3",
        images: [{ imageData: createSampleImage("1e293b", "Image 2") }],
      },
    ],
    createdAt: Date.now() - 60000,
  },
];

export const Empty: Story = {
  args: {
    entries: [],
  },
};

export const WithImages: Story = {
  args: {
    entries: sampleEntries,
  },
};

export const MultiModel: Story = {
  args: {
    entries: [
      {
        id: "img-multi",
        prompt: "A cat in a spacesuit",
        options: { size: "1024x1024", n: 1 },
        results: [
          {
            instanceId: "dall-e-3",
            modelId: "dall-e-3",
            images: [{ imageData: createSampleImage("334155", "DALL-E 3") }],
            costMicrocents: 40000,
          },
          {
            instanceId: "gpt-image-1",
            modelId: "gpt-image-1",
            label: "GPT Image",
            images: [{ imageData: createSampleImage("1e293b", "GPT Image") }],
            costMicrocents: 170000,
          },
        ],
        createdAt: Date.now(),
      },
    ],
  },
};
