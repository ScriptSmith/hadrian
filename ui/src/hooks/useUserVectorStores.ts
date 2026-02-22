import { useQuery, useQueries } from "@tanstack/react-query";

import {
  organizationListOptions,
  vectorStoreListOptions,
} from "@/api/generated/@tanstack/react-query.gen";
import type { VectorStore, Organization } from "@/api/generated/types.gen";

export interface VectorStoreWithOrg extends VectorStore {
  org_id: string;
  org_slug: string;
  org_name: string;
}

export interface UseUserVectorStoresResult {
  vectorStores: VectorStoreWithOrg[];
  organizations: Organization[];
  isLoading: boolean;
  error: Error | null;
}

/**
 * Hook to fetch all vector stores accessible to the current user.
 *
 * This fetches all organizations the user can access, then fetches
 * vector stores for each organization. The authorization layer on the
 * backend filters what the user can see based on their permissions.
 */
export function useUserVectorStores(): UseUserVectorStoresResult {
  // First, fetch all organizations the user can access
  const {
    data: orgsData,
    isLoading: orgsLoading,
    error: orgsError,
  } = useQuery({
    ...organizationListOptions(),
    staleTime: 5 * 60 * 1000, // Cache for 5 minutes
  });

  const organizations = orgsData?.data ?? [];

  // Then fetch vector stores for each organization
  const vectorStoreQueries = useQueries({
    queries: organizations.map((org) => ({
      ...vectorStoreListOptions({
        query: {
          owner_type: "organization",
          owner_id: org.id,
          limit: 50, // Get up to 50 vector stores per org
        },
      }),
      staleTime: 5 * 60 * 1000,
      enabled: organizations.length > 0,
    })),
  });

  // Combine all vector stores with their org info
  const vectorStores: VectorStoreWithOrg[] = [];
  for (let i = 0; i < organizations.length; i++) {
    const org = organizations[i];
    const storesData = vectorStoreQueries[i]?.data?.data ?? [];
    for (const store of storesData) {
      vectorStores.push({
        ...store,
        org_id: org.id,
        org_slug: org.slug,
        org_name: org.name,
      });
    }
  }

  // Sort vector stores by name
  vectorStores.sort((a, b) => a.name.localeCompare(b.name));

  const isLoading = orgsLoading || vectorStoreQueries.some((q) => q.isLoading);
  const storeError = vectorStoreQueries.find((q) => q.error)?.error;
  const error = orgsError ?? storeError ?? null;

  return {
    vectorStores,
    organizations,
    isLoading,
    error: error as Error | null,
  };
}
