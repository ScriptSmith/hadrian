import { useMutation } from "@tanstack/react-query";

import type { DiscoveryResult } from "./types";

interface DiscoverSsoError {
  code: string;
  message: string;
}

/**
 * Hook for discovering SSO configuration based on email address.
 *
 * Used in the login flow to determine if a user's email domain has
 * per-organization SSO configured, and if so, which IdP to use.
 *
 * @example
 * ```tsx
 * const { mutate: discover, data, isLoading, error } = useDiscoverSso();
 *
 * const handleEmailSubmit = (email: string) => {
 *   discover(email, {
 *     onSuccess: (result) => {
 *       if (result.has_sso) {
 *         // Redirect to org-specific IdP
 *         login("oidc", { orgId: result.org_id });
 *       }
 *     },
 *   });
 * };
 * ```
 */
export function useDiscoverSso() {
  return useMutation<DiscoveryResult, DiscoverSsoError, string>({
    mutationFn: async (email: string) => {
      const response = await fetch(`/auth/discover?email=${encodeURIComponent(email)}`);

      if (!response.ok) {
        const errorData = await response.json().catch(() => ({}));
        throw {
          code: errorData.error?.code || "discovery_failed",
          message: errorData.error?.message || "Failed to discover SSO configuration",
        };
      }

      return response.json();
    },
  });
}
