import { useMemo } from "react";
import { useQuery, useQueries } from "@tanstack/react-query";

import {
  organizationListOptions,
  projectListOptions,
} from "@/api/generated/@tanstack/react-query.gen";
import type { Organization, Project } from "@/api/generated/types.gen";

export interface ProjectWithOrg extends Project {
  org_slug: string;
  org_name: string;
}

export interface UseUserProjectsResult {
  projects: ProjectWithOrg[];
  organizations: Organization[];
  isLoading: boolean;
  error: Error | null;
}

/**
 * Hook to fetch all projects accessible to the current user.
 *
 * This fetches all organizations the user can access, then fetches
 * projects for each organization. The authorization layer on the
 * backend filters what the user can see based on their permissions.
 */
export function useUserProjects(): UseUserProjectsResult {
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

  // Then fetch projects for each organization
  const projectQueries = useQueries({
    queries: organizations.map((org) => ({
      ...projectListOptions({ path: { org_slug: org.slug } }),
      staleTime: 5 * 60 * 1000,
      enabled: organizations.length > 0,
    })),
  });

  // Combine all projects with their org info, memoized to avoid new references
  const projects = useMemo(() => {
    const result: ProjectWithOrg[] = [];
    for (let i = 0; i < organizations.length; i++) {
      const org = organizations[i];
      const projectsData = projectQueries[i]?.data?.data ?? [];
      for (const project of projectsData) {
        result.push({
          ...project,
          org_slug: org.slug,
          org_name: org.name,
        });
      }
    }
    // Sort projects by name
    result.sort((a, b) => a.name.localeCompare(b.name));
    return result;
  }, [organizations, projectQueries]);

  const isLoading = orgsLoading || projectQueries.some((q) => q.isLoading);
  const projectError = projectQueries.find((q) => q.error)?.error;
  const error = orgsError ?? projectError ?? null;

  return {
    projects,
    organizations,
    isLoading,
    error: error as Error | null,
  };
}
