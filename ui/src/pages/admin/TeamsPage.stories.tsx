import type { Meta, StoryObj } from "@storybook/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter, Routes, Route } from "react-router-dom";
import { http, HttpResponse } from "msw";
import TeamsPage from "./TeamsPage";
import type {
  Team,
  TeamListResponse,
  Organization,
  OrganizationListResponse,
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
  pagination: { limit: 100, has_more: false },
};

const mockTeams: Team[] = [
  {
    id: "team-001",
    slug: "engineering",
    name: "Engineering",
    org_id: "org-123",
    created_at: "2024-01-15T09:00:00Z",
    updated_at: "2024-06-10T14:30:00Z",
  },
  {
    id: "team-002",
    slug: "data-science",
    name: "Data Science",
    org_id: "org-123",
    created_at: "2024-02-20T11:00:00Z",
    updated_at: "2024-05-28T08:15:00Z",
  },
  {
    id: "team-003",
    slug: "platform",
    name: "Platform",
    org_id: "org-123",
    created_at: "2024-03-05T16:45:00Z",
    updated_at: "2024-06-12T12:00:00Z",
  },
  {
    id: "team-004",
    slug: "security",
    name: "Security",
    org_id: "org-123",
    created_at: "2024-04-10T08:00:00Z",
    updated_at: "2024-04-10T08:00:00Z",
  },
];

const mockTeamsResponse: TeamListResponse = {
  data: mockTeams,
  pagination: { limit: 25, has_more: false },
};

const emptyTeamsResponse: TeamListResponse = {
  data: [],
  pagination: { limit: 25, has_more: false },
};

const meta: Meta<typeof TeamsPage> = {
  title: "Admin/TeamsPage",
  component: TeamsPage,
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
            <MemoryRouter initialEntries={["/admin/teams"]}>
              <Routes>
                <Route path="/admin/teams" element={<Story />} />
                <Route
                  path="/admin/organizations/:orgSlug/teams/:teamSlug"
                  element={<div>Team Detail Page</div>}
                />
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
        http.get("*/admin/v1/organizations/acme-corp/teams", () => {
          return HttpResponse.json(mockTeamsResponse);
        }),
        http.get("*/admin/v1/organizations/stark-industries/teams", () => {
          return HttpResponse.json(emptyTeamsResponse);
        }),
        http.post("*/admin/v1/organizations/:orgSlug/teams", async ({ request }) => {
          const body = (await request.json()) as Record<string, unknown>;
          const newTeam: Team = {
            id: `team-${Date.now()}`,
            slug: body.slug as string,
            name: body.name as string,
            org_id: "org-123",
            created_at: new Date().toISOString(),
            updated_at: new Date().toISOString(),
          };
          return HttpResponse.json(newTeam, { status: 201 });
        }),
        http.patch("*/admin/v1/organizations/:orgSlug/teams/:teamSlug", async ({ request }) => {
          const body = (await request.json()) as Record<string, unknown>;
          const updated: Team = {
            ...mockTeams[0],
            name: (body.name as string) || mockTeams[0].name,
            updated_at: new Date().toISOString(),
          };
          return HttpResponse.json(updated);
        }),
        http.delete("*/admin/v1/organizations/:orgSlug/teams/:teamSlug", () => {
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
        http.get("*/admin/v1/organizations/acme-corp/teams", () => {
          return HttpResponse.json(emptyTeamsResponse);
        }),
        http.post("*/admin/v1/organizations/:orgSlug/teams", async ({ request }) => {
          const body = (await request.json()) as Record<string, unknown>;
          const newTeam: Team = {
            id: `team-${Date.now()}`,
            slug: body.slug as string,
            name: body.name as string,
            org_id: "org-123",
            created_at: new Date().toISOString(),
            updated_at: new Date().toISOString(),
          };
          return HttpResponse.json(newTeam, { status: 201 });
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
        http.get("*/admin/v1/organizations/acme-corp/teams", () => {
          return HttpResponse.json(
            { error: { code: "internal_error", message: "Database connection failed" } },
            { status: 500 }
          );
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
            pagination: { limit: 100, has_more: false },
          });
        }),
      ],
    },
  },
};

export const ManyTeams: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/organizations", () => {
          return HttpResponse.json(mockOrgsResponse);
        }),
        http.get("*/admin/v1/organizations/acme-corp/teams", () => {
          const manyTeams: Team[] = Array.from({ length: 25 }, (_, i) => ({
            id: `team-${i + 1}`,
            slug: `team-${i + 1}`,
            name: `Team ${i + 1}`,
            org_id: "org-123",
            created_at: new Date(2024, 0, i + 1).toISOString(),
            updated_at: new Date(2024, 5, i + 1).toISOString(),
          }));
          return HttpResponse.json({
            data: manyTeams,
            pagination: {
              limit: 25,
              has_more: true,
              next_cursor: "bW9ja19jdXJzb3I=",
            },
          });
        }),
        http.post("*/admin/v1/organizations/:orgSlug/teams", async ({ request }) => {
          const body = (await request.json()) as Record<string, unknown>;
          const newTeam: Team = {
            id: `team-${Date.now()}`,
            slug: body.slug as string,
            name: body.name as string,
            org_id: "org-123",
            created_at: new Date().toISOString(),
            updated_at: new Date().toISOString(),
          };
          return HttpResponse.json(newTeam, { status: 201 });
        }),
        http.patch("*/admin/v1/organizations/:orgSlug/teams/:teamSlug", async ({ request }) => {
          const body = (await request.json()) as Record<string, unknown>;
          const updated: Team = {
            ...mockTeams[0],
            name: (body.name as string) || mockTeams[0].name,
            updated_at: new Date().toISOString(),
          };
          return HttpResponse.json(updated);
        }),
        http.delete("*/admin/v1/organizations/:orgSlug/teams/:teamSlug", () => {
          return HttpResponse.json({});
        }),
      ],
    },
  },
};
