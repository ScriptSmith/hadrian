import type { Meta, StoryObj } from "@storybook/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { http, HttpResponse, delay } from "msw";

import { MoveToProjectModal } from "./MoveToProjectModal";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: false,
      staleTime: Infinity,
    },
  },
});

const mockOrganizations = [
  {
    id: "org-1",
    name: "Acme Corp",
    slug: "acme-corp",
    created_at: "2024-01-01T00:00:00Z",
    updated_at: "2024-01-01T00:00:00Z",
  },
  {
    id: "org-2",
    name: "Personal Org",
    slug: "personal",
    created_at: "2024-01-01T00:00:00Z",
    updated_at: "2024-01-01T00:00:00Z",
  },
];

const mockProjects = [
  {
    id: "proj-1",
    name: "Frontend App",
    slug: "frontend-app",
    org_id: "org-1",
    created_at: "2024-01-01T00:00:00Z",
    updated_at: "2024-01-01T00:00:00Z",
  },
  {
    id: "proj-2",
    name: "Backend API",
    slug: "backend-api",
    org_id: "org-1",
    created_at: "2024-01-01T00:00:00Z",
    updated_at: "2024-01-01T00:00:00Z",
  },
  {
    id: "proj-3",
    name: "Side Project",
    slug: "side-project",
    org_id: "org-2",
    created_at: "2024-01-01T00:00:00Z",
    updated_at: "2024-01-01T00:00:00Z",
  },
];

const handlers = [
  http.get("/api/admin/v1/organizations", () => {
    return HttpResponse.json({
      data: mockOrganizations,
      pagination: { limit: 100, has_more: false },
    });
  }),
  http.get("/api/admin/v1/organizations/:orgSlug/projects", ({ params }) => {
    const projects = mockProjects.filter((p) => {
      const org = mockOrganizations.find((o) => o.slug === params.orgSlug);
      return org && p.org_id === org.id;
    });
    return HttpResponse.json({
      data: projects,
      pagination: { limit: 100, has_more: false },
    });
  }),
];

const meta = {
  title: "Components/MoveToProjectModal",
  component: MoveToProjectModal,
  parameters: {
    msw: { handlers },
  },
  decorators: [
    (Story) => (
      <QueryClientProvider client={queryClient}>
        <Story />
      </QueryClientProvider>
    ),
  ],
  argTypes: {
    onClose: { action: "onClose" },
    onMove: { action: "onMove" },
  },
  args: {
    open: true,
    onClose: () => {},
    onMove: async () => {
      await delay(500);
    },
  },
} satisfies Meta<typeof MoveToProjectModal>;

export default meta;
type Story = StoryObj<typeof meta>;

export const MoveFromPersonalToProject: Story = {
  args: {
    conversation: {
      id: "conv-1",
      title: "Discussion about API design",
      projectId: undefined,
    },
  },
};

export const MoveFromProjectToPersonal: Story = {
  args: {
    conversation: {
      id: "conv-2",
      title: "Sprint planning notes",
      projectId: "proj-1",
    },
  },
};

export const MoveFromProjectToAnotherProject: Story = {
  args: {
    conversation: {
      id: "conv-3",
      title: "Code review feedback",
      projectId: "proj-2",
    },
  },
};

export const LongConversationTitle: Story = {
  args: {
    conversation: {
      id: "conv-4",
      title:
        "This is a very long conversation title that might need to be truncated or wrapped in the modal display",
      projectId: undefined,
    },
  },
};
