import type { Meta, StoryObj } from "@storybook/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter } from "react-router-dom";
import { http, HttpResponse } from "msw";
import SessionInfoPage from "./SessionInfoPage";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: false,
      staleTime: Infinity,
    },
  },
});

const meta: Meta<typeof SessionInfoPage> = {
  title: "Pages/Admin/SessionInfoPage",
  component: SessionInfoPage,
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
        <MemoryRouter initialEntries={["/session"]}>
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

const mockFullSession = {
  identity: {
    external_id: "auth0|123456789",
    email: "alice@acme.com",
    name: "Alice Developer",
    roles: ["org_admin", "team_admin"],
    idp_groups: ["/acme/engineering", "/acme/platform-team", "/acme/admins"],
  },
  user: {
    id: "550e8400-e29b-41d4-a716-446655440000",
    external_id: "auth0|123456789",
    email: "alice@acme.com",
    name: "Alice Developer",
    created_at: "2024-01-15T10:30:00Z",
    updated_at: "2025-01-10T14:22:00Z",
  },
  organizations: [
    {
      org_id: "org-123",
      org_slug: "acme",
      org_name: "Acme Corporation",
      role: "admin",
      joined_at: "2024-01-15T10:30:00Z",
    },
    {
      org_id: "org-456",
      org_slug: "acme-labs",
      org_name: "Acme Labs",
      role: "member",
      joined_at: "2024-06-01T09:00:00Z",
    },
  ],
  teams: [
    {
      team_id: "team-001",
      team_slug: "platform",
      team_name: "Platform Team",
      org_slug: "acme",
      role: "admin",
      joined_at: "2024-01-20T14:00:00Z",
    },
    {
      team_id: "team-002",
      team_slug: "data-science",
      team_name: "Data Science",
      org_slug: "acme",
      role: "member",
      joined_at: "2024-03-15T11:30:00Z",
    },
  ],
  projects: [
    {
      project_id: "proj-001",
      project_slug: "api-gateway",
      project_name: "API Gateway",
      org_slug: "acme",
      role: "owner",
      joined_at: "2024-02-01T08:00:00Z",
    },
  ],
  sso_connection: {
    name: "default",
    type: "oidc",
    issuer: "https://acme.us.auth0.com/",
    groups_claim: "groups",
    jit_enabled: true,
  },
  auth_method: "oidc",
  server_time: new Date().toISOString(),
};

export const FullSession: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("/api/admin/v1/session-info", () => {
          return HttpResponse.json(mockFullSession);
        }),
      ],
    },
  },
};

export const MinimalSession: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("/api/admin/v1/session-info", () => {
          return HttpResponse.json({
            identity: {
              external_id: "user@example.com",
              email: "user@example.com",
              roles: [],
              idp_groups: [],
            },
            organizations: [],
            teams: [],
            projects: [],
            auth_method: "proxy_auth",
            server_time: new Date().toISOString(),
          });
        }),
      ],
    },
  },
};

export const NoDbUser: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("/api/admin/v1/session-info", () => {
          return HttpResponse.json({
            identity: {
              external_id: "newuser@example.com",
              email: "newuser@example.com",
              name: "New User",
              roles: ["user"],
              idp_groups: ["/visitors"],
            },
            // No user property - not yet provisioned
            organizations: [],
            teams: [],
            projects: [],
            sso_connection: {
              name: "default",
              type: "oidc",
              issuer: "https://idp.example.com/",
              groups_claim: "groups",
              jit_enabled: false, // JIT disabled, user not created
            },
            auth_method: "oidc",
            server_time: new Date().toISOString(),
          });
        }),
      ],
    },
  },
};

export const ManyGroups: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("/api/admin/v1/session-info", () => {
          return HttpResponse.json({
            ...mockFullSession,
            identity: {
              ...mockFullSession.identity,
              idp_groups: [
                "/acme/engineering",
                "/acme/platform-team",
                "/acme/admins",
                "/acme/ml-team",
                "/acme/security",
                "/acme/devops",
                "/acme/api-guild",
                "/acme/python-guild",
                "/acme/rust-guild",
                "/acme/frontend-guild",
              ],
            },
          });
        }),
      ],
    },
  },
};

export const ProxyAuth: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("/api/admin/v1/session-info", () => {
          return HttpResponse.json({
            identity: {
              external_id: "alice.developer",
              email: "alice@internal.corp",
              name: "Alice Developer",
              roles: ["admin", "developer"],
              idp_groups: ["admin", "developer"],
            },
            user: {
              id: "550e8400-e29b-41d4-a716-446655440000",
              external_id: "alice.developer",
              email: "alice@internal.corp",
              name: "Alice Developer",
              created_at: "2024-01-15T10:30:00Z",
              updated_at: "2025-01-10T14:22:00Z",
            },
            organizations: [
              {
                org_id: "org-123",
                org_slug: "internal",
                org_name: "Internal",
                role: "admin",
                joined_at: "2024-01-15T10:30:00Z",
              },
            ],
            teams: [],
            projects: [],
            sso_connection: {
              name: "default",
              type: "proxy_auth",
              jit_enabled: false,
            },
            auth_method: "proxy_auth",
            server_time: new Date().toISOString(),
          });
        }),
      ],
    },
  },
};

export const Loading: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("/api/admin/v1/session-info", async () => {
          // Never resolves - shows loading state
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
        http.get("/api/admin/v1/session-info", () => {
          return HttpResponse.json({ error: "Internal server error" }, { status: 500 });
        }),
      ],
    },
  },
};
