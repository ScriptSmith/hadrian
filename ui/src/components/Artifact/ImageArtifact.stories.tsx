import type { Meta, StoryObj } from "@storybook/react";
import { ImageArtifact } from "./ImageArtifact";
import type { Artifact } from "@/components/chat-types";

const meta = {
  title: "Chat/Artifacts/ImageArtifact",
  component: ImageArtifact,
  parameters: {
    layout: "padded",
  },
} satisfies Meta<typeof ImageArtifact>;

export default meta;
type Story = StoryObj<typeof meta>;

// A small placeholder SVG encoded as base64
const placeholderSvg =
  "data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iMjAwIiBoZWlnaHQ9IjE1MCIgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIj48cmVjdCB3aWR0aD0iMjAwIiBoZWlnaHQ9IjE1MCIgZmlsbD0iIzNiODJmNiIvPjx0ZXh0IHg9IjUwJSIgeT0iNTAlIiBkb21pbmFudC1iYXNlbGluZT0ibWlkZGxlIiB0ZXh0LWFuY2hvcj0ibWlkZGxlIiBmaWxsPSJ3aGl0ZSIgZm9udC1mYW1pbHk9InNhbnMtc2VyaWYiIGZvbnQtc2l6ZT0iMjAiPkltYWdlPC90ZXh0Pjwvc3ZnPg==";

const makeArtifact = (src: string, title?: string): Artifact => ({
  id: "img-1",
  type: "image",
  title: title || "Generated Image",
  data: { src },
  mimeType: "image/svg+xml",
});

export const Default: Story = {
  args: {
    artifact: makeArtifact(placeholderSvg, "Chart Output"),
  },
};

export const WithUrl: Story = {
  args: {
    artifact: {
      id: "img-url",
      type: "image",
      title: "External Image",
      data: { url: "https://via.placeholder.com/300x200/3b82f6/ffffff?text=Image" },
      mimeType: "image/png",
    },
  },
};

export const StringData: Story = {
  args: {
    artifact: {
      id: "img-str",
      type: "image",
      title: "Base64 String",
      data: placeholderSvg,
      mimeType: "image/svg+xml",
    },
  },
};
