import type { Meta, StoryObj } from "@storybook/react";
import { MemoryRouter, Routes, Route, useLocation } from "react-router-dom";
import { RequireAdmin } from "./RequireAdmin";
import { AuthContext, AuthContextValue } from "./AuthProvider";
import type { User } from "./types";

// Helper component to show the current location for redirect stories
function LocationDisplay() {
  const location = useLocation();
  return (
    <div className="p-4 bg-zinc-100 dark:bg-zinc-800 rounded-lg">
      <p className="text-sm text-zinc-600 dark:text-zinc-400">Current location:</p>
      <code className="text-lg font-mono">{location.pathname}</code>
    </div>
  );
}

// Protected content shown when admin access is granted
function ProtectedContent() {
  return (
    <div className="p-6 bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800 rounded-lg">
      <h2 className="text-lg font-semibold text-green-800 dark:text-green-200">Admin Area</h2>
      <p className="text-green-700 dark:text-green-300 mt-2">
        You have successfully accessed the admin-protected content.
      </p>
    </div>
  );
}

// Mock auth context provider
function MockAuthProvider({
  children,
  value,
}: {
  children: React.ReactNode;
  value: AuthContextValue;
}) {
  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
}

// Create mock auth context values
const createMockAuth = (overrides: Partial<AuthContextValue> = {}): AuthContextValue => ({
  isAuthenticated: false,
  isLoading: false,
  user: null,
  availableMethods: [],
  login: async () => {},
  loginWithApiKey: async () => {},
  logout: async () => {},
  ...overrides,
});

const adminUser: User = {
  id: "user-1",
  email: "admin@example.com",
  name: "Admin User",
  roles: ["super_admin"],
};

const regularUser: User = {
  id: "user-2",
  email: "user@example.com",
  name: "Regular User",
  roles: ["user"],
};

const orgAdminUser: User = {
  id: "user-3",
  email: "orgadmin@example.com",
  name: "Org Admin User",
  roles: ["org_admin"],
};

const meta: Meta<typeof RequireAdmin> = {
  title: "Auth/RequireAdmin",
  component: RequireAdmin,
  parameters: {
    layout: "centered",
  },
};

export default meta;
type Story = StoryObj<typeof RequireAdmin>;

// Story: Loading state shows spinner
export const Loading: Story = {
  render: () => (
    <MockAuthProvider value={createMockAuth({ isLoading: true })}>
      <MemoryRouter initialEntries={["/admin"]}>
        <RequireAdmin>
          <ProtectedContent />
        </RequireAdmin>
      </MemoryRouter>
    </MockAuthProvider>
  ),
};

// Story: Unauthenticated user is redirected to /login
export const Unauthenticated: Story = {
  render: () => (
    <MockAuthProvider value={createMockAuth({ isAuthenticated: false })}>
      <MemoryRouter initialEntries={["/admin"]}>
        <Routes>
          <Route
            path="/admin"
            element={
              <RequireAdmin>
                <ProtectedContent />
              </RequireAdmin>
            }
          />
          <Route path="/login" element={<LocationDisplay />} />
        </Routes>
      </MemoryRouter>
    </MockAuthProvider>
  ),
};

// Story: Authenticated but without admin role is redirected to /chat
export const AuthenticatedNoAdminAccess: Story = {
  render: () => (
    <MockAuthProvider
      value={createMockAuth({
        isAuthenticated: true,
        user: regularUser,
      })}
    >
      <MemoryRouter initialEntries={["/admin"]}>
        <Routes>
          <Route
            path="/admin"
            element={
              <RequireAdmin>
                <ProtectedContent />
              </RequireAdmin>
            }
          />
          <Route path="/chat" element={<LocationDisplay />} />
        </Routes>
      </MemoryRouter>
    </MockAuthProvider>
  ),
};

// Story: Super admin can access protected content
export const SuperAdmin: Story = {
  render: () => (
    <MockAuthProvider
      value={createMockAuth({
        isAuthenticated: true,
        user: adminUser,
      })}
    >
      <MemoryRouter initialEntries={["/admin"]}>
        <RequireAdmin>
          <ProtectedContent />
        </RequireAdmin>
      </MemoryRouter>
    </MockAuthProvider>
  ),
};

// Story: Org admin can access protected content
export const OrgAdmin: Story = {
  render: () => (
    <MockAuthProvider
      value={createMockAuth({
        isAuthenticated: true,
        user: orgAdminUser,
      })}
    >
      <MemoryRouter initialEntries={["/admin"]}>
        <RequireAdmin>
          <ProtectedContent />
        </RequireAdmin>
      </MemoryRouter>
    </MockAuthProvider>
  ),
};

// Story: Team admin can access protected content
export const TeamAdmin: Story = {
  render: () => (
    <MockAuthProvider
      value={createMockAuth({
        isAuthenticated: true,
        user: {
          id: "user-4",
          email: "teamadmin@example.com",
          name: "Team Admin User",
          roles: ["team_admin"],
        },
      })}
    >
      <MemoryRouter initialEntries={["/admin"]}>
        <RequireAdmin>
          <ProtectedContent />
        </RequireAdmin>
      </MemoryRouter>
    </MockAuthProvider>
  ),
};
