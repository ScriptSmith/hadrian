import type { Meta, StoryObj } from "@storybook/react";
import { useState, useCallback } from "react";
import { FileUpload } from "./FileUpload";
import type { ChatFile } from "../chat-types";
import { ConfigProvider } from "@/config/ConfigProvider";

const meta: Meta<typeof FileUpload> = {
  title: "Chat/FileUpload",
  component: FileUpload,
  parameters: {
    layout: "centered",
  },

  decorators: [
    (Story) => (
      <ConfigProvider>
        <Story />
      </ConfigProvider>
    ),
  ],
};

export default meta;
type Story = StoryObj<typeof meta>;

function DefaultStory() {
  const [files, setFiles] = useState<ChatFile[]>([]);
  const handleFilesChange = useCallback((newFiles: ChatFile[]) => {
    setFiles(newFiles);
  }, []);

  return (
    <div className="w-96">
      <FileUpload files={files} onFilesChange={handleFilesChange} />
      {files.length > 0 && (
        <div className="mt-4 text-sm text-muted-foreground">{files.length} file(s) uploaded</div>
      )}
    </div>
  );
}

export const Default: Story = {
  render: () => <DefaultStory />,
};

function WithFilesStory() {
  const [files, setFiles] = useState<ChatFile[]>([
    {
      id: "1",
      name: "document.pdf",
      type: "application/pdf",
      size: 1024000,
      base64: "",
    },
    {
      id: "2",
      name: "image.png",
      type: "image/png",
      size: 256000,
      base64: "",
      preview:
        "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==",
    },
  ]);
  const handleFilesChange = useCallback((newFiles: ChatFile[]) => {
    setFiles(newFiles);
  }, []);

  return (
    <div className="w-96">
      <FileUpload files={files} onFilesChange={handleFilesChange} />
    </div>
  );
}

export const WithFiles: Story = {
  render: () => <WithFilesStory />,
};

function DisabledStory() {
  const [files, setFiles] = useState<ChatFile[]>([]);
  const handleFilesChange = useCallback((newFiles: ChatFile[]) => {
    setFiles(newFiles);
  }, []);

  return (
    <div className="w-96">
      <FileUpload files={files} onFilesChange={handleFilesChange} disabled />
    </div>
  );
}

export const Disabled: Story = {
  render: () => <DisabledStory />,
};
