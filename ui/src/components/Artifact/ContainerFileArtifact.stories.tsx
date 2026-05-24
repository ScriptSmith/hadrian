import type { Meta, StoryObj } from "@storybook/react";
import { http, HttpResponse } from "msw";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { ContainerFileArtifact } from "./ContainerFileArtifact";
import { AuthProvider } from "@/auth";
import type { Artifact } from "@/components/chat-types";

const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });

// 1x1 transparent PNG.
const PNG_BYTES = Uint8Array.from(
  atob(
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg=="
  ),
  (c) => c.charCodeAt(0)
);

const imageArtifact: Artifact = {
  id: "cfile-1",
  type: "container_file",
  role: "output",
  title: "chart.png",
  data: {
    containerId: "cntr_abc",
    fileId: "cfile_1",
    filename: "chart.png",
    contentType: "image/png",
    bytes: 1024,
  },
};

const fileArtifact: Artifact = {
  id: "cfile-2",
  type: "container_file",
  role: "output",
  title: "results.csv",
  data: {
    containerId: "cntr_abc",
    fileId: "cfile_2",
    filename: "results.csv",
    contentType: "text/csv",
    bytes: 20480,
  },
};

const meta: Meta<typeof ContainerFileArtifact> = {
  title: "Artifact/ContainerFileArtifact",
  component: ContainerFileArtifact,
  decorators: [
    (Story) => (
      <QueryClientProvider client={queryClient}>
        <AuthProvider>
          <div className="max-w-md rounded-lg border">
            <Story />
          </div>
        </AuthProvider>
      </QueryClientProvider>
    ),
  ],
  parameters: {
    msw: {
      handlers: [
        http.get("*/files/cfile_1/content", () =>
          HttpResponse.arrayBuffer(PNG_BYTES.buffer, {
            headers: { "Content-Type": "image/png" },
          })
        ),
        http.get("*/files/cfile_2/content", () => HttpResponse.text("a,b\n1,2\n")),
      ],
    },
  },
};

export default meta;
type Story = StoryObj<typeof ContainerFileArtifact>;

export const ImageInline: Story = { args: { artifact: imageArtifact } };
export const FileDownload: Story = { args: { artifact: fileArtifact } };
