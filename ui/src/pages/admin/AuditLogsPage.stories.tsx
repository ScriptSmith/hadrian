import type { Meta, StoryObj } from "@storybook/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter, Routes, Route } from "react-router-dom";
import { http, HttpResponse } from "msw";
import AuditLogsPage from "./AuditLogsPage";
import type { AuditLog, AuditLogListResponse } from "@/api/generated/types.gen";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: false,
      staleTime: Infinity,
    },
  },
});

const mockAuditLogs: AuditLog[] = [
  {
    id: "log-001",
    action: "api_key.create",
    actor_type: "user",
    actor_id: "usr-001-alice-johnson",
    resource_type: "api_key",
    resource_id: "key-001-production",
    details: { name: "Production API Key" },
    org_id: "org-123",
    project_id: null,
    ip_address: "10.0.1.42",
    timestamp: "2024-06-15T14:30:00Z",
  },
  {
    id: "log-002",
    action: "user.update",
    actor_type: "user",
    actor_id: "usr-002-bob-martinez",
    resource_type: "user",
    resource_id: "usr-003-charlie-dev",
    details: { name: "Charlie Chen" },
    org_id: "org-123",
    project_id: "proj-001",
    ip_address: "192.168.1.100",
    timestamp: "2024-06-15T13:15:00Z",
  },
  {
    id: "log-003",
    action: "api_key.revoke",
    actor_type: "api_key",
    actor_id: "key-admin-abcdef12",
    resource_type: "api_key",
    resource_id: "key-004-deprecated",
    details: {},
    org_id: "org-123",
    project_id: null,
    ip_address: "172.16.0.5",
    timestamp: "2024-06-15T12:00:00Z",
  },
  {
    id: "log-004",
    action: "organization.create",
    actor_type: "system",
    actor_id: null,
    resource_type: "organization",
    resource_id: "org-456-stark-ind",
    details: { slug: "stark-industries" },
    org_id: null,
    project_id: null,
    ip_address: null,
    timestamp: "2024-06-14T09:00:00Z",
  },
  {
    id: "log-005",
    action: "project.delete",
    actor_type: "user",
    actor_id: "usr-001-alice-johnson",
    resource_type: "project",
    resource_id: "proj-old-sandbox",
    details: { name: "Old Sandbox" },
    org_id: "org-123",
    project_id: "proj-old-sandbox",
    ip_address: "10.0.1.42",
    timestamp: "2024-06-14T08:30:00Z",
  },
  {
    id: "log-006",
    action: "service_account.create",
    actor_type: "user",
    actor_id: "usr-002-bob-martinez",
    resource_type: "service_account",
    resource_id: "sa-1-ci-cd-bot",
    details: { slug: "ci-cd-bot", name: "CI/CD Bot" },
    org_id: "org-123",
    project_id: null,
    ip_address: "192.168.1.100",
    timestamp: "2024-06-13T16:45:00Z",
  },
];

const mockAuditLogsResponse: AuditLogListResponse = {
  data: mockAuditLogs,
  pagination: {
    limit: 50,
    has_more: false,
  },
};

const emptyAuditLogsResponse: AuditLogListResponse = {
  data: [],
  pagination: {
    limit: 50,
    has_more: false,
  },
};

const meta: Meta<typeof AuditLogsPage> = {
  title: "Admin/AuditLogsPage",
  component: AuditLogsPage,
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
        <MemoryRouter initialEntries={["/admin/audit-logs"]}>
          <Routes>
            <Route path="/admin/audit-logs" element={<Story />} />
          </Routes>
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
      handlers: [
        http.get("*/admin/v1/audit-logs", () => {
          return HttpResponse.json(mockAuditLogsResponse);
        }),
      ],
    },
  },
};

export const Loading: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/audit-logs", async () => {
          await new Promise((resolve) => setTimeout(resolve, 100000));
          return HttpResponse.json(mockAuditLogsResponse);
        }),
      ],
    },
  },
};

export const Empty: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/audit-logs", () => {
          return HttpResponse.json(emptyAuditLogsResponse);
        }),
      ],
    },
  },
};

export const Error: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/audit-logs", () => {
          return HttpResponse.json(
            { error: { code: "internal_error", message: "Database connection failed" } },
            { status: 500 }
          );
        }),
      ],
    },
  },
};

export const ManyLogs: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/audit-logs", () => {
          const actions = [
            "api_key.create",
            "api_key.revoke",
            "user.update",
            "user.create",
            "organization.create",
            "project.delete",
            "project.create",
            "service_account.create",
            "team.update",
          ];
          const actorTypes = ["user", "api_key", "system"] as const;
          const resourceTypes = [
            "api_key",
            "user",
            "organization",
            "project",
            "service_account",
            "team",
          ];

          const manyLogs: AuditLog[] = Array.from({ length: 50 }, (_, i) => ({
            id: `log-${i + 1}`,
            action: actions[i % actions.length],
            actor_type: actorTypes[i % actorTypes.length],
            actor_id: i % 3 === 2 ? null : `actor-${i + 1}-abcdef12`,
            resource_type: resourceTypes[i % resourceTypes.length],
            resource_id: `res-${i + 1}-abcdef12`,
            details: i % 2 === 0 ? { name: `Resource ${i + 1}` } : {},
            org_id: i % 4 === 0 ? null : "org-123",
            project_id: i % 5 === 0 ? `proj-${i + 1}` : null,
            ip_address: i % 3 === 2 ? null : `10.0.${Math.floor(i / 10)}.${(i % 254) + 1}`,
            timestamp: new Date(2024, 5, 15, 23 - Math.floor(i / 4), 59 - (i % 60)).toISOString(),
          }));

          return HttpResponse.json({
            data: manyLogs,
            pagination: {
              limit: 50,
              has_more: true,
              next_cursor: "bW9ja19jdXJzb3I=",
            },
          });
        }),
      ],
    },
  },
};
