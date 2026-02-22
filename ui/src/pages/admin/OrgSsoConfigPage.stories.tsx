import type { Meta, StoryObj } from "@storybook/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter, Routes, Route } from "react-router-dom";
import { http, HttpResponse, delay } from "msw";
import OrgSsoConfigPage from "./OrgSsoConfigPage";
import type {
  OrgSsoConfig,
  Organization,
  DomainVerification,
  ListDomainVerificationsResponse,
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

const mockConfig: OrgSsoConfig = {
  id: "config-123",
  org_id: "org-123",
  provider_type: "oidc",
  issuer: "https://accounts.google.com",
  discovery_url: null,
  client_id: "google-client-id-12345",
  redirect_uri: "https://gateway.acme.com/auth/callback",
  scopes: ["openid", "email", "profile", "groups"],
  identity_claim: "sub",
  org_claim: null,
  groups_claim: "groups",
  provisioning_enabled: true,
  create_users: true,
  default_team_id: "team-engineering",
  default_org_role: "member",
  default_team_role: "member",
  allowed_email_domains: ["acme.com", "acme.org"],
  sync_attributes_on_login: false,
  sync_memberships_on_login: true,
  enforcement_mode: "optional",
  enabled: true,
  created_at: "2024-01-15T10:00:00Z",
  updated_at: "2024-06-20T14:30:00Z",
};

const mockDomains: DomainVerification[] = [
  {
    id: "domain-1",
    org_sso_config_id: "config-123",
    domain: "acme.com",
    verification_token: "abc123xyz",
    status: "verified",
    dns_txt_record: "hadrian-verify=abc123xyz",
    verification_attempts: 2,
    last_attempt_at: "2024-01-15T10:30:00Z",
    verified_at: "2024-01-15T10:30:00Z",
    created_at: "2024-01-10T09:00:00Z",
    updated_at: "2024-01-15T10:30:00Z",
  },
  {
    id: "domain-2",
    org_sso_config_id: "config-123",
    domain: "acme.org",
    verification_token: "def456uvw",
    status: "pending",
    verification_attempts: 0,
    created_at: "2024-01-12T14:00:00Z",
    updated_at: "2024-01-12T14:00:00Z",
  },
];

const mockDomainsResponse: ListDomainVerificationsResponse = {
  items: mockDomains,
  total: mockDomains.length,
};

const emptyDomainsResponse: ListDomainVerificationsResponse = {
  items: [],
  total: 0,
};

const meta: Meta<typeof OrgSsoConfigPage> = {
  title: "Admin/OrgSsoConfigPage",
  component: OrgSsoConfigPage,
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
            <MemoryRouter initialEntries={["/admin/organizations/acme-corp/sso-config"]}>
              <Routes>
                <Route path="/admin/organizations/:orgSlug/sso-config" element={<Story />} />
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

export const WithConfig: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/organizations/acme-corp", () => {
          return HttpResponse.json(mockOrg);
        }),
        http.get("*/admin/v1/organizations/acme-corp/sso-config", () => {
          return HttpResponse.json(mockConfig);
        }),
        http.get("*/admin/v1/organizations/acme-corp/sso-config/domains", () => {
          return HttpResponse.json(mockDomainsResponse);
        }),
      ],
    },
  },
};

export const NoConfig: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/organizations/acme-corp", () => {
          return HttpResponse.json(mockOrg);
        }),
        http.get("*/admin/v1/organizations/acme-corp/sso-config", () => {
          return HttpResponse.json(
            { error: { message: "SSO config not found", code: "not_found" } },
            { status: 404 }
          );
        }),
        http.get("*/admin/v1/organizations/acme-corp/sso-config/domains", () => {
          return HttpResponse.json(emptyDomainsResponse);
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
          await delay("infinite");
          return HttpResponse.json(mockOrg);
        }),
        http.get("*/admin/v1/organizations/acme-corp/sso-config", async () => {
          await delay("infinite");
          return HttpResponse.json(mockConfig);
        }),
        http.get("*/admin/v1/organizations/acme-corp/sso-config/domains", async () => {
          await delay("infinite");
          return HttpResponse.json(mockDomainsResponse);
        }),
      ],
    },
  },
};

export const DisabledConfig: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/organizations/acme-corp", () => {
          return HttpResponse.json(mockOrg);
        }),
        http.get("*/admin/v1/organizations/acme-corp/sso-config", () => {
          return HttpResponse.json({
            ...mockConfig,
            enabled: false,
            enforcement_mode: "test",
          });
        }),
        http.get("*/admin/v1/organizations/acme-corp/sso-config/domains", () => {
          return HttpResponse.json(mockDomainsResponse);
        }),
      ],
    },
  },
};

export const RequiredEnforcement: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/organizations/acme-corp", () => {
          return HttpResponse.json(mockOrg);
        }),
        http.get("*/admin/v1/organizations/acme-corp/sso-config", () => {
          return HttpResponse.json({
            ...mockConfig,
            enforcement_mode: "required",
          });
        }),
        http.get("*/admin/v1/organizations/acme-corp/sso-config/domains", () => {
          return HttpResponse.json(mockDomainsResponse);
        }),
      ],
    },
  },
};
