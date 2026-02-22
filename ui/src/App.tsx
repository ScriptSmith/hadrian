import { BrowserRouter, Routes, Route, Navigate } from "react-router-dom";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { ConfigProvider } from "@/config/ConfigProvider";
import { PreferencesProvider } from "@/preferences/PreferencesProvider";
import { AuthProvider, RequireAuth, RequireAdmin } from "@/auth";
import { ApiClientProvider } from "@/api/ApiClientProvider";
import { ToastProvider } from "@/components/Toast/Toast";
import { ConfirmDialogProvider } from "@/components/ConfirmDialog/ConfirmDialog";
import { CommandPaletteProvider } from "@/components/CommandPalette/CommandPalette";
import { ConversationsProvider } from "@/components/ConversationsProvider/ConversationsProvider";
import { ErrorBoundary } from "@/components/ErrorBoundary/ErrorBoundary";
import { AppLayout } from "@/components/AppLayout/AppLayout";
import { AdminLayout } from "@/components/AdminLayout/AdminLayout";

// Pages - lazy loaded for code splitting
import { lazy, Suspense } from "react";
import { Spinner } from "@/components/Spinner/Spinner";

const LoginPage = lazy(() => import("@/pages/LoginPage"));
const AccountPage = lazy(() => import("@/pages/AccountPage"));
const ProjectsPage = lazy(() => import("@/pages/ProjectsPage"));
const TeamsPage = lazy(() => import("@/pages/TeamsPage"));
const KnowledgeBasesPage = lazy(() => import("@/pages/KnowledgeBasesPage"));
const ApiKeysPage = lazy(() => import("@/pages/ApiKeysPage"));

const MyUsagePage = lazy(() => import("@/pages/MyUsagePage"));
const MyProvidersPage = lazy(() => import("@/pages/MyProvidersPage"));
const SelfServiceProjectDetailPage = lazy(() => import("@/pages/project/ProjectDetailPage"));
const StudioPage = lazy(() => import("@/pages/studio/StudioPage"));
const ChatPage = lazy(() => import("@/pages/chat/ChatPage"));
const AdminDashboardPage = lazy(() => import("@/pages/admin/DashboardPage"));
const OrganizationsPage = lazy(() => import("@/pages/admin/OrganizationsPage"));
const OrganizationDetailPage = lazy(() => import("@/pages/admin/OrganizationDetailPage"));
const ProjectDetailPage = lazy(() => import("@/pages/admin/ProjectDetailPage"));
const UsersPage = lazy(() => import("@/pages/admin/UsersPage"));
const UserDetailPage = lazy(() => import("@/pages/admin/UserDetailPage"));
const AdminApiKeysPage = lazy(() => import("@/pages/admin/ApiKeysPage"));
const ProvidersPage = lazy(() => import("@/pages/admin/ProvidersPage"));
const ProviderHealthPage = lazy(() => import("@/pages/admin/ProviderHealthPage"));
const ProviderDetailPage = lazy(() => import("@/pages/admin/ProviderDetailPage"));
const PricingPage = lazy(() => import("@/pages/admin/PricingPage"));
const UsagePage = lazy(() => import("@/pages/admin/UsagePage"));
const AdminProjectsPage = lazy(() => import("@/pages/admin/ProjectsPage"));
const AdminTeamsPage = lazy(() => import("@/pages/admin/TeamsPage"));
const ServiceAccountsPage = lazy(() => import("@/pages/admin/ServiceAccountsPage"));
const TeamDetailPage = lazy(() => import("@/pages/admin/TeamDetailPage"));
const SettingsPage = lazy(() => import("@/pages/admin/SettingsPage"));
const AuditLogsPage = lazy(() => import("@/pages/admin/AuditLogsPage"));
const VectorStoresPage = lazy(() => import("@/pages/admin/VectorStoresPage"));
const VectorStoreDetailPage = lazy(() => import("@/pages/admin/VectorStoreDetailPage"));
const SsoConnectionsPage = lazy(() => import("@/pages/admin/SsoConnectionsPage"));
const SsoGroupMappingsPage = lazy(() => import("@/pages/admin/SsoGroupMappingsPage"));
const OrgSsoConfigPage = lazy(() => import("@/pages/admin/OrgSsoConfigPage"));
const ScimConfigPage = lazy(() => import("@/pages/admin/ScimConfigPage"));
const OrgRbacPoliciesPage = lazy(() => import("@/pages/admin/OrgRbacPoliciesPage"));
const SessionInfoPage = lazy(() => import("@/pages/admin/SessionInfoPage"));

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 1000 * 60, // 1 minute
      retry: 1,
    },
  },
});

function PageLoader() {
  return (
    <div className="flex h-full items-center justify-center">
      <Spinner size="lg" />
    </div>
  );
}

export default function App() {
  return (
    <ErrorBoundary>
      <QueryClientProvider client={queryClient}>
        <ConfigProvider>
          <PreferencesProvider>
            <AuthProvider>
              <ApiClientProvider>
                <ToastProvider>
                  <ConfirmDialogProvider>
                    <CommandPaletteProvider>
                      <ConversationsProvider>
                        <BrowserRouter>
                          <Routes>
                            {/* Root redirect */}
                            <Route path="/" element={<Navigate to="/chat" replace />} />

                            {/* Login route */}
                            <Route
                              path="/login"
                              element={
                                <Suspense fallback={<PageLoader />}>
                                  <LoginPage />
                                </Suspense>
                              }
                            />

                            {/* Auth callback route for OIDC */}
                            <Route
                              path="/auth/callback"
                              element={
                                <Suspense fallback={<PageLoader />}>
                                  <LoginPage />
                                </Suspense>
                              }
                            />

                            {/* Protected routes with main AppLayout (chat sidebar) */}
                            <Route
                              element={
                                <RequireAuth>
                                  <AppLayout />
                                </RequireAuth>
                              }
                            >
                              {/* Chat routes */}
                              <Route
                                path="/chat"
                                element={
                                  <Suspense fallback={<PageLoader />}>
                                    <ChatPage />
                                  </Suspense>
                                }
                              />
                              <Route
                                path="/chat/:conversationId"
                                element={
                                  <Suspense fallback={<PageLoader />}>
                                    <ChatPage />
                                  </Suspense>
                                }
                              />

                              {/* Projects route */}
                              <Route
                                path="/projects"
                                element={
                                  <Suspense fallback={<PageLoader />}>
                                    <ProjectsPage />
                                  </Suspense>
                                }
                              />

                              {/* Project detail route */}
                              <Route
                                path="/projects/:orgSlug/:projectSlug"
                                element={
                                  <Suspense fallback={<PageLoader />}>
                                    <SelfServiceProjectDetailPage />
                                  </Suspense>
                                }
                              />

                              {/* Teams route */}
                              <Route
                                path="/teams"
                                element={
                                  <Suspense fallback={<PageLoader />}>
                                    <TeamsPage />
                                  </Suspense>
                                }
                              />

                              {/* Knowledge Bases route */}
                              <Route
                                path="/knowledge-bases"
                                element={
                                  <Suspense fallback={<PageLoader />}>
                                    <KnowledgeBasesPage />
                                  </Suspense>
                                }
                              />

                              {/* API Keys route */}
                              <Route
                                path="/api-keys"
                                element={
                                  <Suspense fallback={<PageLoader />}>
                                    <ApiKeysPage />
                                  </Suspense>
                                }
                              />

                              {/* Providers route (self-service) */}
                              <Route
                                path="/providers"
                                element={
                                  <Suspense fallback={<PageLoader />}>
                                    <MyProvidersPage />
                                  </Suspense>
                                }
                              />

                              {/* Usage route (self-service) */}
                              <Route
                                path="/usage"
                                element={
                                  <Suspense fallback={<PageLoader />}>
                                    <MyUsagePage />
                                  </Suspense>
                                }
                              />

                              {/* Studio route */}
                              <Route
                                path="/studio"
                                element={
                                  <Suspense fallback={<PageLoader />}>
                                    <StudioPage />
                                  </Suspense>
                                }
                              />

                              {/* Account settings route */}
                              <Route
                                path="/account"
                                element={
                                  <Suspense fallback={<PageLoader />}>
                                    <AccountPage />
                                  </Suspense>
                                }
                              />

                              {/* Session info route (debugging) */}
                              <Route
                                path="/session"
                                element={
                                  <Suspense fallback={<PageLoader />}>
                                    <SessionInfoPage />
                                  </Suspense>
                                }
                              />
                            </Route>

                            {/* Admin routes with AdminLayout (admin sidebar) */}
                            <Route
                              element={
                                <RequireAdmin>
                                  <AdminLayout />
                                </RequireAdmin>
                              }
                            >
                              <Route
                                path="/admin"
                                element={
                                  <Suspense fallback={<PageLoader />}>
                                    <AdminDashboardPage />
                                  </Suspense>
                                }
                              />
                              <Route
                                path="/admin/organizations"
                                element={
                                  <Suspense fallback={<PageLoader />}>
                                    <OrganizationsPage />
                                  </Suspense>
                                }
                              />
                              <Route
                                path="/admin/organizations/:slug"
                                element={
                                  <Suspense fallback={<PageLoader />}>
                                    <OrganizationDetailPage />
                                  </Suspense>
                                }
                              />
                              <Route
                                path="/admin/organizations/:orgSlug/projects/:projectSlug"
                                element={
                                  <Suspense fallback={<PageLoader />}>
                                    <ProjectDetailPage />
                                  </Suspense>
                                }
                              />
                              <Route
                                path="/admin/users"
                                element={
                                  <Suspense fallback={<PageLoader />}>
                                    <UsersPage />
                                  </Suspense>
                                }
                              />
                              <Route
                                path="/admin/users/:userId"
                                element={
                                  <Suspense fallback={<PageLoader />}>
                                    <UserDetailPage />
                                  </Suspense>
                                }
                              />
                              <Route
                                path="/admin/sso"
                                element={
                                  <Suspense fallback={<PageLoader />}>
                                    <SsoConnectionsPage />
                                  </Suspense>
                                }
                              />
                              <Route
                                path="/admin/organizations/:orgSlug/sso-group-mappings"
                                element={
                                  <Suspense fallback={<PageLoader />}>
                                    <SsoGroupMappingsPage />
                                  </Suspense>
                                }
                              />
                              <Route
                                path="/admin/organizations/:orgSlug/sso-config"
                                element={
                                  <Suspense fallback={<PageLoader />}>
                                    <OrgSsoConfigPage />
                                  </Suspense>
                                }
                              />
                              <Route
                                path="/admin/organizations/:orgSlug/scim-config"
                                element={
                                  <Suspense fallback={<PageLoader />}>
                                    <ScimConfigPage />
                                  </Suspense>
                                }
                              />
                              <Route
                                path="/admin/organizations/:orgSlug/rbac-policies"
                                element={
                                  <Suspense fallback={<PageLoader />}>
                                    <OrgRbacPoliciesPage />
                                  </Suspense>
                                }
                              />
                              <Route
                                path="/admin/api-keys"
                                element={
                                  <Suspense fallback={<PageLoader />}>
                                    <AdminApiKeysPage />
                                  </Suspense>
                                }
                              />
                              <Route
                                path="/admin/providers"
                                element={
                                  <Suspense fallback={<PageLoader />}>
                                    <ProvidersPage />
                                  </Suspense>
                                }
                              />
                              <Route
                                path="/admin/provider-health"
                                element={
                                  <Suspense fallback={<PageLoader />}>
                                    <ProviderHealthPage />
                                  </Suspense>
                                }
                              />
                              <Route
                                path="/admin/provider-health/:providerName"
                                element={
                                  <Suspense fallback={<PageLoader />}>
                                    <ProviderDetailPage />
                                  </Suspense>
                                }
                              />
                              <Route
                                path="/admin/pricing"
                                element={
                                  <Suspense fallback={<PageLoader />}>
                                    <PricingPage />
                                  </Suspense>
                                }
                              />
                              <Route
                                path="/admin/usage"
                                element={
                                  <Suspense fallback={<PageLoader />}>
                                    <UsagePage />
                                  </Suspense>
                                }
                              />
                              <Route
                                path="/admin/projects"
                                element={
                                  <Suspense fallback={<PageLoader />}>
                                    <AdminProjectsPage />
                                  </Suspense>
                                }
                              />
                              <Route
                                path="/admin/teams"
                                element={
                                  <Suspense fallback={<PageLoader />}>
                                    <AdminTeamsPage />
                                  </Suspense>
                                }
                              />
                              <Route
                                path="/admin/service-accounts"
                                element={
                                  <Suspense fallback={<PageLoader />}>
                                    <ServiceAccountsPage />
                                  </Suspense>
                                }
                              />
                              <Route
                                path="/admin/organizations/:orgSlug/teams/:teamSlug"
                                element={
                                  <Suspense fallback={<PageLoader />}>
                                    <TeamDetailPage />
                                  </Suspense>
                                }
                              />
                              <Route
                                path="/admin/settings"
                                element={
                                  <Suspense fallback={<PageLoader />}>
                                    <SettingsPage />
                                  </Suspense>
                                }
                              />
                              <Route
                                path="/admin/audit-logs"
                                element={
                                  <Suspense fallback={<PageLoader />}>
                                    <AuditLogsPage />
                                  </Suspense>
                                }
                              />
                              <Route
                                path="/admin/vector-stores"
                                element={
                                  <Suspense fallback={<PageLoader />}>
                                    <VectorStoresPage />
                                  </Suspense>
                                }
                              />
                              <Route
                                path="/admin/vector-stores/:vectorStoreId"
                                element={
                                  <Suspense fallback={<PageLoader />}>
                                    <VectorStoreDetailPage />
                                  </Suspense>
                                }
                              />
                            </Route>

                            {/* Catch all - redirect to chat */}
                            <Route path="*" element={<Navigate to="/chat" replace />} />
                          </Routes>
                        </BrowserRouter>
                      </ConversationsProvider>
                    </CommandPaletteProvider>
                  </ConfirmDialogProvider>
                </ToastProvider>
              </ApiClientProvider>
            </AuthProvider>
          </PreferencesProvider>
        </ConfigProvider>
      </QueryClientProvider>
    </ErrorBoundary>
  );
}
