import { useConfigureClient } from "./useConfigureClient";

interface ApiClientProviderProps {
  children: React.ReactNode;
}

/**
 * Component that configures the API client with auth credentials.
 * This should be rendered inside the AuthProvider so it has access to auth state.
 */
export function ApiClientProvider({ children }: ApiClientProviderProps) {
  useConfigureClient();
  return <>{children}</>;
}
