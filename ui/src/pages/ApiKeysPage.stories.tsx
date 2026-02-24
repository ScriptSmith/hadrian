import type { Meta, StoryObj } from "@storybook/react";
import { http, HttpResponse } from "msw";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter } from "react-router-dom";
import ApiKeysPage from "./ApiKeysPage";
import { ToastProvider } from "@/components/Toast/Toast";
import { ConfirmDialogProvider } from "@/components/ConfirmDialog/ConfirmDialog";

const mockMyKeys = {
  data: [
    {
      id: "key-my-1",
      name: "Personal Dev Key",
      key_prefix: "gw_dev_abc",
      owner: { type: "user", user_id: "user-1" },
      budget_limit_cents: 5000,
      budget_period: "monthly",
      created_at: "2024-02-01T00:00:00Z",
      last_used_at: "2024-03-10T14:30:00Z",
      revoked_at: null,
      expires_at: null,
    },
    {
      id: "key-my-2",
      name: "Testing Key",
      key_prefix: "gw_test_xyz",
      owner: { type: "user", user_id: "user-1" },
      budget_limit_cents: null,
      budget_period: null,
      created_at: "2024-03-01T00:00:00Z",
      last_used_at: null,
      revoked_at: null,
      expires_at: "2025-12-31T23:59:59Z",
    },
  ],
  pagination: { has_more: false, next_cursor: null, prev_cursor: null, limit: 100 },
};

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

const mockOrgApiKeys = {
  data: [
    {
      id: "key-org-1",
      name: "Production API Key",
      key_prefix: "gw_prod_abc",
      owner: { type: "organization", org_id: "org-1" },
      budget_limit_cents: 10000,
      budget_period: "monthly",
      created_at: "2024-01-15T00:00:00Z",
      last_used_at: "2024-03-10T14:30:00Z",
      revoked_at: null,
      expires_at: null,
    },
    {
      id: "key-team-1",
      name: "Engineering Team Key",
      key_prefix: "gw_eng_def",
      owner: { type: "team", team_id: "team-1" },
      budget_limit_cents: 5000,
      budget_period: "monthly",
      created_at: "2024-01-20T00:00:00Z",
      last_used_at: "2024-03-05T10:00:00Z",
      revoked_at: null,
      expires_at: null,
    },
    {
      id: "key-proj-1",
      name: "Project Key",
      key_prefix: "gw_proj_ghi",
      owner: { type: "project", project_id: "proj-1" },
      budget_limit_cents: null,
      budget_period: null,
      created_at: "2024-02-10T00:00:00Z",
      last_used_at: null,
      revoked_at: null,
      expires_at: null,
    },
    {
      id: "key-sa-1",
      name: "CI/CD Pipeline Key",
      key_prefix: "gw_cicd_jkl",
      owner: { type: "service_account", service_account_id: "sa-1" },
      budget_limit_cents: 20000,
      budget_period: "monthly",
      created_at: "2024-01-05T00:00:00Z",
      last_used_at: "2024-03-11T08:00:00Z",
      revoked_at: null,
      expires_at: null,
    },
    ...mockMyKeys.data,
  ],
  pagination: { has_more: false, next_cursor: null, prev_cursor: null, limit: 100 },
};

const defaultHandlers = [
  http.get("*/api/admin/v1/me/api-keys", () => {
    return HttpResponse.json(mockMyKeys);
  }),
  http.get("*/api/admin/v1/organizations", () => {
    return HttpResponse.json(mockOrganizations);
  }),
  http.get("*/api/admin/v1/organizations/acme-corp/api-keys", () => {
    return HttpResponse.json(mockOrgApiKeys);
  }),
];

const meta: Meta<typeof ApiKeysPage> = {
  title: "Pages/ApiKeysPage",
  component: ApiKeysPage,
  decorators: [
    (Story) => {
      const queryClient = new QueryClient({
        defaultOptions: { queries: { retry: false } },
      });
      return (
        <QueryClientProvider client={queryClient}>
          <MemoryRouter>
            <ToastProvider>
              <ConfirmDialogProvider>
                <Story />
              </ConfirmDialogProvider>
            </ToastProvider>
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
type Story = StoryObj<typeof ApiKeysPage>;

export const Default: Story = {};

export const Loading: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/api/admin/v1/me/api-keys", async () => {
          await new Promise((resolve) => setTimeout(resolve, 999999));
          return HttpResponse.json(mockMyKeys);
        }),
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
        http.get("*/api/admin/v1/me/api-keys", () => {
          return HttpResponse.json({
            data: [],
            pagination: { has_more: false, next_cursor: null, prev_cursor: null, limit: 100 },
          });
        }),
        http.get("*/api/admin/v1/organizations", () => {
          return HttpResponse.json(mockOrganizations);
        }),
        http.get("*/api/admin/v1/organizations/*/api-keys", () => {
          return HttpResponse.json({
            data: [],
            pagination: { has_more: false, next_cursor: null, prev_cursor: null, limit: 100 },
          });
        }),
      ],
    },
  },
};

export const MyKeysOnly: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/api/admin/v1/me/api-keys", () => {
          return HttpResponse.json(mockMyKeys);
        }),
        http.get("*/api/admin/v1/organizations", () => {
          return HttpResponse.json({ data: [], total: 0 });
        }),
      ],
    },
  },
};

export const WithRevokedKeys: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/api/admin/v1/me/api-keys", () => {
          return HttpResponse.json({
            data: [
              {
                ...mockMyKeys.data[0],
                revoked_at: "2024-03-01T00:00:00Z",
              },
              mockMyKeys.data[1],
            ],
            pagination: { has_more: false, next_cursor: null, prev_cursor: null, limit: 100 },
          });
        }),
        http.get("*/api/admin/v1/organizations", () => {
          return HttpResponse.json(mockOrganizations);
        }),
        http.get("*/api/admin/v1/organizations/acme-corp/api-keys", () => {
          return HttpResponse.json({
            data: [],
            pagination: { has_more: false, next_cursor: null, prev_cursor: null, limit: 100 },
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
        http.get("*/api/admin/v1/me/api-keys", () => {
          return HttpResponse.error();
        }),
        http.get("*/api/admin/v1/organizations", () => {
          return HttpResponse.error();
        }),
      ],
    },
  },
};
