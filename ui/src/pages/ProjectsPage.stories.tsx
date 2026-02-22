import type { Meta, StoryObj } from "@storybook/react";
import { http, HttpResponse } from "msw";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter } from "react-router-dom";
import ProjectsPage from "./ProjectsPage";

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
      created_at: "2024-01-01T00:00:00Z",
      updated_at: "2024-01-01T00:00:00Z",
    },
    {
      id: "team-2",
      name: "Data Science",
      slug: "data-science",
      org_id: "org-1",
      created_at: "2024-01-01T00:00:00Z",
      updated_at: "2024-01-01T00:00:00Z",
    },
  ],
  total: 2,
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
    {
      id: "proj-4",
      name: "Personal Project",
      slug: "personal-project",
      team_id: null,
      created_at: "2024-04-01T00:00:00Z",
      updated_at: "2024-04-15T00:00:00Z",
    },
  ],
  total: 4,
};

const defaultHandlers = [
  http.get("*/api/admin/v1/organizations", () => {
    return HttpResponse.json(mockOrganizations);
  }),
  http.get("*/api/admin/v1/organizations/acme-corp/projects", () => {
    return HttpResponse.json(mockProjectsAcme);
  }),
  http.get("*/api/admin/v1/organizations/acme-corp/teams", () => {
    return HttpResponse.json(mockTeamsAcme);
  }),
];

const meta: Meta<typeof ProjectsPage> = {
  title: "Pages/ProjectsPage",
  component: ProjectsPage,
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
type Story = StoryObj<typeof ProjectsPage>;

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
        http.get("*/api/admin/v1/organizations/*/projects", () => {
          return HttpResponse.json({ data: [], total: 0 });
        }),
        http.get("*/api/admin/v1/organizations/*/teams", () => {
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

export const WithTeamBadges: Story = {
  parameters: {
    msw: {
      handlers: defaultHandlers,
    },
  },
};

export const ManyProjects: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/api/admin/v1/organizations", () => {
          return HttpResponse.json(mockOrganizations);
        }),
        http.get("*/api/admin/v1/organizations/acme-corp/projects", () => {
          return HttpResponse.json({
            data: Array.from({ length: 12 }, (_, i) => ({
              id: `proj-${i + 1}`,
              name: `Project ${i + 1}`,
              slug: `project-${i + 1}`,
              team_id: i % 3 === 0 ? "team-1" : i % 3 === 1 ? "team-2" : null,
              created_at: new Date(2024, 0, i + 1).toISOString(),
              updated_at: new Date(2024, 0, i + 15).toISOString(),
            })),
            total: 12,
          });
        }),
        http.get("*/api/admin/v1/organizations/acme-corp/teams", () => {
          return HttpResponse.json(mockTeamsAcme);
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
