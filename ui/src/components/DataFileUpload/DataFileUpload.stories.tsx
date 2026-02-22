import type { Meta, StoryObj } from "@storybook/react";
import { useEffect } from "react";

import { DataFileUpload } from "./DataFileUpload";
import { useChatUIStore, type DataFile } from "@/stores/chatUIStore";

const meta: Meta<typeof DataFileUpload> = {
  title: "Components/DataFileUpload",
  component: DataFileUpload,
  parameters: {
    layout: "centered",
  },

  decorators: [
    (Story) => (
      <div className="w-80">
        <Story />
      </div>
    ),
  ],
};

export default meta;
type Story = StoryObj<typeof DataFileUpload>;

/** Helper to reset store between stories */
function StoreReset({ children }: { children: React.ReactNode }) {
  useEffect(() => {
    useChatUIStore.getState().clearDataFiles();
    return () => {
      useChatUIStore.getState().clearDataFiles();
    };
  }, []);
  return <>{children}</>;
}

/** Helper to set initial files */
function WithInitialFiles({ files, children }: { files: DataFile[]; children: React.ReactNode }) {
  useEffect(() => {
    files.forEach((file) => useChatUIStore.getState().addDataFile(file));
    return () => {
      useChatUIStore.getState().clearDataFiles();
    };
  }, [files]);
  return <>{children}</>;
}

export const Default: Story = {
  render: () => (
    <StoreReset>
      <DataFileUpload />
    </StoreReset>
  ),
};

export const Compact: Story = {
  render: () => (
    <StoreReset>
      <DataFileUpload compact />
    </StoreReset>
  ),
};

export const Disabled: Story = {
  render: () => (
    <StoreReset>
      <DataFileUpload disabled />
    </StoreReset>
  ),
};

const mockFiles: DataFile[] = [
  {
    id: "file-1",
    name: "sales_data.csv",
    type: "csv",
    size: 1024 * 50,
    uploadedAt: Date.now(),
    registered: true,
  },
  {
    id: "file-2",
    name: "analytics.parquet",
    type: "parquet",
    size: 1024 * 1024 * 2.5,
    uploadedAt: Date.now(),
    registered: true,
  },
];

export const WithFiles: Story = {
  render: () => (
    <WithInitialFiles files={mockFiles}>
      <DataFileUpload />
    </WithInitialFiles>
  ),
};

export const WithFilesCompact: Story = {
  render: () => (
    <WithInitialFiles files={mockFiles}>
      <DataFileUpload compact />
    </WithInitialFiles>
  ),
};

const mockFilesWithError: DataFile[] = [
  {
    id: "file-1",
    name: "valid_data.csv",
    type: "csv",
    size: 1024 * 100,
    uploadedAt: Date.now(),
    registered: true,
  },
  {
    id: "file-2",
    name: "corrupted.json",
    type: "json",
    size: 1024 * 1024,
    uploadedAt: Date.now(),
    registered: false,
    error: "Invalid JSON format",
  },
];

export const WithError: Story = {
  render: () => (
    <WithInitialFiles files={mockFilesWithError}>
      <DataFileUpload />
    </WithInitialFiles>
  ),
};

const mockLoadingFile: DataFile[] = [
  {
    id: "file-1",
    name: "large_dataset.parquet",
    type: "parquet",
    size: 1024 * 1024 * 50,
    uploadedAt: Date.now(),
    registered: false,
  },
];

export const Loading: Story = {
  render: () => (
    <WithInitialFiles files={mockLoadingFile}>
      <DataFileUpload />
    </WithInitialFiles>
  ),
};

const manyFiles: DataFile[] = [
  {
    id: "f1",
    name: "users.csv",
    type: "csv",
    size: 1024,
    uploadedAt: Date.now(),
    registered: true,
  },
  {
    id: "f2",
    name: "orders.csv",
    type: "csv",
    size: 2048,
    uploadedAt: Date.now(),
    registered: true,
  },
  {
    id: "f3",
    name: "products.json",
    type: "json",
    size: 512,
    uploadedAt: Date.now(),
    registered: true,
  },
  {
    id: "f4",
    name: "analytics.parquet",
    type: "parquet",
    size: 1024 * 1024,
    uploadedAt: Date.now(),
    registered: true,
  },
  {
    id: "f5",
    name: "metrics.parquet",
    type: "parquet",
    size: 1024 * 1024 * 5,
    uploadedAt: Date.now(),
    registered: true,
  },
];

export const ManyFiles: Story = {
  render: () => (
    <WithInitialFiles files={manyFiles}>
      <DataFileUpload compact />
    </WithInitialFiles>
  ),
};
