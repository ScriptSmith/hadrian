import type { Meta, StoryObj } from "@storybook/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter, Routes, Route } from "react-router-dom";
import { http, HttpResponse } from "msw";
import ServiceAccountsPage from "./ServiceAccountsPage";
import type {
  Organization,
  ServiceAccount,
  ServiceAccountListResponse,
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
  },
  {
    id: "org-456",
    slug: "stark-industries",
    name: "Stark Industries",
    created_at: "2024-02-01T00:00:00Z",
  },
];

const mockOrgsResponse: OrganizationListResponse = {
  data: mockOrgs,
  pagination: {
    limit: 100,
    has_more: false,
  },
};

const mockServiceAccounts: ServiceAccount[] = [
  {
    id: "sa-1",
    org_id: "org-123",
    slug: "ci-cd-bot",
    name: "CI/CD Bot",
    description: "Automated deployment service account",
    roles: ["deployer", "viewer"],
    created_at: "2024-01-15T00:00:00Z",
    updated_at: "2024-06-15T10:30:00Z",
  },
  {
    id: "sa-2",
    org_id: "org-123",
    slug: "monitoring-agent",
    name: "Monitoring Agent",
    description: "Prometheus metrics scraper",
    roles: ["metrics-reader"],
    created_at: "2024-02-20T00:00:00Z",
    updated_at: "2024-02-20T00:00:00Z",
  },
  {
    id: "sa-3",
    org_id: "org-123",
    slug: "backup-service",
    name: "Backup Service",
    description: "Automated backup and restore operations",
    roles: ["admin", "backup-operator"],
    created_at: "2024-03-01T00:00:00Z",
    updated_at: "2024-05-10T08:00:00Z",
  },
  {
    id: "sa-4",
    org_id: "org-123",
    slug: "api-gateway",
    name: "API Gateway",
    description: null,
    roles: [],
    created_at: "2024-04-01T00:00:00Z",
    updated_at: "2024-04-01T00:00:00Z",
  },
];

const mockServiceAccountsResponse: ServiceAccountListResponse = {
  data: mockServiceAccounts,
  pagination: {
    limit: 100,
    has_more: false,
  },
};

const emptyServiceAccountsResponse: ServiceAccountListResponse = {
  data: [],
  pagination: {
    limit: 100,
    has_more: false,
  },
};

const meta: Meta<typeof ServiceAccountsPage> = {
  title: "Admin/ServiceAccountsPage",
  component: ServiceAccountsPage,
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
            <MemoryRouter initialEntries={["/admin/service-accounts"]}>
              <Routes>
                <Route path="/admin/service-accounts" element={<Story />} />
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

export const WithServiceAccounts: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/organizations", () => {
          return HttpResponse.json(mockOrgsResponse);
        }),
        http.get("*/admin/v1/organizations/acme-corp/service-accounts", () => {
          return HttpResponse.json(mockServiceAccountsResponse);
        }),
        http.get("*/admin/v1/organizations/stark-industries/service-accounts", () => {
          return HttpResponse.json(emptyServiceAccountsResponse);
        }),
        http.post("*/admin/v1/organizations/:orgSlug/service-accounts", async ({ request }) => {
          const body = (await request.json()) as Record<string, unknown>;
          const newSa: ServiceAccount = {
            id: `sa-${Date.now()}`,
            org_id: "org-123",
            slug: body.slug as string,
            name: body.name as string,
            description: (body.description as string) || null,
            roles: (body.roles as string[]) || [],
            created_at: new Date().toISOString(),
            updated_at: new Date().toISOString(),
          };
          return HttpResponse.json(newSa, { status: 201 });
        }),
        http.patch(
          "*/admin/v1/organizations/:orgSlug/service-accounts/:saSlug",
          async ({ request }) => {
            const body = (await request.json()) as Record<string, unknown>;
            const updated: ServiceAccount = {
              ...mockServiceAccounts[0],
              name: (body.name as string) || mockServiceAccounts[0].name,
              description: (body.description as string) || mockServiceAccounts[0].description,
              roles: (body.roles as string[]) || mockServiceAccounts[0].roles,
              updated_at: new Date().toISOString(),
            };
            return HttpResponse.json(updated);
          }
        ),
        http.delete("*/admin/v1/organizations/:orgSlug/service-accounts/:saSlug", () => {
          return HttpResponse.json({});
        }),
      ],
    },
  },
};

export const NoServiceAccounts: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/organizations", () => {
          return HttpResponse.json(mockOrgsResponse);
        }),
        http.get("*/admin/v1/organizations/acme-corp/service-accounts", () => {
          return HttpResponse.json(emptyServiceAccountsResponse);
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

export const Loading: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/organizations", async () => {
          await new Promise((resolve) => setTimeout(resolve, 100000));
          return HttpResponse.json(mockOrgsResponse);
        }),
        http.get("*/admin/v1/organizations/:orgSlug/service-accounts", async () => {
          await new Promise((resolve) => setTimeout(resolve, 100000));
          return HttpResponse.json(mockServiceAccountsResponse);
        }),
      ],
    },
  },
};

export const ManyServiceAccounts: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/organizations", () => {
          return HttpResponse.json(mockOrgsResponse);
        }),
        http.get("*/admin/v1/organizations/acme-corp/service-accounts", () => {
          // Generate many service accounts for pagination testing
          const manySAs: ServiceAccount[] = Array.from({ length: 25 }, (_, i) => ({
            id: `sa-${i + 1}`,
            org_id: "org-123",
            slug: `service-account-${i + 1}`,
            name: `Service Account ${i + 1}`,
            description: i % 2 === 0 ? `Description for service account ${i + 1}` : null,
            roles: i % 3 === 0 ? [] : [`role-${i}`],
            created_at: "2024-01-01T00:00:00Z",
            updated_at: "2024-06-15T10:30:00Z",
          }));
          return HttpResponse.json({
            data: manySAs,
            pagination: {
              limit: 100,
              has_more: true,
            },
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
        http.get("*/admin/v1/organizations", () => {
          return HttpResponse.json(mockOrgsResponse);
        }),
        http.get("*/admin/v1/organizations/acme-corp/service-accounts", () => {
          return HttpResponse.json(
            { error: { code: "internal_error", message: "Database connection failed" } },
            { status: 500 }
          );
        }),
      ],
    },
  },
};
