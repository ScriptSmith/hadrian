import type { Meta, StoryObj } from "@storybook/react";
import { http, HttpResponse } from "msw";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter } from "react-router-dom";
import ApiKeysPage from "./ApiKeysPage";
import { ToastProvider } from "@/components/Toast/Toast";
import { ConfirmDialogProvider } from "@/components/ConfirmDialog/ConfirmDialog";

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

const mockApiKeys = {
  data: [
    {
      id: "key-1",
      name: "Production API Key",
      key_prefix: "sk-prod-abc",
      owner: { type: "organization", org_id: "org-1" },
      budget_limit_cents: 10000,
      budget_period: "monthly",
      created_at: "2024-01-15T00:00:00Z",
      last_used_at: "2024-03-10T14:30:00Z",
      revoked_at: null,
      expires_at: null,
    },
    {
      id: "key-2",
      name: "Development Key",
      key_prefix: "sk-dev-xyz",
      owner: { type: "project", project_id: "proj-1" },
      budget_limit_cents: null,
      budget_period: null,
      created_at: "2024-02-01T00:00:00Z",
      last_used_at: null,
      revoked_at: null,
      expires_at: "2024-12-31T23:59:59Z",
    },
    {
      id: "key-3",
      name: "Old API Key",
      key_prefix: "sk-old-123",
      owner: { type: "team", team_id: "team-1" },
      budget_limit_cents: 5000,
      budget_period: "weekly",
      created_at: "2023-06-01T00:00:00Z",
      last_used_at: "2023-12-15T09:00:00Z",
      revoked_at: "2024-01-01T00:00:00Z",
      expires_at: null,
    },
  ],
  pagination: {
    has_more: false,
    next_cursor: null,
    prev_cursor: null,
  },
};

const defaultHandlers = [
  http.get("*/api/admin/v1/organizations", () => {
    return HttpResponse.json(mockOrganizations);
  }),
  http.get("*/api/admin/v1/organizations/acme-corp/api-keys", () => {
    return HttpResponse.json(mockApiKeys);
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
        http.get("*/api/admin/v1/organizations/*/api-keys", () => {
          return HttpResponse.json({
            data: [],
            pagination: { has_more: false, next_cursor: null, prev_cursor: null },
          });
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

export const WithRevokedKeys: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/api/admin/v1/organizations", () => {
          return HttpResponse.json(mockOrganizations);
        }),
        http.get("*/api/admin/v1/organizations/acme-corp/api-keys", () => {
          return HttpResponse.json({
            data: [
              {
                id: "key-active",
                name: "Active Key",
                key_prefix: "sk-active",
                owner: { type: "organization", org_id: "org-1" },
                budget_limit_cents: 5000,
                budget_period: "monthly",
                created_at: "2024-01-15T00:00:00Z",
                last_used_at: "2024-03-10T14:30:00Z",
                revoked_at: null,
                expires_at: null,
              },
              {
                id: "key-revoked-1",
                name: "Revoked Key 1",
                key_prefix: "sk-rev1",
                owner: { type: "project", project_id: "proj-1" },
                budget_limit_cents: null,
                budget_period: null,
                created_at: "2024-01-01T00:00:00Z",
                last_used_at: "2024-01-15T00:00:00Z",
                revoked_at: "2024-02-01T00:00:00Z",
                expires_at: null,
              },
              {
                id: "key-revoked-2",
                name: "Revoked Key 2",
                key_prefix: "sk-rev2",
                owner: { type: "team", team_id: "team-1" },
                budget_limit_cents: 1000,
                budget_period: "weekly",
                created_at: "2023-12-01T00:00:00Z",
                last_used_at: null,
                revoked_at: "2024-01-15T00:00:00Z",
                expires_at: null,
              },
            ],
            pagination: { has_more: false, next_cursor: null, prev_cursor: null },
          });
        }),
      ],
    },
  },
};

export const ManyKeys: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/api/admin/v1/organizations", () => {
          return HttpResponse.json(mockOrganizations);
        }),
        http.get("*/api/admin/v1/organizations/acme-corp/api-keys", () => {
          return HttpResponse.json({
            data: Array.from({ length: 9 }, (_, i) => ({
              id: `key-${i + 1}`,
              name: `API Key ${i + 1}`,
              key_prefix: `sk-key${i + 1}`,
              owner:
                i % 3 === 0
                  ? { type: "organization", org_id: "org-1" }
                  : i % 3 === 1
                    ? { type: "project", project_id: `proj-${i}` }
                    : { type: "team", team_id: `team-${i}` },
              budget_limit_cents: i % 2 === 0 ? (i + 1) * 1000 : null,
              budget_period: i % 2 === 0 ? "monthly" : null,
              created_at: new Date(2024, 0, i + 1).toISOString(),
              last_used_at: i % 3 === 0 ? new Date(2024, 2, i + 1).toISOString() : null,
              revoked_at: i === 8 ? new Date(2024, 2, 1).toISOString() : null,
              expires_at: i === 2 ? new Date(2024, 11, 31).toISOString() : null,
            })),
            pagination: { has_more: false, next_cursor: null, prev_cursor: null },
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
