import type { Meta, StoryObj } from "@storybook/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter, Routes, Route } from "react-router-dom";
import { AdminLayout } from "./AdminLayout";
import { AuthProvider } from "@/auth";
import { ConfigProvider } from "@/config/ConfigProvider";
import { PreferencesProvider } from "@/preferences/PreferencesProvider";
import { CommandPaletteProvider } from "@/components/CommandPalette/CommandPalette";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: false,
    },
  },
});

// Sample page content for stories
function SampleDashboard() {
  return (
    <div className="p-6">
      <h1 className="text-2xl font-bold mb-4">Dashboard</h1>
      <p className="text-muted-foreground">Welcome to the admin dashboard.</p>
    </div>
  );
}

function SampleOrganizations() {
  return (
    <div className="p-6">
      <h1 className="text-2xl font-bold mb-4">Organizations</h1>
      <p className="text-muted-foreground">Manage your organizations here.</p>
    </div>
  );
}

function SampleUsers() {
  return (
    <div className="p-6">
      <h1 className="text-2xl font-bold mb-4">Users</h1>
      <p className="text-muted-foreground">Manage users and permissions.</p>
    </div>
  );
}

const meta: Meta<typeof AdminLayout> = {
  title: "Layout/AdminLayout",
  component: AdminLayout,
  parameters: {
    layout: "fullscreen",
  },
  decorators: [
    (Story, context) => {
      const initialEntries = context.parameters.initialEntries || ["/admin"];
      return (
        <QueryClientProvider client={queryClient}>
          <MemoryRouter initialEntries={initialEntries}>
            <ConfigProvider>
              <AuthProvider>
                <PreferencesProvider>
                  <CommandPaletteProvider>
                    <div style={{ height: "100vh" }}>
                      <Routes>
                        <Route element={<Story />}>
                          <Route path="/admin" element={<SampleDashboard />} />
                          <Route path="/admin/organizations" element={<SampleOrganizations />} />
                          <Route path="/admin/users" element={<SampleUsers />} />
                          <Route path="/admin/*" element={<SampleDashboard />} />
                        </Route>
                      </Routes>
                    </div>
                  </CommandPaletteProvider>
                </PreferencesProvider>
              </AuthProvider>
            </ConfigProvider>
          </MemoryRouter>
        </QueryClientProvider>
      );
    },
  ],
};

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  parameters: {
    initialEntries: ["/admin"],
  },
};

export const OnOrganizations: Story = {
  parameters: {
    initialEntries: ["/admin/organizations"],
  },
};

export const OnUsers: Story = {
  parameters: {
    initialEntries: ["/admin/users"],
  },
};

export const OnProviders: Story = {
  parameters: {
    initialEntries: ["/admin/providers"],
  },
};

export const OnSettings: Story = {
  parameters: {
    initialEntries: ["/admin/settings"],
  },
};

export const WithChildren: Story = {
  args: {
    children: (
      <div className="p-6">
        <h1 className="text-2xl font-bold mb-4">Custom Content</h1>
        <p className="text-muted-foreground">
          This demonstrates passing children directly to AdminLayout instead of using Outlet.
        </p>
      </div>
    ),
  },
};
