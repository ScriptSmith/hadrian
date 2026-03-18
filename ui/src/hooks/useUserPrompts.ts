import { useMemo } from "react";
import { useQuery, useQueries } from "@tanstack/react-query";

import {
  organizationListOptions,
  templateListByOrgOptions,
  templateListByUserOptions,
} from "@/api/generated/@tanstack/react-query.gen";
import type { Organization, Template } from "@/api/generated/types.gen";
import { useAuth } from "@/auth";

export interface TemplateWithContext extends Template {
  /** Which org this template is accessible through (for org/team/project-owned templates) */
  org_id?: string;
  org_slug?: string;
  org_name?: string;
}

export interface UseUserTemplatesResult {
  templates: TemplateWithContext[];
  organizations: Organization[];
  isLoading: boolean;
  error: Error | null;
  hasMore: boolean;
}

/**
 * Hook to fetch all templates accessible to the current user.
 *
 * Fetches:
 * - Templates owned by the user directly
 * - Templates from each organization the user can access (includes org, team, and project-scoped templates
 *   that the backend authorization layer makes visible)
 */
export function useUserTemplates(): UseUserTemplatesResult {
  const { user } = useAuth();

  // Fetch user's own templates
  const {
    data: userTemplatesData,
    isLoading: userTemplatesLoading,
    error: userTemplatesError,
  } = useQuery({
    ...templateListByUserOptions({
      path: { user_id: user?.id ?? "" },
      query: { limit: 50 },
    }),
    staleTime: 5 * 60 * 1000,
    enabled: !!user?.id,
  });

  // Fetch all organizations the user can access
  const {
    data: orgsData,
    isLoading: orgsLoading,
    error: orgsError,
  } = useQuery({
    ...organizationListOptions(),
    staleTime: 5 * 60 * 1000,
  });

  const organizations = useMemo(() => orgsData?.data ?? [], [orgsData?.data]);

  // Fetch templates for each organization (backend returns org + team + project templates the user can see)
  const orgQueries = useQueries({
    queries: organizations.map((org) => ({
      ...templateListByOrgOptions({
        path: { org_slug: org.slug },
        query: { limit: 50 },
      }),
      staleTime: 5 * 60 * 1000,
      enabled: organizations.length > 0,
    })),
  });

  // Combine all templates, deduplicating by id
  const templates = useMemo(() => {
    const seen = new Set<string>();
    const result: TemplateWithContext[] = [];

    // User-owned templates first
    for (const t of userTemplatesData?.data ?? []) {
      if (!seen.has(t.id)) {
        seen.add(t.id);
        result.push(t);
      }
    }

    // Then org-scoped templates
    for (let i = 0; i < organizations.length; i++) {
      const org = organizations[i];
      for (const t of orgQueries[i]?.data?.data ?? []) {
        if (!seen.has(t.id)) {
          seen.add(t.id);
          result.push({
            ...t,
            org_id: org.id,
            org_slug: org.slug,
            org_name: org.name,
          });
        }
      }
    }

    result.sort((a, b) => a.name.localeCompare(b.name));
    return result;
  }, [userTemplatesData?.data, organizations, orgQueries]);

  const isLoading = userTemplatesLoading || orgsLoading || orgQueries.some((q) => q.isLoading);
  const queryError = orgQueries.find((q) => q.error)?.error;
  const error = userTemplatesError ?? orgsError ?? queryError ?? null;
  const hasMore =
    (userTemplatesData?.pagination?.has_more ?? false) ||
    orgQueries.some((q) => q.data?.pagination?.has_more === true);

  return {
    templates,
    organizations,
    isLoading,
    error: error as Error | null,
    hasMore,
  };
}

// Backwards compat aliases
export type PromptWithOrg = TemplateWithContext;
export interface UseUserPromptsResult extends UseUserTemplatesResult {
  prompts: TemplateWithContext[];
}
export const useUserPrompts = (): UseUserPromptsResult => {
  const result = useUserTemplates();
  return { ...result, prompts: result.templates };
};
