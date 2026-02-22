import { useEffect } from "react";

import { useAuth } from "@/auth";

import { client } from "./generated/client.gen";

/**
 * Hook to configure the API client with the current auth token.
 * This should be called once at the app root level.
 */
export function useConfigureClient() {
  const { token, method } = useAuth();

  useEffect(() => {
    // Configure the client with auth headers
    if (token && method !== "header") {
      client.setConfig({
        headers: {
          Authorization: `Bearer ${token}`,
        },
      });
    } else {
      // Clear auth header for header-based auth (the proxy sets headers)
      // or when not authenticated
      client.setConfig({
        headers: {},
      });
    }
  }, [token, method]);
}
