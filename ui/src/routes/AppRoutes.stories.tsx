import type { Meta, StoryObj } from "@storybook/react";
import { MemoryRouter } from "react-router-dom";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { ConfigProvider } from "@/config/ConfigProvider";
import { AuthProvider } from "@/auth";
import { ApiClientProvider } from "@/api/ApiClientProvider";
import { ToastProvider } from "@/components/Toast/Toast";
import { ConfirmDialogProvider } from "@/components/ConfirmDialog/ConfirmDialog";
import { CommandPaletteProvider } from "@/components/CommandPalette/CommandPalette";
import { ConversationsProvider } from "@/components/ConversationsProvider/ConversationsProvider";
import { AppRoutes } from "./AppRoutes";

const queryClient = new QueryClient({
  defaultOptions: { queries: { retry: false } },
});

const meta: Meta<typeof AppRoutes> = {
  title: "Routes/AppRoutes",
  component: AppRoutes,
  decorators: [
    (Story) => (
      <QueryClientProvider client={queryClient}>
        <ConfigProvider>
          <AuthProvider>
            <ApiClientProvider>
              <ToastProvider>
                <ConfirmDialogProvider>
                  <CommandPaletteProvider>
                    <ConversationsProvider>
                      <MemoryRouter initialEntries={["/login"]}>
                        <Story />
                      </MemoryRouter>
                    </ConversationsProvider>
                  </CommandPaletteProvider>
                </ConfirmDialogProvider>
              </ToastProvider>
            </ApiClientProvider>
          </AuthProvider>
        </ConfigProvider>
      </QueryClientProvider>
    ),
  ],
  parameters: {
    layout: "fullscreen",
    a11y: {
      config: {
        rules: [
          // Route tree renders lazily — landmark/heading checks are irrelevant in isolation
          { id: "region", enabled: false },
          { id: "page-has-heading-one", enabled: false },
        ],
      },
    },
  },
};

export default meta;
type Story = StoryObj<typeof meta>;

export const LoginRoute: Story = {};
