import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { Meta, StoryObj } from "@storybook/react";
import { MemoryRouter, Routes, Route } from "react-router-dom";
import { AppLayout } from "./AppLayout";
import { AuthProvider } from "@/auth";
import { ConfigProvider } from "@/config/ConfigProvider";
import { PreferencesProvider } from "@/preferences/PreferencesProvider";
import { ConfirmDialogProvider } from "../ConfirmDialog/ConfirmDialog";
import { ConversationsProvider } from "../ConversationsProvider/ConversationsProvider";
import { CommandPaletteProvider } from "../CommandPalette/CommandPalette";
import { ToastProvider } from "../Toast/Toast";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: false,
    },
  },
});

const meta: Meta<typeof AppLayout> = {
  title: "Layout/AppLayout",
  component: AppLayout,
  parameters: {
    layout: "fullscreen",
  },
};

export default meta;
type Story = StoryObj<typeof meta>;

function MockContent() {
  return (
    <div className="p-8">
      <h1 className="text-2xl font-bold mb-4">Page Content</h1>
      <p className="text-muted-foreground">
        This is the main content area of the application. The layout includes a sidebar and header
        that frame this content.
      </p>
    </div>
  );
}

export const Default: Story = {
  render: () => (
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={["/chat"]}>
        <ConfigProvider>
          <AuthProvider>
            <PreferencesProvider>
              <ToastProvider>
                <ConfirmDialogProvider>
                  <ConversationsProvider>
                    <CommandPaletteProvider>
                      <Routes>
                        <Route element={<AppLayout />}>
                          <Route path="/chat" element={<MockContent />} />
                        </Route>
                      </Routes>
                    </CommandPaletteProvider>
                  </ConversationsProvider>
                </ConfirmDialogProvider>
              </ToastProvider>
            </PreferencesProvider>
          </AuthProvider>
        </ConfigProvider>
      </MemoryRouter>
    </QueryClientProvider>
  ),
};

export const AdminPage: Story = {
  render: () => (
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={["/admin"]}>
        <ConfigProvider>
          <AuthProvider>
            <PreferencesProvider>
              <ToastProvider>
                <ConfirmDialogProvider>
                  <ConversationsProvider>
                    <CommandPaletteProvider>
                      <Routes>
                        <Route element={<AppLayout />}>
                          <Route path="/admin" element={<MockContent />} />
                        </Route>
                      </Routes>
                    </CommandPaletteProvider>
                  </ConversationsProvider>
                </ConfirmDialogProvider>
              </ToastProvider>
            </PreferencesProvider>
          </AuthProvider>
        </ConfigProvider>
      </MemoryRouter>
    </QueryClientProvider>
  ),
};
