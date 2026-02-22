import type { Meta, StoryObj } from "@storybook/react";
import { useState } from "react";
import { AudioDropZone } from "./AudioDropZone";

const meta = {
  title: "Studio/AudioDropZone",
  component: AudioDropZone,
  parameters: {
    layout: "centered",
  },
  decorators: [
    (Story) => (
      <div className="w-[400px]">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof AudioDropZone>;

export default meta;
type Story = StoryObj<typeof AudioDropZone>;

function EmptyStory() {
  const [file, setFile] = useState<File | null>(null);
  return <AudioDropZone file={file} onFileChange={setFile} />;
}

export const Empty: Story = {
  render: () => <EmptyStory />,
};

function WithFileStory() {
  const [file, setFile] = useState<File | null>(
    new File([""], "recording.mp3", { type: "audio/mp3" })
  );
  return <AudioDropZone file={file} onFileChange={setFile} />;
}

export const WithFile: Story = {
  render: () => <WithFileStory />,
};

function DisabledStory() {
  const [file, setFile] = useState<File | null>(null);
  return <AudioDropZone file={file} onFileChange={setFile} disabled />;
}

export const Disabled: Story = {
  render: () => <DisabledStory />,
};
