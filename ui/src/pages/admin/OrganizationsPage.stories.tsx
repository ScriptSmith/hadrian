import type { Meta, StoryObj } from "@storybook/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter, Routes, Route } from "react-router-dom";
import { http, HttpResponse } from "msw";
import OrganizationsPage from "./OrganizationsPage";
import type { Organization, OrganizationListResponse } from "@/api/generated/types.gen";
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

const mockOrganizations: Organization[] = [
  {
    id: "org-123",
    slug: "acme-corp",
    name: "Acme Corporation",
    created_at: "2024-01-01T00:00:00Z",
    updated_at: "2024-06-15T10:00:00Z",
  },
  {
    id: "org-456",
    slug: "stark-industries",
    name: "Stark Industries",
    created_at: "2024-02-01T00:00:00Z",
    updated_at: "2024-05-20T14:30:00Z",
  },
  {
    id: "org-789",
    slug: "wayne-enterprises",
    name: "Wayne Enterprises",
    created_at: "2024-03-15T00:00:00Z",
    updated_at: "2024-03-15T00:00:00Z",
  },
  {
    id: "org-012",
    slug: "oscorp",
    name: "Oscorp",
    created_at: "2024-04-10T08:00:00Z",
    updated_at: "2024-06-01T12:00:00Z",
  },
];

const mockOrgsResponse: OrganizationListResponse = {
  data: mockOrganizations,
  pagination: {
    limit: 25,
    has_more: false,
  },
};

const emptyOrgsResponse: OrganizationListResponse = {
  data: [],
  pagination: {
    limit: 25,
    has_more: false,
  },
};

const meta: Meta<typeof OrganizationsPage> = {
  title: "Admin/OrganizationsPage",
  component: OrganizationsPage,
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
            <MemoryRouter initialEntries={["/admin/organizations"]}>
              <Routes>
                <Route path="/admin/organizations" element={<Story />} />
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
        http.post("*/admin/v1/organizations", async ({ request }) => {
          const body = (await request.json()) as Record<string, unknown>;
          const newOrg: Organization = {
            id: `org-${Date.now()}`,
            slug: body.slug as string,
            name: body.name as string,
            created_at: new Date().toISOString(),
            updated_at: new Date().toISOString(),
          };
          return HttpResponse.json(newOrg, { status: 201 });
        }),
        http.delete("*/admin/v1/organizations/:slug", () => {
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
      ],
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
        http.post("*/admin/v1/organizations", async ({ request }) => {
          const body = (await request.json()) as Record<string, unknown>;
          const newOrg: Organization = {
            id: `org-${Date.now()}`,
            slug: body.slug as string,
            name: body.name as string,
            created_at: new Date().toISOString(),
            updated_at: new Date().toISOString(),
          };
          return HttpResponse.json(newOrg, { status: 201 });
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
      ],
    },
  },
};

export const ManyOrganizations: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/organizations", () => {
          const manyOrgs: Organization[] = Array.from({ length: 25 }, (_, i) => ({
            id: `org-${i + 1}`,
            slug: `organization-${i + 1}`,
            name: `Organization ${i + 1}`,
            created_at: new Date(2024, 0, i + 1).toISOString(),
            updated_at: new Date(2024, 5, i + 1).toISOString(),
          }));
          return HttpResponse.json({
            data: manyOrgs,
            pagination: {
              limit: 25,
              has_more: true,
              next_cursor: "bW9ja19jdXJzb3I=",
            },
          });
        }),
        http.post("*/admin/v1/organizations", async ({ request }) => {
          const body = (await request.json()) as Record<string, unknown>;
          const newOrg: Organization = {
            id: `org-${Date.now()}`,
            slug: body.slug as string,
            name: body.name as string,
            created_at: new Date().toISOString(),
            updated_at: new Date().toISOString(),
          };
          return HttpResponse.json(newOrg, { status: 201 });
        }),
        http.delete("*/admin/v1/organizations/:slug", () => {
          return HttpResponse.json({});
        }),
      ],
    },
  },
};
