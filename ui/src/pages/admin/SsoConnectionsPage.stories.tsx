import type { Meta, StoryObj } from "@storybook/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter } from "react-router-dom";
import { http, HttpResponse, delay } from "msw";
import SsoConnectionsPage from "./SsoConnectionsPage";
import type { SsoConnection, SsoConnectionsResponse } from "@/api/generated/types.gen";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: false,
      staleTime: Infinity,
    },
  },
});

const meta: Meta<typeof SsoConnectionsPage> = {
  title: "Admin/SsoConnectionsPage",
  component: SsoConnectionsPage,
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
        <MemoryRouter initialEntries={["/admin/sso"]}>
          <div className="min-h-screen bg-background">
            <Story />
          </div>
        </MemoryRouter>
      </QueryClientProvider>
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

const mockResponse: SsoConnectionsResponse = {
  data: [oidcConnection],
};

const emptyResponse: SsoConnectionsResponse = {
  data: [],
};

export const WithOidcConnection: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/sso-connections", () => {
          return HttpResponse.json(mockResponse);
        }),
      ],
    },
  },
};

export const Empty: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/sso-connections", () => {
          return HttpResponse.json(emptyResponse);
        }),
      ],
    },
  },
};

export const Loading: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/sso-connections", async () => {
          await delay("infinite");
          return HttpResponse.json(mockResponse);
        }),
      ],
    },
  },
};

export const Error: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/sso-connections", () => {
          return HttpResponse.json({ error: "Internal server error" }, { status: 500 });
        }),
      ],
    },
  },
};
