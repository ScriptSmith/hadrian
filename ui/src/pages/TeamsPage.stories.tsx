import type { Meta, StoryObj } from "@storybook/react";
import { http, HttpResponse } from "msw";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter } from "react-router-dom";
import TeamsPage from "./TeamsPage";

const mockOrganizations = {
  data: [
    {
      id: "org-1",
      name: "Acme Corp",
      slug: "acme-corp",
      created_at: "2024-01-01T00:00:00Z",
      updated_at: "2024-01-01T00:00:00Z",
    },
  ],
  total: 1,
};

const mockTeamsAcme = {
  data: [
    {
      id: "team-1",
      name: "Engineering",
      slug: "engineering",
      org_id: "org-1",
      created_at: "2024-01-15T00:00:00Z",
      updated_at: "2024-02-01T00:00:00Z",
    },
    {
      id: "team-2",
      name: "Data Science",
      slug: "data-science",
      org_id: "org-1",
      created_at: "2024-02-01T00:00:00Z",
      updated_at: "2024-02-15T00:00:00Z",
    },
    {
      id: "team-3",
      name: "Product",
      slug: "product",
      org_id: "org-1",
      created_at: "2024-03-01T00:00:00Z",
      updated_at: "2024-03-10T00:00:00Z",
    },
  ],
  total: 3,
};

const mockProjectsAcme = {
  data: [
    {
      id: "proj-1",
      name: "Main Backend",
      slug: "main-backend",
      team_id: "team-1",
      created_at: "2024-01-15T00:00:00Z",
      updated_at: "2024-02-01T00:00:00Z",
    },
    {
      id: "proj-2",
      name: "Mobile App",
      slug: "mobile-app",
      team_id: "team-1",
      created_at: "2024-02-01T00:00:00Z",
      updated_at: "2024-02-15T00:00:00Z",
    },
    {
      id: "proj-3",
      name: "Data Pipeline",
      slug: "data-pipeline",
      team_id: "team-2",
      created_at: "2024-03-01T00:00:00Z",
      updated_at: "2024-03-10T00:00:00Z",
    },
  ],
  total: 3,
};

const defaultHandlers = [
  http.get("*/api/admin/v1/organizations", () => {
    return HttpResponse.json(mockOrganizations);
  }),
  http.get("*/api/admin/v1/organizations/acme-corp/teams", () => {
    return HttpResponse.json(mockTeamsAcme);
  }),
  http.get("*/api/admin/v1/organizations/acme-corp/projects", () => {
    return HttpResponse.json(mockProjectsAcme);
  }),
];

const meta: Meta<typeof TeamsPage> = {
  title: "Pages/TeamsPage",
  component: TeamsPage,
  decorators: [
    (Story) => {
      const queryClient = new QueryClient({
        defaultOptions: { queries: { retry: false } },
      });
      return (
        <QueryClientProvider client={queryClient}>
          <MemoryRouter>
            <Story />
          </MemoryRouter>
        </QueryClientProvider>
      );
    },
  ],
  parameters: {
    layout: "fullscreen",
    msw: {
      handlers: defaultHandlers,
    },
  },
};

export default meta;
type Story = StoryObj<typeof TeamsPage>;

export const Default: Story = {};

export const Loading: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/api/admin/v1/organizations", async () => {
          await new Promise((resolve) => setTimeout(resolve, 999999));
          return HttpResponse.json(mockOrganizations);
        }),
      ],
    },
  },
};

export const Empty: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/api/admin/v1/organizations", () => {
          return HttpResponse.json(mockOrganizations);
        }),
        http.get("*/api/admin/v1/organizations/*/teams", () => {
          return HttpResponse.json({ data: [], total: 0 });
        }),
        http.get("*/api/admin/v1/organizations/*/projects", () => {
          return HttpResponse.json({ data: [], total: 0 });
        }),
      ],
    },
  },
};

export const NoOrganizations: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/api/admin/v1/organizations", () => {
          return HttpResponse.json({ data: [], total: 0 });
        }),
      ],
    },
  },
};

export const WithProjectCounts: Story = {
  parameters: {
    msw: {
      handlers: defaultHandlers,
    },
  },
};

export const ManyTeams: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/api/admin/v1/organizations", () => {
          return HttpResponse.json(mockOrganizations);
        }),
        http.get("*/api/admin/v1/organizations/acme-corp/teams", () => {
          return HttpResponse.json({
            data: Array.from({ length: 9 }, (_, i) => ({
              id: `team-${i + 1}`,
              name: `Team ${i + 1}`,
              slug: `team-${i + 1}`,
              org_id: "org-1",
              created_at: new Date(2024, 0, i + 1).toISOString(),
              updated_at: new Date(2024, 0, i + 15).toISOString(),
            })),
            total: 9,
          });
        }),
        http.get("*/api/admin/v1/organizations/acme-corp/projects", () => {
          return HttpResponse.json({
            data: Array.from({ length: 20 }, (_, i) => ({
              id: `proj-${i + 1}`,
              name: `Project ${i + 1}`,
              slug: `project-${i + 1}`,
              team_id: `team-${(i % 9) + 1}`,
              created_at: new Date(2024, 0, i + 1).toISOString(),
              updated_at: new Date(2024, 0, i + 15).toISOString(),
            })),
            total: 20,
          });
        }),
      ],
    },
  },
};

export const Error: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/api/admin/v1/organizations", () => {
          return HttpResponse.error();
        }),
      ],
    },
  },
};
