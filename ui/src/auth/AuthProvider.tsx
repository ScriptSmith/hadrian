import { createContext, useCallback, useContext, useEffect, useMemo, useState } from "react";

import { useConfig } from "@/config/ConfigProvider";
import { useLocalStorage } from "@/hooks/useLocalStorage";

import type { AuthContextValue, AuthMethod, AuthState, LoginCredentials, User } from "./types";

export const AuthContext = createContext<AuthContextValue | null>(null);

const STORAGE_KEY = "hadrian-auth";

interface StoredAuth {
  method: AuthMethod;
  token: string;
  user?: User;
}

interface MeResponse {
  external_id: string;
  email?: string;
  name?: string;
  user_id?: string;
  roles?: string[];
}

/** Fetch current user identity from the server */
async function fetchMe(token?: string): Promise<User | null> {
  try {
    const headers: Record<string, string> = {};
    if (token) {
      headers.Authorization = `Bearer ${token}`;
    }
    const response = await fetch("/auth/me", {
      headers,
      credentials: "include",
    });
    if (response.ok) {
      const data: MeResponse = await response.json();
      return {
        id: data.user_id || data.external_id,
        email: data.email,
        name: data.name,
        roles: data.roles ?? [],
      };
    }
  } catch {
    // Failed to fetch user info - endpoint may not exist
  }
  return null;
}

export function AuthProvider({ children }: { children: React.ReactNode }) {
  const { config, isLoading: configLoading } = useConfig();
  const [storedAuth, setStoredAuth] = useLocalStorage<StoredAuth | null>(STORAGE_KEY, null);
  const [state, setState] = useState<AuthState>({
    isAuthenticated: false,
    isLoading: true,
    user: null,
    method: null,
    token: null,
  });

  // Check for header-based auth (zero-trust proxy)
  const checkHeaderAuth = useCallback(async (): Promise<{
    user: User;
    token: string;
  } | null> => {
    // In header auth mode, the proxy sets headers that the backend trusts
    // We can make a request to a "whoami" endpoint or just trust the UI config
    // For now, we'll check if header auth is available and make a test request
    if (!config?.auth.methods.includes("header")) {
      return null;
    }

    try {
      // Try to access an admin endpoint to see if we're authenticated via headers
      const response = await fetch("/admin/v1/organizations?limit=1", {
        credentials: "include",
      });

      if (response.ok) {
        // Fetch user info from /auth/me
        const user = await fetchMe();
        if (user) {
          return { user, token: "header-auth" };
        }
        // Fallback if /auth/me doesn't work
        const userEmail = response.headers.get("X-Forwarded-User");
        return {
          user: {
            id: userEmail || "header-user",
            email: userEmail || undefined,
          },
          token: "header-auth",
        };
      }
    } catch {
      // Header auth not working
    }

    return null;
  }, [config?.auth.methods]);

  // Initialize auth state
  useEffect(() => {
    if (configLoading) return;

    const initAuth = async () => {
      // Check if auth is disabled (none method)
      if (config?.auth.methods.includes("none")) {
        // Get user info from /auth/me - backend provides a default anonymous user
        const user = await fetchMe();
        setState({
          isAuthenticated: true,
          isLoading: false,
          user,
          method: "none",
          token: null,
        });
        return;
      }

      // First, check for header-based auth (zero-trust proxy)
      const headerAuth = await checkHeaderAuth();
      if (headerAuth) {
        setState({
          isAuthenticated: true,
          isLoading: false,
          user: headerAuth.user,
          method: "header",
          token: headerAuth.token,
        });
        return;
      }

      // Check for stored credentials
      if (storedAuth) {
        // Refresh user info from server (user_id may have changed)
        const user = await fetchMe(storedAuth.token);
        setState({
          isAuthenticated: true,
          isLoading: false,
          user: user || storedAuth.user || null,
          method: storedAuth.method,
          token: storedAuth.token,
        });
        // Update stored auth with fresh user info
        if (user && (!storedAuth.user || storedAuth.user.id !== user.id)) {
          setStoredAuth({ ...storedAuth, user });
        }
        return;
      }

      // Check for OIDC session by calling /auth/me
      // This works after the backend's /auth/callback sets the session cookie
      if (config?.auth.methods.includes("oidc")) {
        const user = await fetchMe();
        if (user) {
          setState({
            isAuthenticated: true,
            isLoading: false,
            user,
            method: "oidc",
            token: null, // Session is cookie-based
          });
          return;
        }
      }

      // No authentication found
      setState({
        isAuthenticated: false,
        isLoading: false,
        user: null,
        method: null,
        token: null,
      });
    };

    initAuth();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [configLoading, config?.auth.methods]);

  const login = useCallback(
    async (method: AuthMethod, credentials?: LoginCredentials): Promise<void> => {
      setState((prev) => ({ ...prev, isLoading: true }));

      try {
        if (method === "api_key" && credentials?.apiKey) {
          // Validate API key by making a test request
          const response = await fetch("/admin/v1/organizations?limit=1", {
            headers: {
              Authorization: `Bearer ${credentials.apiKey}`,
            },
          });

          if (!response.ok) {
            throw new Error("Invalid API key");
          }

          // Fetch user info
          const user = await fetchMe(credentials.apiKey);

          const authData: StoredAuth = {
            method: "api_key",
            token: credentials.apiKey,
            user: user || undefined,
          };

          setStoredAuth(authData);
          setState({
            isAuthenticated: true,
            isLoading: false,
            user,
            method: "api_key",
            token: credentials.apiKey,
          });
        } else if (method === "oidc") {
          // Redirect to backend's OIDC login endpoint
          // The backend handles PKCE and state management
          // If orgId is provided, use per-organization SSO
          const url = credentials?.orgId
            ? `/auth/login?org=${encodeURIComponent(credentials.orgId)}`
            : "/auth/login";
          window.location.href = url;
        } else {
          throw new Error("Invalid auth method or missing credentials");
        }
      } catch (error) {
        setState({
          isAuthenticated: false,
          isLoading: false,
          user: null,
          method: null,
          token: null,
        });
        throw error;
      }
    },
    [setStoredAuth]
  );

  const logout = useCallback(() => {
    setStoredAuth(null);
    setState({
      isAuthenticated: false,
      isLoading: false,
      user: null,
      method: null,
      token: null,
    });

    // For OIDC, we might want to redirect to the logout endpoint
    if (state.method === "oidc" && config?.auth.oidc) {
      // Most OIDC providers have a logout endpoint
      const logoutUrl = config.auth.oidc.authorization_url.replace("/auth", "/logout");
      window.location.href = `${logoutUrl}?redirect_uri=${encodeURIComponent(window.location.origin)}`;
    }
  }, [config?.auth.oidc, setStoredAuth, state.method]);

  const setApiKey = useCallback(
    (apiKey: string) => {
      const authData: StoredAuth = {
        method: "api_key",
        token: apiKey,
      };
      setStoredAuth(authData);
      setState({
        isAuthenticated: true,
        isLoading: false,
        user: null,
        method: "api_key",
        token: apiKey,
      });
    },
    [setStoredAuth]
  );

  const value = useMemo<AuthContextValue>(
    () => ({
      ...state,
      login,
      logout,
      setApiKey,
    }),
    [state, login, logout, setApiKey]
  );

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
}

export function useAuth(): AuthContextValue {
  const context = useContext(AuthContext);
  if (!context) {
    throw new Error("useAuth must be used within an AuthProvider");
  }
  return context;
}
