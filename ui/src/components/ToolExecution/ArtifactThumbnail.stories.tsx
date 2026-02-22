import type { Meta, StoryObj } from "@storybook/react";
import { ArtifactThumbnail } from "./ArtifactThumbnail";
import type { Artifact } from "@/components/chat-types";

const meta = {
  title: "Chat/ToolExecution/ArtifactThumbnail",
  component: ArtifactThumbnail,
  parameters: {
    layout: "centered",
  },
} satisfies Meta<typeof ArtifactThumbnail>;

export default meta;
type Story = StoryObj<typeof meta>;

const makeArtifact = (type: Artifact["type"], title?: string): Artifact => ({
  id: `artifact-${type}`,
  type,
  title,
  data: {},
});

export const Code: Story = {
  args: {
    artifact: makeArtifact("code", "main.py"),
    onClick: () => console.log("Clicked code artifact"),
  },
};

export const Table: Story = {
  args: {
    artifact: makeArtifact("table", "Query Results"),
    onClick: () => console.log("Clicked table artifact"),
  },
};

export const Chart: Story = {
  args: {
    artifact: makeArtifact("chart", "Sales Chart"),
    onClick: () => console.log("Clicked chart artifact"),
  },
};

export const Image: Story = {
  args: {
    artifact: makeArtifact("image", "Generated Plot"),
    onClick: () => console.log("Clicked image artifact"),
  },
};

export const Html: Story = {
  args: {
    artifact: makeArtifact("html", "Preview"),
    onClick: () => console.log("Clicked html artifact"),
  },
};

export const Agent: Story = {
  args: {
    artifact: makeArtifact("agent", "Research Task"),
    onClick: () => console.log("Clicked agent artifact"),
  },
};

export const FileSearch: Story = {
  args: {
    artifact: makeArtifact("file_search", "Search Results"),
    onClick: () => console.log("Clicked file_search artifact"),
  },
};

export const NoTitle: Story = {
  args: {
    artifact: makeArtifact("code"),
    onClick: () => console.log("Clicked"),
  },
};
