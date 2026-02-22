import type { Meta, StoryObj } from "@storybook/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fn } from "storybook/test";
import { HttpResponse, http } from "msw";
import { RbacPolicyVersionHistoryModal } from "./RbacPolicyVersionHistoryModal";
import type { OrgRbacPolicy, OrgRbacPolicyVersionListResponse } from "@/api/generated/types.gen";
import { ToastProvider } from "@/components/Toast/Toast";
import { ConfirmDialogProvider } from "@/components/ConfirmDialog/ConfirmDialog";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: { retry: false, staleTime: Infinity },
  },
});

const mockPolicy: OrgRbacPolicy = {
  id: "policy-1",
  org_id: "org-1",
  name: "require-admin-for-settings",
  description: "Restricts settings access to administrators only",
  resource: "settings/*",
  action: "*",
  condition: "'admin' in subject.roles",
  effect: "deny",
  priority: 100,
  enabled: true,
  version: 3,
  created_at: "2024-01-01T00:00:00Z",
  updated_at: "2024-06-15T10:30:00Z",
};

const mockVersions: OrgRbacPolicyVersionListResponse = {
  data: [
    {
      id: "version-3",
      policy_id: "policy-1",
      version: 3,
      name: "require-admin-for-settings",
      description: "Restricts settings access to administrators only",
      resource: "settings/*",
      action: "*",
      condition: "'admin' in subject.roles",
      effect: "deny",
      priority: 100,
      enabled: true,
      reason: "Updated condition to use new role structure",
      created_at: "2024-06-15T10:30:00Z",
      created_by: "admin@acme.com",
    },
    {
      id: "version-2",
      policy_id: "policy-1",
      version: 2,
      name: "require-admin-for-settings",
      description: "Restricts settings access to administrators only",
      resource: "settings/*",
      action: "*",
      condition: "subject.roles.exists(r, r == 'admin')",
      effect: "deny",
      priority: 100,
      enabled: true,
      reason: "Fixed condition syntax",
      created_at: "2024-03-10T14:20:00Z",
      created_by: "admin@acme.com",
    },
    {
      id: "version-1",
      policy_id: "policy-1",
      version: 1,
      name: "require-admin-for-settings",
      description: null,
      resource: "settings/*",
      action: "write",
      condition: "subject.role == 'admin'",
      effect: "deny",
      priority: 50,
      enabled: false,
      reason: "Initial policy creation",
      created_at: "2024-01-01T00:00:00Z",
      created_by: "setup@acme.com",
    },
  ],
  pagination: {
    offset: 0,
    limit: 20,
    total: 3,
    has_more: false,
  },
};

const meta: Meta<typeof RbacPolicyVersionHistoryModal> = {
  title: "Admin/RbacPolicyVersionHistoryModal",
  component: RbacPolicyVersionHistoryModal,
  decorators: [
    (Story) => (
      <QueryClientProvider client={queryClient}>
        <ToastProvider>
          <ConfirmDialogProvider>
            <div style={{ minHeight: "500px" }}>
              <Story />
            </div>
          </ConfirmDialogProvider>
        </ToastProvider>
      </QueryClientProvider>
    ),
  ],
  args: {
    open: true,
    onClose: fn(),
    policy: mockPolicy,
    orgSlug: "acme",
  },
  parameters: {
    msw: {
      handlers: [
        http.get("/admin/v1/organizations/:orgSlug/rbac-policies/:policyId/versions", () => {
          return HttpResponse.json(mockVersions);
        }),
        http.post("/admin/v1/organizations/:orgSlug/rbac-policies/:policyId/rollback", () => {
          return HttpResponse.json(mockPolicy);
        }),
      ],
    },
  },
};

export default meta;
type Story = StoryObj<typeof RbacPolicyVersionHistoryModal>;

export const WithVersions: Story = {};

export const Loading: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("/admin/v1/organizations/:orgSlug/rbac-policies/:policyId/versions", async () => {
          await new Promise((resolve) => setTimeout(resolve, 100000));
          return HttpResponse.json(mockVersions);
        }),
      ],
    },
  },
};

export const Empty: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("/admin/v1/organizations/:orgSlug/rbac-policies/:policyId/versions", () => {
          return HttpResponse.json({
            data: [],
            pagination: { offset: 0, limit: 20, total: 0, has_more: false },
          });
        }),
      ],
    },
  },
};
