import type { Meta, StoryObj } from "@storybook/react";
import { http, HttpResponse } from "msw";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { useEffect } from "react";
import { AgentButton } from "./AgentButton";
import { useChatUIStore } from "@/stores/chatUIStore";

const queryClient = new QueryClient({
  defaultOptions: { queries: { retry: false } },
});

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

/** Reset agent state between stories so toggles start predictable. */
function Reset({ enabled }: { enabled: boolean }) {
  useEffect(() => {
    useChatUIStore.getState().setAgentEnabled(enabled);
    useChatUIStore.getState().setAgentContainerMode("auto");
    useChatUIStore.getState().setToolSearchEnabled(false);
  }, [enabled]);
  return null;
}

const meta: Meta<typeof AgentButton> = {
  title: "Chat/AgentButton",
  component: AgentButton,
  decorators: [
    (Story) => (
      <QueryClientProvider client={queryClient}>
        <div className="p-8">
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
type Story = StoryObj<typeof AgentButton>;

export const Disabled: Story = {
  render: (args) => (
    <>
      <Reset enabled={false} />
      <AgentButton {...args} />
    </>
  ),
};

export const Enabled: Story = {
  render: (args) => (
    <>
      <Reset enabled={true} />
      <AgentButton {...args} />
    </>
  ),
};
