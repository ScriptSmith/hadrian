import type { Meta, StoryObj } from "@storybook/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter, Routes, Route } from "react-router-dom";
import { http, HttpResponse } from "msw";
import ApiKeysPage from "./ApiKeysPage";
import type {
  ApiKey,
  ApiKeyListResponse,
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
  pagination: {
    limit: 100,
    has_more: false,
  },
};

const mockApiKeys: ApiKey[] = [
  {
    id: "key-001",
    name: "Production API Key",
    key_prefix: "hdr_prod_a1b2c3",
    owner: { type: "organization", org_id: "org-123" },
    budget_limit_cents: 50000,
    budget_period: "monthly",
    revoked_at: null,
    expires_at: "2025-12-31T23:59:59Z",
    last_used_at: "2024-06-15T14:30:00Z",
    created_at: "2024-01-10T09:00:00Z",
  },
  {
    id: "key-002",
    name: "Development Key",
    key_prefix: "hdr_dev_x9y8z7",
    owner: { type: "user", user_id: "usr-001" },
    budget_limit_cents: null,
    budget_period: null,
    revoked_at: null,
    expires_at: null,
    last_used_at: "2024-06-14T08:22:00Z",
    created_at: "2024-02-20T11:00:00Z",
  },
  {
    id: "key-003",
    name: "CI/CD Pipeline Key",
    key_prefix: "hdr_ci_m4n5o6",
    owner: { type: "service_account", service_account_id: "sa-1" },
    budget_limit_cents: 10000,
    budget_period: "daily",
    revoked_at: null,
    expires_at: "2025-06-30T23:59:59Z",
    last_used_at: "2024-06-15T16:45:00Z",
    created_at: "2024-03-05T16:45:00Z",
  },
  {
    id: "key-004",
    name: "Deprecated Key",
    key_prefix: "hdr_old_q1w2e3",
    owner: { type: "team", team_id: "team-001" },
    budget_limit_cents: 25000,
    budget_period: "monthly",
    revoked_at: "2024-05-01T12:00:00Z",
    expires_at: null,
    last_used_at: "2024-04-28T09:15:00Z",
    created_at: "2024-01-15T10:30:00Z",
  },
  {
    id: "key-005",
    name: "Monitoring Key",
    key_prefix: "hdr_mon_r4s5t6",
    owner: { type: "project", project_id: "proj-001" },
    budget_limit_cents: null,
    budget_period: null,
    revoked_at: null,
    expires_at: null,
    last_used_at: null,
    created_at: "2024-05-01T07:30:00Z",
  },
];

const mockApiKeysResponse: ApiKeyListResponse = {
  data: mockApiKeys,
  pagination: {
    limit: 25,
    has_more: false,
  },
};

const emptyApiKeysResponse: ApiKeyListResponse = {
  data: [],
  pagination: {
    limit: 25,
    has_more: false,
  },
};

const meta: Meta<typeof ApiKeysPage> = {
  title: "Admin/ApiKeysPage",
  component: ApiKeysPage,
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
            <MemoryRouter initialEntries={["/admin/api-keys"]}>
              <Routes>
                <Route path="/admin/api-keys" element={<Story />} />
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
        http.get("*/admin/v1/organizations/acme-corp/api-keys", () => {
          return HttpResponse.json(mockApiKeysResponse);
        }),
        http.get("*/admin/v1/organizations/stark-industries/api-keys", () => {
          return HttpResponse.json(emptyApiKeysResponse);
        }),
        http.post("*/admin/v1/api-keys", async ({ request }) => {
          const body = (await request.json()) as Record<string, unknown>;
          const newKey: ApiKey & { key: string } = {
            id: `key-${Date.now()}`,
            name: body.name as string,
            key_prefix: "hdr_new_abc123",
            key: "hdr_new_abc123def456ghi789jkl012mno345",
            owner: body.owner as ApiKey["owner"],
            budget_limit_cents: (body.budget_limit_cents as number) || null,
            budget_period: null,
            revoked_at: null,
            expires_at: null,
            last_used_at: null,
            created_at: new Date().toISOString(),
          };
          return HttpResponse.json(newKey, { status: 201 });
        }),
        http.delete("*/admin/v1/api-keys/:keyId/revoke", () => {
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
        http.get("*/admin/v1/organizations/:orgSlug/api-keys", async () => {
          await new Promise((resolve) => setTimeout(resolve, 100000));
          return HttpResponse.json(mockApiKeysResponse);
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
        http.get("*/admin/v1/organizations/acme-corp/api-keys", () => {
          return HttpResponse.json(emptyApiKeysResponse);
        }),
        http.post("*/admin/v1/api-keys", async ({ request }) => {
          const body = (await request.json()) as Record<string, unknown>;
          const newKey: ApiKey & { key: string } = {
            id: `key-${Date.now()}`,
            name: body.name as string,
            key_prefix: "hdr_new_abc123",
            key: "hdr_new_abc123def456ghi789jkl012mno345",
            owner: body.owner as ApiKey["owner"],
            budget_limit_cents: null,
            budget_period: null,
            revoked_at: null,
            expires_at: null,
            last_used_at: null,
            created_at: new Date().toISOString(),
          };
          return HttpResponse.json(newKey, { status: 201 });
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
        http.get("*/admin/v1/organizations/acme-corp/api-keys", () => {
          return HttpResponse.json(
            { error: { code: "internal_error", message: "Database connection failed" } },
            { status: 500 }
          );
        }),
      ],
    },
  },
};

export const ManyKeys: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/organizations", () => {
          return HttpResponse.json(mockOrgsResponse);
        }),
        http.get("*/admin/v1/organizations/acme-corp/api-keys", () => {
          const manyKeys: ApiKey[] = Array.from({ length: 25 }, (_, i) => ({
            id: `key-${i + 1}`,
            name: `API Key ${i + 1}`,
            key_prefix: `hdr_k${i + 1}_${Math.random().toString(36).slice(2, 8)}`,
            owner:
              i % 3 === 0
                ? { type: "organization" as const, org_id: "org-123" }
                : { type: "user" as const, user_id: `usr-${i}` },
            budget_limit_cents: i % 4 === 0 ? (i + 1) * 1000 : null,
            budget_period: i % 4 === 0 ? ("monthly" as const) : null,
            revoked_at: i % 5 === 0 ? "2024-05-01T12:00:00Z" : null,
            expires_at: i % 6 === 0 ? "2025-12-31T23:59:59Z" : null,
            last_used_at: i % 2 === 0 ? new Date(2024, 5, i + 1).toISOString() : null,
            created_at: new Date(2024, 0, i + 1).toISOString(),
          }));
          return HttpResponse.json({
            data: manyKeys,
            pagination: {
              limit: 25,
              has_more: true,
              next_cursor: "bW9ja19jdXJzb3I=",
            },
          });
        }),
      ],
    },
  },
};
