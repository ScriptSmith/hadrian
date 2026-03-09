import { BrowserRouter } from "react-router-dom";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { ConfigProvider } from "@/config/ConfigProvider";
import { PreferencesProvider } from "@/preferences/PreferencesProvider";
import { AuthProvider } from "@/auth";
import { ApiClientProvider } from "@/api/ApiClientProvider";
import { ToastProvider } from "@/components/Toast/Toast";
import { ConfirmDialogProvider } from "@/components/ConfirmDialog/ConfirmDialog";
import { CommandPaletteProvider } from "@/components/CommandPalette/CommandPalette";
import { ConversationsProvider } from "@/components/ConversationsProvider/ConversationsProvider";
import { ErrorBoundary } from "@/components/ErrorBoundary/ErrorBoundary";
import { WasmSetupGuard } from "@/components/WasmSetup/WasmSetupGuard";
import { AppRoutes } from "@/routes/AppRoutes";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 1000 * 60, // 1 minute
      retry: 1,
    },
  },
});

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
                      <WasmSetupGuard>
                        <ConversationsProvider>
                          <BrowserRouter>
                            <AppRoutes />
                          </BrowserRouter>
                        </ConversationsProvider>
                      </WasmSetupGuard>
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
