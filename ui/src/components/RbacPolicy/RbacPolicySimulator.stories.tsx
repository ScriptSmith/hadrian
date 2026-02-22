import type { Meta, StoryObj } from "@storybook/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { HttpResponse, http } from "msw";
import { RbacPolicySimulator } from "./RbacPolicySimulator";
import type { OrgRbacPolicy, SimulatePolicyResponse } from "@/api/generated/types.gen";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: { retry: false, staleTime: Infinity },
  },
});

const mockPolicies: OrgRbacPolicy[] = [
  {
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
    version: 1,
    created_at: "2024-01-01T00:00:00Z",
    updated_at: "2024-01-01T00:00:00Z",
  },
  {
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
    created_at: "2024-01-01T00:00:00Z",
    updated_at: "2024-01-01T00:00:00Z",
  },
  {
    id: "policy-3",
    org_id: "org-1",
    name: "deny-external-users",
    description: "Deny access for external users",
    resource: "*",
    action: "*",
    condition: "subject.email.endsWith('@external.com')",
    effect: "deny",
    priority: 200,
    enabled: true,
    version: 1,
    created_at: "2024-01-01T00:00:00Z",
    updated_at: "2024-01-01T00:00:00Z",
  },
];

const allowedResponse: SimulatePolicyResponse = {
  rbac_enabled: true,
  allowed: true,
  matched_policy: "allow-viewers-read",
  matched_policy_source: "organization",
  reason: "Policy 'allow-viewers-read' matched with effect 'allow'",
  system_policies_evaluated: [],
  org_policies_evaluated: [
    {
      id: "policy-3",
      name: "deny-external-users",
      source: "organization",
      priority: 200,
      effect: "deny",
      pattern_matched: true,
      condition_matched: false,
    },
    {
      id: "policy-1",
      name: "require-admin-for-settings",
      source: "organization",
      priority: 100,
      effect: "deny",
      pattern_matched: false,
      condition_matched: null,
    },
    {
      id: "policy-4",
      name: "disabled-policy",
      source: "organization",
      priority: 75,
      effect: "deny",
      pattern_matched: true,
      condition_matched: null,
      skipped_reason: "Policy is disabled",
    },
    {
      id: "policy-2",
      name: "allow-viewers-read",
      source: "organization",
      priority: 50,
      effect: "allow",
      pattern_matched: true,
      condition_matched: true,
    },
  ],
};

const deniedResponse: SimulatePolicyResponse = {
  rbac_enabled: true,
  allowed: false,
  matched_policy: "deny-external-users",
  matched_policy_source: "organization",
  reason: "Policy 'deny-external-users' matched with effect 'deny'",
  system_policies_evaluated: [],
  org_policies_evaluated: [
    {
      id: "policy-3",
      name: "deny-external-users",
      source: "organization",
      priority: 200,
      effect: "deny",
      pattern_matched: true,
      condition_matched: true,
    },
    {
      id: "policy-1",
      name: "require-admin-for-settings",
      source: "organization",
      priority: 100,
      effect: "deny",
      pattern_matched: false,
      condition_matched: null,
    },
    {
      id: "policy-2",
      name: "allow-viewers-read",
      source: "organization",
      priority: 50,
      effect: "allow",
      pattern_matched: true,
      condition_matched: false,
    },
  ],
};

const meta: Meta<typeof RbacPolicySimulator> = {
  title: "Admin/RbacPolicySimulator",
  component: RbacPolicySimulator,
  decorators: [
    (Story) => (
      <QueryClientProvider client={queryClient}>
        <Story />
      </QueryClientProvider>
    ),
  ],
  args: {
    orgSlug: "acme",
    policies: mockPolicies,
  },
  parameters: {
    msw: {
      handlers: [
        http.post(
          "/admin/v1/organizations/:orgSlug/rbac-policies/simulate",
          async ({ request }) => {
            const body = (await request.json()) as { subject?: { email?: string } };
            // Return denied for external users, allowed otherwise
            if (body.subject?.email?.endsWith("@external.com")) {
              return HttpResponse.json(deniedResponse);
            }
            return HttpResponse.json(allowedResponse);
          }
        ),
      ],
    },
  },
};

export default meta;
type Story = StoryObj<typeof RbacPolicySimulator>;

export const Default: Story = {};

export const NoPolicies: Story = {
  args: {
    policies: [],
  },
};
