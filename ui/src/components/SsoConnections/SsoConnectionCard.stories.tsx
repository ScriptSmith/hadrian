import type { Meta, StoryObj } from "@storybook/react";
import { BrowserRouter } from "react-router-dom";
import { SsoConnectionCard } from "./SsoConnectionCard";
import type { SsoConnection } from "@/api/generated/types.gen";

const meta: Meta<typeof SsoConnectionCard> = {
  title: "Admin/SsoConnectionCard",
  component: SsoConnectionCard,
  parameters: {
    layout: "padded",
  },
  decorators: [
    (Story) => (
      <BrowserRouter>
        <div className="max-w-2xl">
          <Story />
        </div>
      </BrowserRouter>
    ),
  ],
};

export default meta;
type Story = StoryObj<typeof meta>;

const oidcConnection: SsoConnection = {
  name: "default",
  type: "oidc",
  issuer: "https://auth.example.com/realms/production",
  client_id: "hadrian-gateway",
  scopes: ["openid", "profile", "email", "groups"],
  identity_claim: "preferred_username",
  groups_claim: "groups",
  jit_enabled: true,
  organization_id: "acme-corp",
  default_team_id: "engineering",
  default_org_role: "member",
  default_team_role: "member",
  sync_memberships_on_login: true,
};

const oidcConnectionNoJit: SsoConnection = {
  name: "default",
  type: "oidc",
  issuer: "https://login.microsoftonline.com/tenant-id/v2.0",
  client_id: "12345678-1234-1234-1234-123456789012",
  scopes: ["openid", "profile", "email"],
  identity_claim: "email",
  groups_claim: null,
  jit_enabled: false,
  organization_id: null,
  default_team_id: null,
  default_org_role: null,
  default_team_role: null,
  sync_memberships_on_login: false,
};

const proxyAuthConnection: SsoConnection = {
  name: "default",
  type: "proxy_auth",
  issuer: null,
  client_id: null,
  scopes: null,
  identity_claim: null,
  groups_claim: null,
  jit_enabled: false,
  organization_id: null,
  default_team_id: null,
  default_org_role: null,
  default_team_role: null,
  sync_memberships_on_login: false,
};

export const OidcWithJit: Story = {
  args: {
    connection: oidcConnection,
  },
};

export const OidcWithoutJit: Story = {
  args: {
    connection: oidcConnectionNoJit,
  },
};

export const ProxyAuth: Story = {
  args: {
    connection: proxyAuthConnection,
  },
};

export const OidcMinimal: Story = {
  args: {
    connection: {
      name: "default",
      type: "oidc",
      issuer: "https://accounts.google.com",
      client_id: "google-client-id.apps.googleusercontent.com",
      scopes: ["openid", "email"],
      identity_claim: "email",
      groups_claim: null,
      jit_enabled: true,
      organization_id: "my-org",
      default_team_id: null,
      default_org_role: "viewer",
      default_team_role: null,
      sync_memberships_on_login: false,
    },
  },
};
