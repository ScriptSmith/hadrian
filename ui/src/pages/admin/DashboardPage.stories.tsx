import type { Meta, StoryObj } from "@storybook/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter } from "react-router-dom";
import { http, HttpResponse } from "msw";
import DashboardPage from "./DashboardPage";
import type {
  Organization,
  OrganizationListResponse,
  Project,
  ProjectListResponse,
  User,
  UserListResponse,
} from "@/api/generated/types.gen";

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
    updated_at: "2024-06-15T10:30:00Z",
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

const mockProjects: Project[] = [
  {
    id: "proj-1",
    slug: "api-gateway",
    name: "API Gateway",
    org_id: "org-123",
    created_at: "2024-01-15T00:00:00Z",
    updated_at: "2024-06-15T10:30:00Z",
  },
  {
    id: "proj-2",
    slug: "data-pipeline",
    name: "Data Pipeline",
    org_id: "org-123",
    team_id: "team-001",
    created_at: "2024-03-01T00:00:00Z",
    updated_at: "2024-05-10T08:00:00Z",
  },
  {
    id: "proj-3",
    slug: "ml-platform",
    name: "ML Platform",
    org_id: "org-123",
    created_at: "2024-04-01T00:00:00Z",
    updated_at: "2024-04-01T00:00:00Z",
  },
];

const mockProjectsResponse: ProjectListResponse = {
  data: mockProjects,
  pagination: {
    limit: 100,
    has_more: false,
  },
};

const mockUsers: User[] = [
  {
    id: "user-1",
    external_id: "auth0|alice",
    email: "alice@acme.com",
    name: "Alice Developer",
    created_at: "2024-01-10T00:00:00Z",
    updated_at: "2024-06-15T10:30:00Z",
  },
  {
    id: "user-2",
    external_id: "auth0|bob",
    email: "bob@acme.com",
    name: "Bob Engineer",
    created_at: "2024-02-20T00:00:00Z",
    updated_at: "2024-02-20T00:00:00Z",
  },
  {
    id: "user-3",
    external_id: "auth0|carol",
    email: "carol@acme.com",
    name: "Carol Admin",
    created_at: "2024-03-05T00:00:00Z",
    updated_at: "2024-05-10T08:00:00Z",
  },
  {
    id: "user-4",
    external_id: "auth0|dave",
    email: "dave@acme.com",
    name: "Dave Ops",
    created_at: "2024-04-01T00:00:00Z",
    updated_at: "2024-04-01T00:00:00Z",
  },
  {
    id: "user-5",
    external_id: "auth0|eve",
    email: "eve@acme.com",
    name: "Eve Analyst",
    created_at: "2024-05-01T00:00:00Z",
    updated_at: "2024-05-01T00:00:00Z",
  },
];

const mockUsersResponse: UserListResponse = {
  data: mockUsers,
  pagination: {
    limit: 100,
    has_more: false,
  },
};

const emptyOrgsResponse: OrganizationListResponse = {
  data: [],
  pagination: {
    limit: 100,
    has_more: false,
  },
};

const emptyUsersResponse: UserListResponse = {
  data: [],
  pagination: {
    limit: 100,
    has_more: false,
  },
};

const defaultHandlers = [
  http.get("*/admin/v1/organizations", () => {
    return HttpResponse.json(mockOrgsResponse);
  }),
  http.get("*/admin/v1/organizations/acme-corp/projects", () => {
    return HttpResponse.json(mockProjectsResponse);
  }),
  http.get("*/admin/v1/users", () => {
    return HttpResponse.json(mockUsersResponse);
  }),
];

const meta: Meta<typeof DashboardPage> = {
  title: "Admin/DashboardPage",
  component: DashboardPage,
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
        <MemoryRouter initialEntries={["/admin"]}>
          <div className="min-h-screen bg-background">
            <Story />
          </div>
        </MemoryRouter>
      </QueryClientProvider>
    ),
  ],
};

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  parameters: {
    msw: {
      handlers: defaultHandlers,
    },
  },
};

export const Empty: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/organizations", () => {
          return HttpResponse.json(emptyOrgsResponse);
        }),
        http.get("*/admin/v1/users", () => {
          return HttpResponse.json(emptyUsersResponse);
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
          await new Promise(() => {});
          return new Response();
        }),
        http.get("*/admin/v1/users", async () => {
          await new Promise(() => {});
          return new Response();
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
          return HttpResponse.json(
            { error: { code: "internal_error", message: "Database connection failed" } },
            { status: 500 }
          );
        }),
        http.get("*/admin/v1/users", () => {
          return HttpResponse.json(
            { error: { code: "internal_error", message: "Database connection failed" } },
            { status: 500 }
          );
        }),
      ],
    },
  },
};
