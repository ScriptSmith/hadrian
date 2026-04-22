import { useMemo } from "react";
import { useQuery, useQueries } from "@tanstack/react-query";

import {
  organizationListOptions,
  skillListByOrgOptions,
  skillListByUserOptions,
} from "@/api/generated/@tanstack/react-query.gen";
import type { Organization, Skill } from "@/api/generated/types.gen";
import { useAuth } from "@/auth";

export interface SkillWithContext extends Skill {
  /** Org this skill is accessible through (for org/team/project-owned skills). */
  org_id?: string;
  org_slug?: string;
  org_name?: string;
}

export interface UseUserSkillsResult {
  skills: SkillWithContext[];
  organizations: Organization[];
  isLoading: boolean;
  error: Error | null;
  hasMore: boolean;
}

/**
 * Fetch every skill accessible to the current user: their own, plus skills
 * reachable through each organization they belong to (the backend returns
 * org- team- and project-scoped skills together). Deduplicated by id.
 *
 * Skills are returned with `files_manifest` populated but file contents
 * omitted — call `skillGet` for the full body.
 */
export function useUserSkills(): UseUserSkillsResult {
  const { user } = useAuth();

  const {
    data: userSkillsData,
    isLoading: userSkillsLoading,
    error: userSkillsError,
  } = useQuery({
    ...skillListByUserOptions({
      path: { user_id: user?.id ?? "" },
      query: { limit: 50 },
    }),
    staleTime: 5 * 60 * 1000,
    enabled: !!user?.id,
  });

  const {
    data: orgsData,
    isLoading: orgsLoading,
    error: orgsError,
  } = useQuery({
    ...organizationListOptions(),
    staleTime: 5 * 60 * 1000,
  });

  const organizations = useMemo(() => orgsData?.data ?? [], [orgsData?.data]);

  const orgQueries = useQueries({
    queries: organizations.map((org) => ({
      ...skillListByOrgOptions({
        path: { org_slug: org.slug },
        query: { limit: 50 },
      }),
      staleTime: 5 * 60 * 1000,
      enabled: organizations.length > 0,
    })),
  });

  const skills = useMemo(() => {
    const seen = new Set<string>();
    const result: SkillWithContext[] = [];

    for (const s of userSkillsData?.data ?? []) {
      if (!seen.has(s.id)) {
        seen.add(s.id);
        result.push(s);
      }
    }

    for (let i = 0; i < organizations.length; i++) {
      const org = organizations[i];
      for (const s of orgQueries[i]?.data?.data ?? []) {
        if (!seen.has(s.id)) {
          seen.add(s.id);
          result.push({
            ...s,
            org_id: org.id,
            org_slug: org.slug,
            org_name: org.name,
          });
        }
      }
    }

    result.sort((a, b) => a.name.localeCompare(b.name));
    return result;
  }, [userSkillsData?.data, organizations, orgQueries]);

  const isLoading = userSkillsLoading || orgsLoading || orgQueries.some((q) => q.isLoading);
  const queryError = orgQueries.find((q) => q.error)?.error;
  const error = userSkillsError ?? orgsError ?? queryError ?? null;
  const hasMore =
    (userSkillsData?.pagination?.has_more ?? false) ||
    orgQueries.some((q) => q.data?.pagination?.has_more === true);

  return {
    skills,
    organizations,
    isLoading,
    error: error as Error | null,
    hasMore,
  };
}
