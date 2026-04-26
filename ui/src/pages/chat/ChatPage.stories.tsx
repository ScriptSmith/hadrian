import type { Meta, StoryObj } from "@storybook/react";
import { http, HttpResponse } from "msw";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter, Route, Routes } from "react-router-dom";

import ChatPage from "./ChatPage";
import { AuthProvider } from "@/auth";
import { ConfigProvider } from "@/config/ConfigProvider";
import { PreferencesProvider } from "@/preferences/PreferencesProvider";
import { ConfirmDialogProvider } from "@/components/ConfirmDialog/ConfirmDialog";
import { ToastProvider } from "@/components/Toast/Toast";
import { TooltipProvider } from "@/components/Tooltip/Tooltip";
import { ConversationsProvider } from "@/components/ConversationsProvider/ConversationsProvider";
import type { UiConfig } from "@/config/types";

const mockConfig: UiConfig = {
  branding: {
    title: "Hadrian Gateway",
    tagline: null,
    logo_url: null,
    logo_dark_url: null,
    favicon_url: null,
    colors: {},
    colors_dark: null,
    fonts: null,
    footer_text: null,
    footer_links: [],
    show_version: false,
    version: null,
    login: null,
  },
  chat: {
    enabled: true,
    default_model: "openai/gpt-5",
    available_models: [],
    file_uploads_enabled: true,
    max_file_size_bytes: 10 * 1024 * 1024,
    allowed_file_types: [],
  },
  admin: { enabled: true },
  auth: { methods: ["api_key"], oidc: null },
};

const mockModels = [
  { id: "openai/gpt-5", object: "model", created: 0, owned_by: "openai" },
  { id: "anthropic/claude-opus-4-7", object: "model", created: 0, owned_by: "anthropic" },
];

const handlers = [
  http.get("*/admin/v1/ui/config", () => HttpResponse.json(mockConfig)),
  http.get("*/auth/me", () =>
    HttpResponse.json({
      external_id: "story-user",
      email: "story@example.com",
      name: "Story User",
      user_id: "00000000-0000-0000-0000-000000000001",
      roles: [],
      idp_groups: [],
    })
  ),
  http.get("*/api/v1/models", () => HttpResponse.json({ object: "list", data: mockModels })),
  http.get("*/admin/v1/users/*/conversations*", () =>
    HttpResponse.json({ data: [], pagination: { limit: 100, has_more: false } })
  ),
  http.get("*/admin/v1/users/*/api-keys*", () =>
    HttpResponse.json({ data: [], pagination: { limit: 100, has_more: false } })
  ),
  http.get("*/admin/v1/users/*/skills*", () =>
    HttpResponse.json({ data: [], pagination: { limit: 100, has_more: false } })
  ),
];

const meta: Meta<typeof ChatPage> = {
  title: "Pages/ChatPage",
  component: ChatPage,
  decorators: [
    (Story) => {
      const queryClient = new QueryClient({
        defaultOptions: { queries: { retry: false } },
      });
      return (
        <QueryClientProvider client={queryClient}>
          <ConfigProvider>
            <AuthProvider>
              <PreferencesProvider>
                <ToastProvider>
                  <ConfirmDialogProvider>
                    <TooltipProvider>
                      <ConversationsProvider>
                        <MemoryRouter initialEntries={["/chat"]}>
                          <Routes>
                            <Route path="/chat" element={<Story />} />
                            <Route path="/chat/:conversationId" element={<Story />} />
                          </Routes>
                        </MemoryRouter>
                      </ConversationsProvider>
                    </TooltipProvider>
                  </ConfirmDialogProvider>
                </ToastProvider>
              </PreferencesProvider>
            </AuthProvider>
          </ConfigProvider>
        </QueryClientProvider>
      );
    },
  ],
  parameters: {
    layout: "fullscreen",
    msw: { handlers },
    a11y: {
      config: {
        rules: [
          // ChatPage is rendered without the AppLayout shell in this story, so
          // landmark/heading checks are spurious.
          { id: "region", enabled: false },
          { id: "page-has-heading-one", enabled: false },
        ],
      },
    },
  },
};

export default meta;
type Story = StoryObj<typeof ChatPage>;

/**
 * Empty chat page — no current conversation, default models. Smokes the route +
 * provider stack so refactors that break the routing/state wiring fail visibly
 * in Storybook before they hit prod.
 */
export const Empty: Story = {};
