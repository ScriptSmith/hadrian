import { useMemo } from "react";
import { useQuery, useQueries } from "@tanstack/react-query";

import {
  organizationListOptions,
  promptListByOrgOptions,
} from "@/api/generated/@tanstack/react-query.gen";
import type { Organization, Prompt } from "@/api/generated/types.gen";

export interface PromptWithOrg extends Prompt {
  org_id: string;
  org_slug: string;
  org_name: string;
}

export interface UseUserPromptsResult {
  prompts: PromptWithOrg[];
  organizations: Organization[];
  isLoading: boolean;
  error: Error | null;
}

/**
 * Hook to fetch all prompts accessible to the current user.
 *
 * This fetches all organizations the user can access, then fetches
 * prompts for each organization. The authorization layer on the
 * backend filters what the user can see based on their permissions.
 */
export function useUserPrompts(): UseUserPromptsResult {
  // First, fetch all organizations the user can access
  const {
    data: orgsData,
    isLoading: orgsLoading,
    error: orgsError,
  } = useQuery({
    ...organizationListOptions(),
    staleTime: 5 * 60 * 1000, // Cache for 5 minutes
  });

  const organizations = useMemo(() => orgsData?.data ?? [], [orgsData?.data]);

  // Then fetch prompts for each organization
  const promptQueries = useQueries({
    queries: organizations.map((org) => ({
      ...promptListByOrgOptions({
        path: { org_slug: org.slug },
        query: { limit: 50 }, // Get up to 50 prompts per org
      }),
      staleTime: 5 * 60 * 1000,
      enabled: organizations.length > 0,
    })),
  });

  // Combine all prompts with their org info
  const prompts = useMemo(() => {
    const result: PromptWithOrg[] = [];
    for (let i = 0; i < organizations.length; i++) {
      const org = organizations[i];
      const promptsData = promptQueries[i]?.data?.data ?? [];
      for (const prompt of promptsData) {
        result.push({
          ...prompt,
          org_id: org.id,
          org_slug: org.slug,
          org_name: org.name,
        });
      }
    }
    // Sort prompts by name
    result.sort((a, b) => a.name.localeCompare(b.name));
    return result;
  }, [organizations, promptQueries]);

  const isLoading = orgsLoading || promptQueries.some((q) => q.isLoading);
  const promptError = promptQueries.find((q) => q.error)?.error;
  const error = orgsError ?? promptError ?? null;

  return {
    prompts,
    organizations,
    isLoading,
    error: error as Error | null,
  };
}
