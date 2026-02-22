import type { Meta, StoryObj } from "@storybook/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter, Routes, Route } from "react-router-dom";
import { http, HttpResponse } from "msw";
import TeamDetailPage from "./TeamDetailPage";
import type { Team, TeamMember, User } from "@/api/generated/types.gen";
import { ToastProvider } from "@/components/Toast/Toast";
import { ConfirmDialogProvider } from "@/components/ConfirmDialog/ConfirmDialog";

const createQueryClient = () =>
  new QueryClient({
    defaultOptions: {
      queries: {
        retry: false,
        staleTime: Infinity,
      },
    },
  });

const mockTeam: Team = {
  id: "team-abc",
  name: "Platform Engineering",
  slug: "platform-engineering",
  org_id: "org-123",
  created_at: "2024-02-01T09:00:00Z",
  updated_at: "2024-06-15T10:30:00Z",
};

const mockMembers: TeamMember[] = [
  {
    user_id: "usr-001",
    external_id: "auth0|alice",
    name: "Alice Johnson",
    email: "alice@acme-corp.com",
    role: "admin",
    joined_at: "2024-02-01T09:00:00Z",
  },
  {
    user_id: "usr-002",
    external_id: "auth0|bob",
    name: "Bob Martinez",
    email: "bob@acme-corp.com",
    role: "member",
    joined_at: "2024-02-14T11:20:00Z",
  },
  {
    user_id: "usr-003",
    external_id: "okta|charlie",
    name: null,
    email: "charlie@acme-corp.com",
    role: "member",
    joined_at: "2024-03-05T16:45:00Z",
  },
  {
    user_id: "usr-006",
    external_id: "saml|frank",
    name: "Frank Wilson",
    email: null,
    role: "viewer",
    joined_at: "2024-05-10T14:00:00Z",
  },
];

const mockAllUsers: User[] = [
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
    email: "charlie@acme-corp.com",
    created_at: "2024-03-05T16:45:00Z",
    updated_at: "2024-05-20T08:10:00Z",
  },
  {
    id: "usr-004",
    external_id: "saml|diana",
    name: "Diana Chen",
    email: "diana@acme-corp.com",
    created_at: "2024-04-22T13:00:00Z",
    updated_at: "2024-04-22T13:00:00Z",
  },
  {
    id: "usr-005",
    external_id: "oidc|eve",
    name: "Eve Taylor",
    email: "eve@acme-corp.com",
    created_at: "2024-05-01T07:30:00Z",
    updated_at: "2024-05-01T07:30:00Z",
  },
  {
    id: "usr-006",
    external_id: "saml|frank",
    name: "Frank Wilson",
    email: null,
    created_at: "2024-05-10T14:00:00Z",
    updated_at: "2024-05-10T14:00:00Z",
  },
];

const orgSlug = "acme-corp";
const teamSlug = "platform-engineering";

const createDecorator = () => (Story: React.ComponentType) => {
  const queryClient = createQueryClient();
  return (
    <QueryClientProvider client={queryClient}>
      <ToastProvider>
        <ConfirmDialogProvider>
          <MemoryRouter initialEntries={[`/admin/organizations/${orgSlug}/teams/${teamSlug}`]}>
            <Routes>
              <Route path="/admin/organizations/:orgSlug/teams/:teamSlug" element={<Story />} />
              <Route path="/admin/teams" element={<div>Teams List Page</div>} />
            </Routes>
          </MemoryRouter>
        </ConfirmDialogProvider>
      </ToastProvider>
    </QueryClientProvider>
  );
};

const defaultHandlers = [
  http.get("*/admin/v1/organizations/:orgSlug/teams/:teamSlug", () => {
    return HttpResponse.json(mockTeam);
  }),
  http.get("*/admin/v1/organizations/:orgSlug/teams/:teamSlug/members", () => {
    return HttpResponse.json({
      data: mockMembers,
      pagination: { limit: 100, has_more: false },
    });
  }),
  http.get("*/admin/v1/users", () => {
    return HttpResponse.json({
      data: mockAllUsers,
      pagination: { limit: 100, has_more: false },
    });
  }),
  http.patch("*/admin/v1/organizations/:orgSlug/teams/:teamSlug", async ({ request }) => {
    const body = (await request.json()) as Record<string, unknown>;
    return HttpResponse.json({ ...mockTeam, ...body, updated_at: new Date().toISOString() });
  }),
  http.post("*/admin/v1/organizations/:orgSlug/teams/:teamSlug/members", () => {
    return HttpResponse.json({}, { status: 201 });
  }),
  http.delete("*/admin/v1/organizations/:orgSlug/teams/:teamSlug/members/:userId", () => {
    return HttpResponse.json({});
  }),
  // Usage endpoints
  http.get("*/admin/v1/organizations/:orgSlug/teams/:teamSlug/usage", () => {
    return HttpResponse.json({
      total_requests: 4200,
      total_input_tokens: 800000,
      total_output_tokens: 420000,
      total_cost_microcents: 1200000000,
    });
  }),
  http.get("*/admin/v1/organizations/:orgSlug/teams/:teamSlug/usage/*", () => {
    return HttpResponse.json({ data: [] });
  }),
];

const meta: Meta<typeof TeamDetailPage> = {
  title: "Admin/TeamDetailPage",
  component: TeamDetailPage,
  parameters: {
    layout: "fullscreen",
    a11y: {
      config: {
        rules: [{ id: "heading-order", enabled: false }],
      },
    },
  },
};

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  decorators: [createDecorator()],
  parameters: {
    msw: {
      handlers: defaultHandlers,
    },
  },
};

export const Loading: Story = {
  decorators: [createDecorator()],
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/organizations/:orgSlug/teams/:teamSlug", async () => {
          await new Promise((resolve) => setTimeout(resolve, 100000));
          return HttpResponse.json(mockTeam);
        }),
      ],
    },
  },
};

export const Empty: Story = {
  decorators: [createDecorator()],
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/organizations/:orgSlug/teams/:teamSlug", () => {
          return HttpResponse.json(mockTeam);
        }),
        http.get("*/admin/v1/organizations/:orgSlug/teams/:teamSlug/members", () => {
          return HttpResponse.json({
            data: [],
            pagination: { limit: 100, has_more: false },
          });
        }),
        http.get("*/admin/v1/users", () => {
          return HttpResponse.json({
            data: mockAllUsers,
            pagination: { limit: 100, has_more: false },
          });
        }),
        http.get("*/admin/v1/organizations/:orgSlug/teams/:teamSlug/usage", () => {
          return HttpResponse.json({
            total_requests: 0,
            total_input_tokens: 0,
            total_output_tokens: 0,
            total_cost_microcents: 0,
          });
        }),
        http.get("*/admin/v1/organizations/:orgSlug/teams/:teamSlug/usage/*", () => {
          return HttpResponse.json({ data: [] });
        }),
      ],
    },
  },
};

export const Error: Story = {
  decorators: [createDecorator()],
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/organizations/:orgSlug/teams/:teamSlug", () => {
          return HttpResponse.json(
            { error: { code: "not_found", message: "Team not found" } },
            { status: 404 }
          );
        }),
      ],
    },
  },
};

export const ManyMembers: Story = {
  decorators: [createDecorator()],
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/organizations/:orgSlug/teams/:teamSlug", () => {
          return HttpResponse.json(mockTeam);
        }),
        http.get("*/admin/v1/organizations/:orgSlug/teams/:teamSlug/members", () => {
          const manyMembers: TeamMember[] = Array.from({ length: 25 }, (_, i) => ({
            user_id: `usr-${i + 1}`,
            external_id: `sso|member-${i + 1}`,
            name: i % 5 === 0 ? null : `Team Member ${i + 1}`,
            email: i % 4 === 0 ? null : `member${i + 1}@acme-corp.com`,
            role: i === 0 ? "admin" : i < 5 ? "member" : "viewer",
            joined_at: new Date(2024, 0, i + 1).toISOString(),
          }));
          return HttpResponse.json({
            data: manyMembers,
            pagination: { limit: 100, has_more: false },
          });
        }),
        http.get("*/admin/v1/users", () => {
          return HttpResponse.json({
            data: mockAllUsers,
            pagination: { limit: 100, has_more: false },
          });
        }),
        http.get("*/admin/v1/organizations/:orgSlug/teams/:teamSlug/usage", () => {
          return HttpResponse.json({
            total_requests: 4200,
            total_input_tokens: 800000,
            total_output_tokens: 420000,
            total_cost_microcents: 1200000000,
          });
        }),
        http.get("*/admin/v1/organizations/:orgSlug/teams/:teamSlug/usage/*", () => {
          return HttpResponse.json({ data: [] });
        }),
      ],
    },
  },
};
