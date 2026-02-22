import type { Meta, StoryObj } from "@storybook/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter } from "react-router-dom";
import { Header } from "./Header";
import { AuthProvider } from "@/auth";
import { ConfigProvider } from "@/config/ConfigProvider";
import { PreferencesProvider } from "@/preferences/PreferencesProvider";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: false,
    },
  },
});

const meta: Meta<typeof Header> = {
  title: "Layout/Header",
  component: Header,
  parameters: {
    layout: "fullscreen",
  },
  decorators: [
    (Story, context) => (
      <QueryClientProvider client={queryClient}>
        <MemoryRouter initialEntries={[context.args.initialRoute || "/chat"]}>
          <ConfigProvider>
            <AuthProvider>
              <PreferencesProvider>
                <Story />
              </PreferencesProvider>
            </AuthProvider>
          </ConfigProvider>
        </MemoryRouter>
      </QueryClientProvider>
    ),
  ],
};

export default meta;
type Story = StoryObj<typeof meta> & { args?: { initialRoute?: string } };

export const Default: Story = {
  args: {},
};

export const WithMenuButton: Story = {
  args: {
    showMenuButton: true,
    onMenuClick: () => console.log("Menu clicked"),
  },
};

export const OnChatPage: Story = {
  args: {
    initialRoute: "/chat",
  },
};

export const OnProjectsPage: Story = {
  args: {
    initialRoute: "/projects",
  },
};

export const OnKnowledgeBasesPage: Story = {
  args: {
    initialRoute: "/knowledge-bases",
  },
};

export const OnApiKeysPage: Story = {
  args: {
    initialRoute: "/api-keys",
  },
};

export const OnAdminPage: Story = {
  args: {
    initialRoute: "/admin",
  },
};

export const OnAdminSubpage: Story = {
  args: {
    initialRoute: "/admin/users",
  },
};
