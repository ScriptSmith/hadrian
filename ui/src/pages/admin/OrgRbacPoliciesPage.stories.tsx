import type { Meta, StoryObj } from "@storybook/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter, Routes, Route } from "react-router-dom";
import { http, HttpResponse } from "msw";
import OrgRbacPoliciesPage from "./OrgRbacPoliciesPage";
import type {
  Organization,
  OrgRbacPolicy,
  OrgRbacPolicyListResponse,
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

const mockOrg: Organization = {
  id: "org-123",
  slug: "acme-corp",
  name: "Acme Corporation",
  created_at: "2024-01-01T00:00:00Z",
};

const mockPolicies: OrgRbacPolicy[] = [
  {
    id: "policy-1",
    org_id: "org-123",
    name: "require-admin-for-settings",
    description: "Restricts settings access to administrators only",
    resource: "sso_config",
    action: "*",
    condition: "'org_admin' in subject.roles || 'super_admin' in subject.roles",
    effect: "allow",
    priority: 100,
    enabled: true,
    version: 3,
    created_at: "2024-01-01T00:00:00Z",
    updated_at: "2024-06-15T10:30:00Z",
  },
  {
    id: "policy-2",
    org_id: "org-123",
    name: "allow-team-leads-manage-members",
    description: "Allow team leads to manage their team members",
    resource: "team",
    action: "manage",
    condition: "'team_lead' in subject.roles && context.team_id in subject.team_ids",
    effect: "allow",
    priority: 50,
    enabled: true,
    version: 1,
    created_at: "2024-03-01T00:00:00Z",
    updated_at: "2024-03-01T00:00:00Z",
  },
  {
    id: "policy-3",
    org_id: "org-123",
    name: "deny-external-api-keys",
    description: "Deny API key creation for external contractors",
    resource: "api_key",
    action: "create",
    condition: "subject.email.endsWith('@contractor.acme.com')",
    effect: "deny",
    priority: 200,
    enabled: false,
    version: 2,
    created_at: "2024-04-15T00:00:00Z",
    updated_at: "2024-05-10T08:00:00Z",
  },
  {
    id: "policy-4",
    org_id: "org-123",
    name: "restrict-model-pricing",
    description: "Only finance team can modify model pricing",
    resource: "model_pricing",
    action: "*",
    condition: "'finance' in subject.roles",
    effect: "allow",
    priority: 75,
    enabled: true,
    version: 1,
    created_at: "2024-06-01T00:00:00Z",
    updated_at: "2024-06-01T00:00:00Z",
  },
];

const mockPoliciesResponse: OrgRbacPolicyListResponse = {
  data: mockPolicies,
  pagination: {
    offset: 0,
    limit: 20,
    total: mockPolicies.length,
    has_more: false,
  },
};

const emptyPoliciesResponse: OrgRbacPolicyListResponse = {
  data: [],
  pagination: {
    offset: 0,
    limit: 20,
    total: 0,
    has_more: false,
  },
};

const meta: Meta<typeof OrgRbacPoliciesPage> = {
  title: "Admin/OrgRbacPoliciesPage",
  component: OrgRbacPoliciesPage,
  parameters: {
    layout: "fullscreen",
  },
  decorators: [
    (Story) => (
      <QueryClientProvider client={queryClient}>
        <ToastProvider>
          <ConfirmDialogProvider>
            <MemoryRouter initialEntries={["/admin/organizations/acme-corp/rbac-policies"]}>
              <Routes>
                <Route path="/admin/organizations/:orgSlug/rbac-policies" element={<Story />} />
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

export const WithPolicies: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/organizations/acme-corp", () => {
          return HttpResponse.json(mockOrg);
        }),
        http.get("*/admin/v1/organizations/acme-corp/rbac-policies", () => {
          return HttpResponse.json(mockPoliciesResponse);
        }),
        http.post("*/admin/v1/organizations/acme-corp/rbac-policies/simulate", () => {
          return HttpResponse.json({
            rbac_enabled: true,
            allowed: true,
            matched_policy: "allow-team-leads-manage-members",
            matched_policy_source: "organization",
            reason: "Policy matched with effect 'allow'",
            system_policies_evaluated: [],
            org_policies_evaluated: [
              {
                id: "policy-3",
                name: "deny-external-api-keys",
                source: "organization",
                priority: 200,
                effect: "deny",
                pattern_matched: false,
                condition_matched: null,
              },
              {
                id: "policy-1",
                name: "require-admin-for-settings",
                source: "organization",
                priority: 100,
                effect: "allow",
                pattern_matched: false,
                condition_matched: null,
              },
              {
                id: "policy-5",
                name: "disabled-test-policy",
                source: "organization",
                priority: 80,
                effect: "deny",
                pattern_matched: true,
                condition_matched: null,
                skipped_reason: "Policy is disabled",
              },
              {
                id: "policy-4",
                name: "restrict-model-pricing",
                source: "organization",
                priority: 75,
                effect: "allow",
                pattern_matched: false,
                condition_matched: null,
              },
              {
                id: "policy-2",
                name: "allow-team-leads-manage-members",
                source: "organization",
                priority: 50,
                effect: "allow",
                pattern_matched: true,
                condition_matched: true,
              },
            ],
          });
        }),
        http.post("*/admin/v1/rbac-policies/validate", () => {
          return HttpResponse.json({ valid: true, error: null });
        }),
      ],
    },
  },
};

export const NoPolicies: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/organizations/acme-corp", () => {
          return HttpResponse.json(mockOrg);
        }),
        http.get("*/admin/v1/organizations/acme-corp/rbac-policies", () => {
          return HttpResponse.json(emptyPoliciesResponse);
        }),
        http.post("*/admin/v1/rbac-policies/validate", () => {
          return HttpResponse.json({ valid: true, error: null });
        }),
      ],
    },
  },
};

export const Loading: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/organizations/acme-corp", async () => {
          await new Promise((resolve) => setTimeout(resolve, 100000));
          return HttpResponse.json(mockOrg);
        }),
        http.get("*/admin/v1/organizations/acme-corp/rbac-policies", async () => {
          await new Promise((resolve) => setTimeout(resolve, 100000));
          return HttpResponse.json(mockPoliciesResponse);
        }),
      ],
    },
  },
};

export const ManyPolicies: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/organizations/acme-corp", () => {
          return HttpResponse.json(mockOrg);
        }),
        http.get("*/admin/v1/organizations/acme-corp/rbac-policies", () => {
          // Generate many policies for pagination testing
          const manyPolicies: OrgRbacPolicy[] = Array.from({ length: 25 }, (_, i) => ({
            id: `policy-${i + 1}`,
            org_id: "org-123",
            name: `policy-${i + 1}`,
            description: `Auto-generated policy #${i + 1}`,
            resource: ["*", "team", "project", "user", "api_key"][i % 5],
            action: ["*", "create", "read", "update", "delete"][i % 5],
            condition: `'role_${i}' in subject.roles`,
            effect: i % 3 === 0 ? "deny" : "allow",
            priority: 100 - i,
            enabled: i % 4 !== 0,
            version: Math.floor(Math.random() * 5) + 1,
            created_at: "2024-01-01T00:00:00Z",
            updated_at: "2024-06-15T10:30:00Z",
          }));
          return HttpResponse.json({
            data: manyPolicies,
            pagination: {
              offset: 0,
              limit: 20,
              total: manyPolicies.length,
              has_more: true,
            },
          });
        }),
        http.post("*/admin/v1/organizations/acme-corp/rbac-policies/simulate", () => {
          return HttpResponse.json({
            allowed: false,
            matched_policy: "policy-1",
            reason: "Policy matched with effect 'deny'",
            policies_evaluated: [],
          });
        }),
        http.post("*/admin/v1/rbac-policies/validate", () => {
          return HttpResponse.json({ valid: true, error: null });
        }),
      ],
    },
  },
};
