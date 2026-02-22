import type { Meta, StoryObj } from "@storybook/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter, Routes, Route } from "react-router-dom";
import { http, HttpResponse } from "msw";
import ProjectsPage from "./ProjectsPage";
import type {
  Project,
  ProjectListResponse,
  Organization,
  OrganizationListResponse,
  Team,
  TeamListResponse,
} from "@/api/generated/types.gen";
import { ToastProvider } from "@/components/Toast/Toast";
import { ConfirmDialogProvider } from "@/components/ConfirmDialog/ConfirmDialog";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: false,
      staleTime: Infinity,
    },
  },
});

const mockOrgs: Organization[] = [
  {
    id: "org-123",
    slug: "acme-corp",
    name: "Acme Corporation",
    created_at: "2024-01-01T00:00:00Z",
    updated_at: "2024-01-01T00:00:00Z",
  },
  {
    id: "org-456",
    slug: "stark-industries",
    name: "Stark Industries",
    created_at: "2024-02-01T00:00:00Z",
    updated_at: "2024-02-01T00:00:00Z",
  },
];

const mockOrgsResponse: OrganizationListResponse = {
  data: mockOrgs,
  pagination: {
    limit: 100,
    has_more: false,
  },
};

const mockTeams: Team[] = [
  {
    id: "team-001",
    slug: "engineering",
    name: "Engineering",
    org_id: "org-123",
    created_at: "2024-01-15T00:00:00Z",
    updated_at: "2024-01-15T00:00:00Z",
  },
  {
    id: "team-002",
    slug: "data-science",
    name: "Data Science",
    org_id: "org-123",
    created_at: "2024-02-20T00:00:00Z",
    updated_at: "2024-02-20T00:00:00Z",
  },
];

const mockTeamsResponse: TeamListResponse = {
  data: mockTeams,
  pagination: {
    limit: 100,
    has_more: false,
  },
};

const mockProjects: Project[] = [
  {
    id: "proj-001",
    slug: "production-api",
    name: "Production API",
    org_id: "org-123",
    team_id: "team-001",
    created_at: "2024-01-20T09:00:00Z",
    updated_at: "2024-06-15T14:30:00Z",
  },
  {
    id: "proj-002",
    slug: "ml-pipeline",
    name: "ML Pipeline",
    org_id: "org-123",
    team_id: "team-002",
    created_at: "2024-02-10T11:00:00Z",
    updated_at: "2024-05-28T08:15:00Z",
  },
  {
    id: "proj-003",
    slug: "internal-tools",
    name: "Internal Tools",
    org_id: "org-123",
    team_id: null,
    created_at: "2024-03-05T16:45:00Z",
    updated_at: "2024-06-10T12:00:00Z",
  },
  {
    id: "proj-004",
    slug: "sandbox",
    name: "Sandbox",
    org_id: "org-123",
    team_id: null,
    created_at: "2024-04-22T13:00:00Z",
    updated_at: "2024-04-22T13:00:00Z",
  },
];

const mockProjectsResponse: ProjectListResponse = {
  data: mockProjects,
  pagination: {
    limit: 25,
    has_more: false,
  },
};

const emptyProjectsResponse: ProjectListResponse = {
  data: [],
  pagination: {
    limit: 25,
    has_more: false,
  },
};

const meta: Meta<typeof ProjectsPage> = {
  title: "Admin/ProjectsPage",
  component: ProjectsPage,
  parameters: {
    layout: "fullscreen",
    a11y: {
      config: {
        rules: [{ id: "heading-order", enabled: false }],
      },
    },
  },
  decorators: [
    (Story) => (
      <QueryClientProvider client={queryClient}>
        <ToastProvider>
          <ConfirmDialogProvider>
            <MemoryRouter initialEntries={["/admin/projects"]}>
              <Routes>
                <Route path="/admin/projects" element={<Story />} />
              </Routes>
            </MemoryRouter>
          </ConfirmDialogProvider>
        </ToastProvider>
      </QueryClientProvider>
    ),
  ],
};

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/organizations", () => {
          return HttpResponse.json(mockOrgsResponse);
        }),
        http.get("*/admin/v1/organizations/acme-corp/projects", () => {
          return HttpResponse.json(mockProjectsResponse);
        }),
        http.get("*/admin/v1/organizations/stark-industries/projects", () => {
          return HttpResponse.json(emptyProjectsResponse);
        }),
        http.get("*/admin/v1/organizations/acme-corp/teams", () => {
          return HttpResponse.json(mockTeamsResponse);
        }),
        http.get("*/admin/v1/organizations/stark-industries/teams", () => {
          return HttpResponse.json({ data: [], pagination: { limit: 100, has_more: false } });
        }),
        http.post("*/admin/v1/organizations/:orgSlug/projects", async ({ request }) => {
          const body = (await request.json()) as Record<string, unknown>;
          const newProject: Project = {
            id: `proj-${Date.now()}`,
            slug: body.slug as string,
            name: body.name as string,
            org_id: "org-123",
            team_id: (body.team_id as string) || null,
            created_at: new Date().toISOString(),
            updated_at: new Date().toISOString(),
          };
          return HttpResponse.json(newProject, { status: 201 });
        }),
        http.patch(
          "*/admin/v1/organizations/:orgSlug/projects/:projectSlug",
          async ({ request }) => {
            const body = (await request.json()) as Record<string, unknown>;
            const updated: Project = {
              ...mockProjects[0],
              name: (body.name as string) || mockProjects[0].name,
              updated_at: new Date().toISOString(),
            };
            return HttpResponse.json(updated);
          }
        ),
        http.delete("*/admin/v1/organizations/:orgSlug/projects/:projectSlug", () => {
          return HttpResponse.json({});
        }),
      ],
    },
  },
};

export const Loading: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/organizations", async () => {
          await new Promise((resolve) => setTimeout(resolve, 100000));
          return HttpResponse.json(mockOrgsResponse);
        }),
        http.get("*/admin/v1/organizations/:orgSlug/projects", async () => {
          await new Promise((resolve) => setTimeout(resolve, 100000));
          return HttpResponse.json(mockProjectsResponse);
        }),
        http.get("*/admin/v1/organizations/:orgSlug/teams", async () => {
          await new Promise((resolve) => setTimeout(resolve, 100000));
          return HttpResponse.json(mockTeamsResponse);
        }),
      ],
    },
  },
};

export const Empty: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/organizations", () => {
          return HttpResponse.json(mockOrgsResponse);
        }),
        http.get("*/admin/v1/organizations/acme-corp/projects", () => {
          return HttpResponse.json(emptyProjectsResponse);
        }),
        http.get("*/admin/v1/organizations/acme-corp/teams", () => {
          return HttpResponse.json(mockTeamsResponse);
        }),
        http.post("*/admin/v1/organizations/:orgSlug/projects", async ({ request }) => {
          const body = (await request.json()) as Record<string, unknown>;
          const newProject: Project = {
            id: `proj-${Date.now()}`,
            slug: body.slug as string,
            name: body.name as string,
            org_id: "org-123",
            team_id: (body.team_id as string) || null,
            created_at: new Date().toISOString(),
            updated_at: new Date().toISOString(),
          };
          return HttpResponse.json(newProject, { status: 201 });
        }),
      ],
    },
  },
};

export const Error: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/organizations", () => {
          return HttpResponse.json(mockOrgsResponse);
        }),
        http.get("*/admin/v1/organizations/acme-corp/projects", () => {
          return HttpResponse.json(
            { error: { code: "internal_error", message: "Database connection failed" } },
            { status: 500 }
          );
        }),
        http.get("*/admin/v1/organizations/acme-corp/teams", () => {
          return HttpResponse.json(mockTeamsResponse);
        }),
      ],
    },
  },
};

export const NoOrganizations: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/organizations", () => {
          return HttpResponse.json({
            data: [],
            pagination: {
              limit: 100,
              has_more: false,
            },
          });
        }),
      ],
    },
  },
};

export const ManyProjects: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/organizations", () => {
          return HttpResponse.json(mockOrgsResponse);
        }),
        http.get("*/admin/v1/organizations/acme-corp/projects", () => {
          const manyProjects: Project[] = Array.from({ length: 25 }, (_, i) => ({
            id: `proj-${i + 1}`,
            slug: `project-${i + 1}`,
            name: `Project ${i + 1}`,
            org_id: "org-123",
            team_id: i % 3 === 0 ? "team-001" : null,
            created_at: new Date(2024, 0, i + 1).toISOString(),
            updated_at: new Date(2024, 5, i + 1).toISOString(),
          }));
          return HttpResponse.json({
            data: manyProjects,
            pagination: {
              limit: 25,
              has_more: true,
              next_cursor: "bW9ja19jdXJzb3I=",
            },
          });
        }),
        http.get("*/admin/v1/organizations/acme-corp/teams", () => {
          return HttpResponse.json(mockTeamsResponse);
        }),
      ],
    },
  },
};
