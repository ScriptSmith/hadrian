import type { Meta, StoryObj } from "@storybook/react";
import { http, HttpResponse } from "msw";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { QuickCreateProjectModal } from "./QuickCreateProjectModal";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: false,
    },
  },
});

const mockOrganizations = {
  data: [
    {
      id: "org-1",
      name: "Acme Corp",
      slug: "acme-corp",
      created_at: "2024-01-01T00:00:00Z",
      updated_at: "2024-01-01T00:00:00Z",
    },
    {
      id: "org-2",
      name: "Test Organization",
      slug: "test-org",
      created_at: "2024-01-01T00:00:00Z",
      updated_at: "2024-01-01T00:00:00Z",
    },
  ],
  total: 2,
};

const meta: Meta<typeof QuickCreateProjectModal> = {
  title: "Components/QuickCreateProjectModal",
  component: QuickCreateProjectModal,
  decorators: [
    (Story) => (
      <QueryClientProvider client={queryClient}>
        <Story />
      </QueryClientProvider>
    ),
  ],
  parameters: {
    layout: "centered",
    msw: {
      handlers: [
        http.get("*/api/admin/v1/organizations", () => {
          return HttpResponse.json(mockOrganizations);
        }),
        http.get("*/api/admin/v1/organizations/*/projects", () => {
          return HttpResponse.json({ data: [], total: 0 });
        }),
        http.post("*/api/admin/v1/organizations/*/projects", async ({ request }) => {
          const body = (await request.json()) as { name: string; slug: string };
          return HttpResponse.json({
            id: "new-project-id",
            name: body.name,
            slug: body.slug,
            created_at: new Date().toISOString(),
            updated_at: new Date().toISOString(),
          });
        }),
      ],
    },
  },
  argTypes: {
    open: { control: "boolean" },
    onClose: { action: "onClose" },
    onCreated: { action: "onCreated" },
  },
};

export default meta;
type Story = StoryObj<typeof QuickCreateProjectModal>;

export const Default: Story = {
  args: {
    open: true,
    onClose: () => {},
    onCreated: () => {},
  },
};

export const Closed: Story = {
  args: {
    open: false,
    onClose: () => {},
    onCreated: () => {},
  },
};
