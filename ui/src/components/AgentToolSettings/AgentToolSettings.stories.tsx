import type { Meta, StoryObj } from "@storybook/react";
import { http, HttpResponse } from "msw";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { useEffect } from "react";
import { AgentToolSettings } from "./AgentToolSettings";
import { useChatUIStore } from "@/stores/chatUIStore";

const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });

const now = Math.floor(Date.now() / 1000);
const mockContainers = {
  object: "list",
  data: [
    {
      id: "cntr_a1b2c3",
      object: "container",
      status: "active",
      created_at: now - 600,
      last_active_at: now - 60,
      expires_at: now + 1140,
      idle_ttl_secs: 1200,
      runtime: "microsandbox",
      name: "data-analysis",
    },
  ],
  has_more: false,
};

function Reset({ mode }: { mode: "auto" | "reference" }) {
  useEffect(() => {
    useChatUIStore.getState().setAgentContainerMode(mode);
  }, [mode]);
  return null;
}

const meta: Meta<typeof AgentToolSettings> = {
  title: "Chat/AgentToolSettings",
  component: AgentToolSettings,
  decorators: [
    (Story) => (
      <QueryClientProvider client={queryClient}>
        <div className="p-4 max-w-sm">
          <Story />
        </div>
      </QueryClientProvider>
    ),
  ],
  parameters: {
    msw: {
      handlers: [http.get("*/v1/containers", () => HttpResponse.json(mockContainers))],
    },
  },
};

export default meta;
type Story = StoryObj<typeof AgentToolSettings>;

export const NewContainer: Story = {
  render: (args) => (
    <>
      <Reset mode="auto" />
      <AgentToolSettings {...args} />
    </>
  ),
};

export const AttachExisting: Story = {
  render: (args) => (
    <>
      <Reset mode="reference" />
      <AgentToolSettings {...args} />
    </>
  ),
};
