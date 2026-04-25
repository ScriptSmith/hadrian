import type { Meta, StoryObj } from "@storybook/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { http, HttpResponse } from "msw";
import { MemoryRouter, Route, Routes } from "react-router-dom";

import { AuthContext } from "@/auth";
import type { AuthContextValue } from "@/auth";

import OAuthAuthorizePage from "./OAuthAuthorizePage";

const queryClient = new QueryClient({
  defaultOptions: { queries: { retry: false, staleTime: Infinity } },
});

const authedContext: AuthContextValue = {
  isAuthenticated: true,
  isLoading: false,
  user: {
    id: "11111111-2222-3333-4444-555555555555",
    email: "alice@acme-corp.com",
    name: "Alice Johnson",
    roles: ["member"],
  },
  method: "oidc",
  token: "mock-token",
  login: async () => {},
  logout: () => {},
  setApiKey: () => {},
};

const preflightOkHandler = http.get("*/admin/v1/oauth/preflight", () =>
  HttpResponse.json({ callback_host: "app.example.com" })
);

const eligibleOwnersHandler = http.get("*/admin/v1/me/eligible-owners", () =>
  HttpResponse.json({
    user: {
      id: "11111111-2222-3333-4444-555555555555",
      slug: "alice@acme-corp.com",
      name: "Alice Johnson",
    },
    organizations: [
      {
        id: "00000000-0000-0000-0000-0000000000a1",
        slug: "acme",
        name: "Acme Corp",
        role: "admin",
      },
    ],
    teams: [
      {
        id: "00000000-0000-0000-0000-0000000000b1",
        slug: "platform",
        name: "Platform",
        org_id: "00000000-0000-0000-0000-0000000000a1",
        org_slug: "acme",
        role: "lead",
      },
    ],
    projects: [
      {
        id: "00000000-0000-0000-0000-0000000000c1",
        slug: "billing",
        name: "Billing",
        org_id: "00000000-0000-0000-0000-0000000000a1",
        org_slug: "acme",
        role: "owner",
      },
    ],
  })
);

const noOwnersHandler = http.get("*/admin/v1/me/eligible-owners", () =>
  HttpResponse.json({
    user: {
      id: "11111111-2222-3333-4444-555555555555",
      slug: "alice@acme-corp.com",
      name: "Alice Johnson",
    },
    organizations: [],
    teams: [],
    projects: [],
  })
);

const meta: Meta<typeof OAuthAuthorizePage> = {
  title: "Pages/OAuthAuthorizePage",
  component: OAuthAuthorizePage,
  parameters: {
    layout: "fullscreen",
  },
};
export default meta;
type Story = StoryObj<typeof meta>;

function renderAt(initialUrl: string) {
  // Fresh QueryClient per render so stories don't share cached responses.
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false, staleTime: Infinity } },
  });
  return (
    <QueryClientProvider client={client}>
      <AuthContext.Provider value={authedContext}>
        <MemoryRouter initialEntries={[initialUrl]}>
          <Routes>
            <Route path="/oauth/authorize" element={<OAuthAuthorizePage />} />
          </Routes>
        </MemoryRouter>
      </AuthContext.Provider>
    </QueryClientProvider>
  );
}

const baseUrl =
  "/oauth/authorize?callback_url=https%3A%2F%2Fapp.example.com%2Fcb&code_challenge=E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM&code_challenge_method=S256&app_name=Acme%20Notes&scopes=chat,embeddings";

export const Default: Story = {
  parameters: {
    msw: {
      handlers: [
        preflightOkHandler,
        eligibleOwnersHandler,
        http.post("*/admin/v1/oauth/authorize", () =>
          HttpResponse.json({
            code: "abc-123",
            redirect_url: "https://app.example.com/cb?code=abc-123",
            expires_at: new Date(Date.now() + 600_000).toISOString(),
          })
        ),
      ],
    },
  },
  render: () => renderAt(baseUrl),
};

export const PersonalOnly: Story = {
  parameters: {
    msw: { handlers: [preflightOkHandler, noOwnersHandler] },
  },
  render: () => renderAt(baseUrl),
};

export const WithoutScopes: Story = {
  parameters: {
    msw: { handlers: [preflightOkHandler, eligibleOwnersHandler] },
  },
  render: () =>
    renderAt(
      "/oauth/authorize?callback_url=https%3A%2F%2Fapp.example.com%2Fcb&code_challenge=E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM&code_challenge_method=S256"
    ),
};

export const InvalidParams: Story = {
  render: () => (
    <QueryClientProvider client={queryClient}>
      <AuthContext.Provider value={authedContext}>
        <MemoryRouter initialEntries={["/oauth/authorize"]}>
          <Routes>
            <Route path="/oauth/authorize" element={<OAuthAuthorizePage />} />
          </Routes>
        </MemoryRouter>
      </AuthContext.Provider>
    </QueryClientProvider>
  ),
};

export const InvalidCallbackScheme: Story = {
  render: () => (
    <QueryClientProvider client={queryClient}>
      <AuthContext.Provider value={authedContext}>
        <MemoryRouter
          initialEntries={[
            "/oauth/authorize?callback_url=ftp%3A%2F%2Fapp.example.com%2Fcb&code_challenge=E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM&code_challenge_method=S256",
          ]}
        >
          <Routes>
            <Route path="/oauth/authorize" element={<OAuthAuthorizePage />} />
          </Routes>
        </MemoryRouter>
      </AuthContext.Provider>
    </QueryClientProvider>
  ),
};
