import type { Meta, StoryObj } from "@storybook/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter, Routes, Route } from "react-router-dom";
import { http, HttpResponse } from "msw";
import ScimConfigPage from "./ScimConfigPage";
import type {
  OrgScimConfig,
  Organization,
  TeamListResponse,
  Team,
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

const mockOrg: Organization = {
  id: "org-123",
  slug: "acme-corp",
  name: "Acme Corporation",
  created_at: "2024-01-01T00:00:00Z",
  updated_at: "2024-01-01T00:00:00Z",
};

const mockTeams: Team[] = [
  {
    id: "team-001",
    name: "Engineering",
    slug: "engineering",
    org_id: "org-123",
    created_at: "2024-01-15T09:00:00Z",
    updated_at: "2024-01-15T09:00:00Z",
  },
  {
    id: "team-002",
    name: "Product",
    slug: "product",
    org_id: "org-123",
    created_at: "2024-02-01T11:00:00Z",
    updated_at: "2024-02-01T11:00:00Z",
  },
];

const mockTeamsResponse: TeamListResponse = {
  data: mockTeams,
  pagination: {
    limit: 100,
    has_more: false,
  },
};

const mockScimConfig: OrgScimConfig = {
  id: "scim-001",
  org_id: "org-123",
  enabled: true,
  create_users: true,
  sync_display_name: true,
  deactivate_deletes_user: false,
  revoke_api_keys_on_deactivate: true,
  default_org_role: "member",
  default_team_id: "team-001",
  default_team_role: "member",
  token_prefix: "scim_a1b2",
  token_last_used_at: "2024-06-14T16:30:00Z",
  created_at: "2024-03-01T10:00:00Z",
  updated_at: "2024-06-10T08:45:00Z",
};

const meta: Meta<typeof ScimConfigPage> = {
  title: "Admin/ScimConfigPage",
  component: ScimConfigPage,
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
            <MemoryRouter initialEntries={["/admin/organizations/acme-corp/scim"]}>
              <Routes>
                <Route path="/admin/organizations/:orgSlug/scim" element={<Story />} />
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
        http.get("*/admin/v1/organizations/acme-corp", () => {
          return HttpResponse.json(mockOrg);
        }),
        http.get("*/admin/v1/organizations/acme-corp/scim-config", () => {
          return HttpResponse.json(mockScimConfig);
        }),
        http.get("*/admin/v1/organizations/acme-corp/teams", () => {
          return HttpResponse.json(mockTeamsResponse);
        }),
        http.patch("*/admin/v1/organizations/acme-corp/scim-config", async ({ request }) => {
          const body = (await request.json()) as Record<string, unknown>;
          return HttpResponse.json({
            ...mockScimConfig,
            ...body,
            updated_at: new Date().toISOString(),
          });
        }),
        http.delete("*/admin/v1/organizations/acme-corp/scim-config", () => {
          return new HttpResponse(null, { status: 200 });
        }),
        http.post("*/admin/v1/organizations/acme-corp/scim-config/rotate-token", () => {
          return HttpResponse.json({
            ...mockScimConfig,
            token_prefix: "scim_x9y8",
            token: "scim_x9y8z7w6v5u4t3s2r1q0p9o8n7m6l5k4j3i2h1g0",
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
        http.get("*/admin/v1/organizations/acme-corp", async () => {
          await new Promise((resolve) => setTimeout(resolve, 100000));
          return HttpResponse.json(mockOrg);
        }),
        http.get("*/admin/v1/organizations/acme-corp/scim-config", async () => {
          await new Promise((resolve) => setTimeout(resolve, 100000));
          return HttpResponse.json(mockScimConfig);
        }),
      ],
    },
  },
};

export const Empty: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/organizations/acme-corp", () => {
          return HttpResponse.json(mockOrg);
        }),
        http.get("*/admin/v1/organizations/acme-corp/scim-config", () => {
          return HttpResponse.json(
            { error: { code: "not_found", message: "SCIM config not found" } },
            { status: 404 }
          );
        }),
        http.get("*/admin/v1/organizations/acme-corp/teams", () => {
          return HttpResponse.json(mockTeamsResponse);
        }),
        http.post("*/admin/v1/organizations/acme-corp/scim-config", async ({ request }) => {
          const body = (await request.json()) as Record<string, unknown>;
          return HttpResponse.json(
            {
              ...mockScimConfig,
              ...body,
              token: "scim_new_a1b2c3d4e5f6g7h8i9j0k1l2m3n4o5p6",
            },
            { status: 201 }
          );
        }),
      ],
    },
  },
};

export const Error: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/organizations/acme-corp", () => {
          return HttpResponse.json(mockOrg);
        }),
        http.get("*/admin/v1/organizations/acme-corp/scim-config", () => {
          return HttpResponse.json(
            { error: { code: "internal_error", message: "Database connection failed" } },
            { status: 500 }
          );
        }),
      ],
    },
  },
};

export const Disabled: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/organizations/acme-corp", () => {
          return HttpResponse.json(mockOrg);
        }),
        http.get("*/admin/v1/organizations/acme-corp/scim-config", () => {
          return HttpResponse.json({
            ...mockScimConfig,
            enabled: false,
            token_last_used_at: null,
          });
        }),
        http.get("*/admin/v1/organizations/acme-corp/teams", () => {
          return HttpResponse.json(mockTeamsResponse);
        }),
      ],
    },
  },
};

export const NeverUsed: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/organizations/acme-corp", () => {
          return HttpResponse.json(mockOrg);
        }),
        http.get("*/admin/v1/organizations/acme-corp/scim-config", () => {
          return HttpResponse.json({
            ...mockScimConfig,
            token_last_used_at: null,
            default_team_id: null,
          });
        }),
        http.get("*/admin/v1/organizations/acme-corp/teams", () => {
          return HttpResponse.json(mockTeamsResponse);
        }),
      ],
    },
  },
};
