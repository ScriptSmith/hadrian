import type { Meta, StoryObj } from "@storybook/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fn } from "storybook/test";
import { HttpResponse, http } from "msw";
import { RbacPolicyFormModal } from "./RbacPolicyFormModal";
import type { OrgRbacPolicy } from "@/api/generated/types.gen";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: { retry: false, staleTime: Infinity },
  },
});

const mockExistingPolicy: OrgRbacPolicy = {
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

const mockAllowPolicy: OrgRbacPolicy = {
  id: "policy-2",
  org_id: "org-1",
  name: "allow-viewers-read",
  description: "Allow viewers to read all resources",
  resource: "*",
  action: "read",
  condition: "'viewer' in subject.roles",
  effect: "allow",
  priority: 50,
  enabled: true,
  version: 1,
  created_at: "2024-03-01T00:00:00Z",
  updated_at: "2024-03-01T00:00:00Z",
};

const meta: Meta<typeof RbacPolicyFormModal> = {
  title: "Admin/RbacPolicyFormModal",
  component: RbacPolicyFormModal,
  decorators: [
    (Story) => (
      <QueryClientProvider client={queryClient}>
        <div style={{ minHeight: "600px" }}>
          <Story />
        </div>
      </QueryClientProvider>
    ),
  ],
  args: {
    open: true,
    onClose: fn(),
    onCreateSubmit: fn(),
    onUpdateSubmit: fn(),
    isLoading: false,
    editingPolicy: null,
  },
  parameters: {
    msw: {
      handlers: [
        http.post("/admin/v1/rbac-policies/validate", () => {
          return HttpResponse.json({ valid: true, error: null });
        }),
      ],
    },
  },
};

export default meta;
type Story = StoryObj<typeof RbacPolicyFormModal>;

export const CreateMode: Story = {
  args: {
    editingPolicy: null,
  },
};

export const EditMode: Story = {
  args: {
    editingPolicy: mockExistingPolicy,
  },
};

export const EditAllowPolicy: Story = {
  args: {
    editingPolicy: mockAllowPolicy,
  },
};

export const Loading: Story = {
  args: {
    isLoading: true,
  },
};
