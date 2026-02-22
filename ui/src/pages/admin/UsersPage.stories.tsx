import type { Meta, StoryObj } from "@storybook/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter, Routes, Route } from "react-router-dom";
import { http, HttpResponse } from "msw";
import UsersPage from "./UsersPage";
import type { User, UserListResponse } from "@/api/generated/types.gen";
import { ToastProvider } from "@/components/Toast/Toast";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: false,
      staleTime: Infinity,
    },
  },
});

const mockUsers: User[] = [
  {
    id: "usr-001",
    external_id: "auth0|alice",
    name: "Alice Johnson",
    email: "alice@acme-corp.com",
    created_at: "2024-01-10T09:00:00Z",
    updated_at: "2024-06-15T14:30:00Z",
  },
  {
    id: "usr-002",
    external_id: "auth0|bob",
    name: "Bob Martinez",
    email: "bob@acme-corp.com",
    created_at: "2024-02-14T11:20:00Z",
    updated_at: "2024-02-14T11:20:00Z",
  },
  {
    id: "usr-003",
    external_id: "okta|charlie",
    name: null,
    email: "charlie@stark-industries.com",
    created_at: "2024-03-05T16:45:00Z",
    updated_at: "2024-05-20T08:10:00Z",
  },
  {
    id: "usr-004",
    external_id: "saml|diana",
    name: "Diana Chen",
    email: null,
    created_at: "2024-04-22T13:00:00Z",
    updated_at: "2024-04-22T13:00:00Z",
  },
  {
    id: "usr-005",
    external_id: "oidc|eve",
    name: null,
    email: null,
    created_at: "2024-05-01T07:30:00Z",
    updated_at: "2024-05-01T07:30:00Z",
  },
];

const mockUsersResponse: UserListResponse = {
  data: mockUsers,
  pagination: {
    limit: 25,
    has_more: false,
  },
};

const emptyUsersResponse: UserListResponse = {
  data: [],
  pagination: {
    limit: 25,
    has_more: false,
  },
};

const meta: Meta<typeof UsersPage> = {
  title: "Admin/UsersPage",
  component: UsersPage,
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
          <MemoryRouter initialEntries={["/admin/users"]}>
            <Routes>
              <Route path="/admin/users" element={<Story />} />
            </Routes>
          </MemoryRouter>
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
        http.get("*/admin/v1/users", () => {
          return HttpResponse.json(mockUsersResponse);
        }),
        http.post("*/admin/v1/users", async ({ request }) => {
          const body = (await request.json()) as Record<string, unknown>;
          const newUser: User = {
            id: `usr-${Date.now()}`,
            external_id: body.external_id as string,
            name: (body.name as string) || null,
            email: (body.email as string) || null,
            created_at: new Date().toISOString(),
            updated_at: new Date().toISOString(),
          };
          return HttpResponse.json(newUser, { status: 201 });
        }),
        http.patch("*/admin/v1/users/:userId", async ({ request }) => {
          const body = (await request.json()) as Record<string, unknown>;
          const updated: User = {
            ...mockUsers[0],
            name: (body.name as string) ?? mockUsers[0].name,
            email: (body.email as string) ?? mockUsers[0].email,
            updated_at: new Date().toISOString(),
          };
          return HttpResponse.json(updated);
        }),
      ],
    },
  },
};

export const Loading: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/users", async () => {
          await new Promise((resolve) => setTimeout(resolve, 100000));
          return HttpResponse.json(mockUsersResponse);
        }),
      ],
    },
  },
};

export const Empty: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/users", () => {
          return HttpResponse.json(emptyUsersResponse);
        }),
        http.post("*/admin/v1/users", async ({ request }) => {
          const body = (await request.json()) as Record<string, unknown>;
          const newUser: User = {
            id: `usr-${Date.now()}`,
            external_id: body.external_id as string,
            name: (body.name as string) || null,
            email: (body.email as string) || null,
            created_at: new Date().toISOString(),
            updated_at: new Date().toISOString(),
          };
          return HttpResponse.json(newUser, { status: 201 });
        }),
      ],
    },
  },
};

export const Error: Story = {
  parameters: {
    msw: {
      handlers: [
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

export const ManyUsers: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/users", () => {
          const manyUsers: User[] = Array.from({ length: 25 }, (_, i) => ({
            id: `usr-${i + 1}`,
            external_id: `sso|user-${i + 1}`,
            name: i % 4 === 0 ? null : `User ${i + 1}`,
            email: i % 3 === 0 ? null : `user${i + 1}@acme-corp.com`,
            created_at: new Date(2024, 0, i + 1).toISOString(),
            updated_at: new Date(2024, 5, i + 1).toISOString(),
          }));
          return HttpResponse.json({
            data: manyUsers,
            pagination: {
              limit: 25,
              has_more: true,
              next_cursor: "bW9ja19jdXJzb3I=",
            },
          });
        }),
        http.post("*/admin/v1/users", async ({ request }) => {
          const body = (await request.json()) as Record<string, unknown>;
          const newUser: User = {
            id: `usr-${Date.now()}`,
            external_id: body.external_id as string,
            name: (body.name as string) || null,
            email: (body.email as string) || null,
            created_at: new Date().toISOString(),
            updated_at: new Date().toISOString(),
          };
          return HttpResponse.json(newUser, { status: 201 });
        }),
        http.patch("*/admin/v1/users/:userId", async ({ request }) => {
          const body = (await request.json()) as Record<string, unknown>;
          const updated: User = {
            ...mockUsers[0],
            name: (body.name as string) ?? mockUsers[0].name,
            email: (body.email as string) ?? mockUsers[0].email,
            updated_at: new Date().toISOString(),
          };
          return HttpResponse.json(updated);
        }),
      ],
    },
  },
};
