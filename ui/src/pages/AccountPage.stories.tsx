import type { Meta, StoryObj } from "@storybook/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter, Routes, Route } from "react-router-dom";
import { http, HttpResponse } from "msw";
import AccountPage from "./AccountPage";
import { AuthContext } from "@/auth";
import type { AuthContextValue } from "@/auth";
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

const mockAuthValue: AuthContextValue = {
  isAuthenticated: true,
  isLoading: false,
  user: {
    id: "usr-001",
    email: "alice@acme-corp.com",
    name: "Alice Johnson",
    roles: ["super_admin"],
  },
  method: "oidc",
  token: "mock-token",
  login: async () => {},
  logout: () => {},
  setApiKey: () => {},
};

const mockAuthNoName: AuthContextValue = {
  ...mockAuthValue,
  user: {
    id: "usr-002",
    email: "bob@acme-corp.com",
    roles: [],
  },
};

const mockExportData = {
  user: {
    id: "usr-001",
    external_id: "auth0|alice",
    name: "Alice Johnson",
    email: "alice@acme-corp.com",
  },
  api_keys: [],
  conversations: [],
  usage_summary: {
    total_cost: 12.45,
    total_tokens: 125000,
    request_count: 84,
  },
  memberships: {
    organizations: [],
    projects: [],
  },
  audit_logs: [],
  sessions: [],
  exported_at: "2024-06-15T14:30:00Z",
};

const mockSessions = {
  data: [
    {
      id: "sess-001",
      device: {
        user_agent: "Mozilla/5.0 (X11; Linux x86_64) Chrome/120",
        ip_address: "192.168.1.42",
        device_description: "Chrome 120 on Linux",
      },
      created_at: new Date(Date.now() - 2 * 60 * 60 * 1000).toISOString(),
      last_activity: new Date(Date.now() - 5 * 60 * 1000).toISOString(),
      expires_at: new Date(Date.now() + 22 * 60 * 60 * 1000).toISOString(),
    },
    {
      id: "sess-002",
      device: {
        user_agent: "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0)",
        ip_address: "10.0.0.5",
        device_description: "Safari on iPhone",
      },
      created_at: new Date(Date.now() - 24 * 60 * 60 * 1000).toISOString(),
      last_activity: new Date(Date.now() - 3 * 60 * 60 * 1000).toISOString(),
      expires_at: new Date(Date.now() + 12 * 60 * 60 * 1000).toISOString(),
    },
  ],
  enhanced_enabled: true,
  current_session_id: "sess-001",
};

const sessionsHandler = http.get("*/admin/v1/me/sessions", () => HttpResponse.json(mockSessions));

const sessionsDisabledHandler = http.get("*/admin/v1/me/sessions", () =>
  HttpResponse.json({ data: [], enhanced_enabled: false })
);

const meta: Meta<typeof AccountPage> = {
  title: "Pages/AccountPage",
  component: AccountPage,
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
        <AuthContext.Provider value={mockAuthValue}>
          <ToastProvider>
            <ConfirmDialogProvider>
              <MemoryRouter initialEntries={["/account"]}>
                <Routes>
                  <Route path="/account" element={<Story />} />
                </Routes>
              </MemoryRouter>
            </ConfirmDialogProvider>
          </ToastProvider>
        </AuthContext.Provider>
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
        sessionsHandler,
        http.get("*/admin/v1/me/export", () => {
          return HttpResponse.json(mockExportData);
        }),
        http.delete("*/admin/v1/me", () => {
          return HttpResponse.json({
            deleted: true,
            user_id: "usr-001",
            conversations_deleted: 12,
            api_keys_deleted: 3,
            dynamic_providers_deleted: 1,
            usage_records_deleted: 84,
          });
        }),
        http.delete("*/admin/v1/me/sessions/:sessionId", () => {
          return HttpResponse.json({ sessions_revoked: 1 });
        }),
      ],
    },
  },
};

export const Loading: Story = {
  parameters: {
    msw: {
      handlers: [
        sessionsHandler,
        http.get("*/admin/v1/me/export", async () => {
          await new Promise((resolve) => setTimeout(resolve, 100000));
          return HttpResponse.json(mockExportData);
        }),
      ],
    },
  },
};

export const MinimalProfile: Story = {
  decorators: [
    (Story) => (
      <AuthContext.Provider value={mockAuthNoName}>
        <Story />
      </AuthContext.Provider>
    ),
  ],
  parameters: {
    msw: {
      handlers: [
        sessionsDisabledHandler,
        http.get("*/admin/v1/me/export", () => {
          return HttpResponse.json(mockExportData);
        }),
        http.delete("*/admin/v1/me", () => {
          return HttpResponse.json({
            deleted: true,
            user_id: "usr-002",
            conversations_deleted: 0,
            api_keys_deleted: 0,
            dynamic_providers_deleted: 0,
            usage_records_deleted: 0,
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
        sessionsHandler,
        http.get("*/admin/v1/me/export", () => {
          return HttpResponse.json(
            { error: { code: "internal_error", message: "Failed to export data" } },
            { status: 500 }
          );
        }),
        http.delete("*/admin/v1/me", () => {
          return HttpResponse.json(
            { error: { code: "internal_error", message: "Failed to delete account" } },
            { status: 500 }
          );
        }),
      ],
    },
  },
};
