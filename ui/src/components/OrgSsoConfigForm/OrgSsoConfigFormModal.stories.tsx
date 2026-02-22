import type { Meta, StoryObj } from "@storybook/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fn } from "storybook/test";
import { OrgSsoConfigFormModal } from "./OrgSsoConfigFormModal";
import type { OrgSsoConfig, Team } from "@/api/generated/types.gen";
import { ToastProvider } from "@/components/Toast/Toast";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: false,
    },
  },
});

const mockTeams: Team[] = [
  {
    id: "team-1",
    org_id: "org-1",
    name: "Engineering",
    slug: "engineering",
    created_at: "2024-01-01T00:00:00Z",
    updated_at: "2024-01-01T00:00:00Z",
  },
  {
    id: "team-2",
    org_id: "org-1",
    name: "Platform",
    slug: "platform",
    created_at: "2024-01-01T00:00:00Z",
    updated_at: "2024-01-01T00:00:00Z",
  },
];

const mockExistingConfig: OrgSsoConfig = {
  id: "config-1",
  org_id: "org-1",
  provider_type: "oidc",
  issuer: "https://accounts.google.com",
  discovery_url: null,
  client_id: "my-client-id-123",
  redirect_uri: "https://gateway.example.com/auth/callback",
  scopes: ["openid", "email", "profile", "groups"],
  identity_claim: "sub",
  org_claim: "org",
  groups_claim: "groups",
  provisioning_enabled: true,
  create_users: true,
  default_team_id: "team-1",
  default_org_role: "member",
  default_team_role: "member",
  allowed_email_domains: ["acme.com", "acme.org"],
  sync_attributes_on_login: false,
  sync_memberships_on_login: true,
  enforcement_mode: "optional",
  enabled: true,
  created_at: "2024-01-01T00:00:00Z",
  updated_at: "2024-06-15T10:30:00Z",
  // SAML fields (not used for OIDC)
  saml_sign_requests: false,
  saml_force_authn: false,
};

// SAML config using metadata URL
const mockSamlConfig: OrgSsoConfig = {
  id: "config-saml-1",
  org_id: "org-1",
  provider_type: "saml",
  // SAML IdP fields
  saml_metadata_url: "https://idp.okta.com/app/abc123/sso/saml/metadata",
  saml_idp_entity_id: "https://idp.okta.com/abc123",
  saml_idp_sso_url: "https://idp.okta.com/app/abc123/sso/saml",
  saml_idp_slo_url: "https://idp.okta.com/app/abc123/slo/saml",
  saml_idp_certificate:
    "-----BEGIN CERTIFICATE-----\nMIIDpDCCAoygAwIBAgIGAYwZ...(truncated)...\n-----END CERTIFICATE-----",
  // SAML SP fields
  saml_sp_entity_id: "https://gateway.acme.com/saml",
  saml_name_id_format: "urn:oasis:names:tc:SAML:1.1:nameid-format:emailAddress",
  saml_sign_requests: true,
  saml_force_authn: false,
  saml_authn_context_class_ref: null,
  // SAML attribute mappings
  saml_identity_attribute: "urn:oid:0.9.2342.19200300.100.1.1",
  saml_email_attribute: "urn:oid:0.9.2342.19200300.100.1.3",
  saml_name_attribute: "urn:oid:2.16.840.1.113730.3.1.241",
  saml_groups_attribute: "memberOf",
  // OIDC fields (not used for SAML)
  issuer: null,
  discovery_url: null,
  client_id: null,
  redirect_uri: null,
  scopes: [],
  identity_claim: null,
  org_claim: null,
  groups_claim: null,
  // Common fields
  provisioning_enabled: true,
  create_users: true,
  default_team_id: "team-1",
  default_org_role: "member",
  default_team_role: "member",
  allowed_email_domains: ["acme.com"],
  sync_attributes_on_login: true,
  sync_memberships_on_login: true,
  enforcement_mode: "required",
  enabled: true,
  created_at: "2024-01-01T00:00:00Z",
  updated_at: "2024-06-15T10:30:00Z",
};

// SAML config with manual IdP configuration (no metadata URL)
const mockSamlManualConfig: OrgSsoConfig = {
  ...mockSamlConfig,
  id: "config-saml-2",
  saml_metadata_url: null,
  // Manual IdP configuration
  saml_idp_entity_id: "https://adfs.corp.acme.com/adfs/services/trust",
  saml_idp_sso_url: "https://adfs.corp.acme.com/adfs/ls/",
  saml_idp_slo_url: null,
  saml_idp_certificate:
    "-----BEGIN CERTIFICATE-----\nMIIC8DCCAdigAwIBAgIQH...(truncated)...\n-----END CERTIFICATE-----",
};

const meta: Meta<typeof OrgSsoConfigFormModal> = {
  title: "Admin/OrgSsoConfigFormModal",
  component: OrgSsoConfigFormModal,

  decorators: [
    (Story) => (
      <QueryClientProvider client={queryClient}>
        <ToastProvider>
          <div style={{ minHeight: "600px" }}>
            <Story />
          </div>
        </ToastProvider>
      </QueryClientProvider>
    ),
  ],
  args: {
    open: true,
    onClose: fn(),
    onCreateSubmit: fn(),
    onUpdateSubmit: fn(),
    isLoading: false,
    editingConfig: null,
    teams: mockTeams,
    orgSlug: "acme",
  },
};

export default meta;
type Story = StoryObj<typeof OrgSsoConfigFormModal>;

export const CreateMode: Story = {
  args: {
    editingConfig: null,
  },
};

export const EditMode: Story = {
  args: {
    editingConfig: mockExistingConfig,
  },
};

export const Loading: Story = {
  args: {
    isLoading: true,
  },
};

export const NoTeams: Story = {
  args: {
    teams: [],
    editingConfig: null,
  },
};

// SAML Stories

export const SAMLEditMode: Story = {
  args: {
    editingConfig: mockSamlConfig,
  },
};

export const SAMLManualConfig: Story = {
  args: {
    editingConfig: mockSamlManualConfig,
  },
};
