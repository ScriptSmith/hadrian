import { useState, useMemo } from "react";
import { Link } from "react-router-dom";
import { UsersRound, Users, Calendar, FolderOpen } from "lucide-react";
import { useQuery, useQueries } from "@tanstack/react-query";

import {
  organizationListOptions,
  teamListOptions,
  projectListOptions,
} from "@/api/generated/@tanstack/react-query.gen";
import type { Team } from "@/api/generated/types.gen";
import { Badge } from "@/components/Badge/Badge";
import { Card, CardContent } from "@/components/Card/Card";
import { Input } from "@/components/Input/Input";
import { Skeleton } from "@/components/Skeleton/Skeleton";
import { formatDateTime } from "@/utils/formatters";

interface TeamWithProjects extends Team {
  org_slug: string;
  project_count: number;
}

function TeamCard({ team }: { team: TeamWithProjects }) {
  return (
    <Link to={`/admin/organizations/${team.org_slug}/teams/${team.slug}`} className="block">
      <Card className="h-full transition-colors hover:bg-muted/50">
        <CardContent className="p-4">
          <div className="flex items-start justify-between gap-2">
            <div className="flex items-center gap-2 min-w-0">
              <UsersRound className="h-5 w-5 text-muted-foreground shrink-0" />
              <h3 className="font-medium truncate">{team.name}</h3>
            </div>
            {team.project_count > 0 && (
              <Badge variant="secondary" className="shrink-0 text-xs">
                <FolderOpen className="h-3 w-3 mr-1" />
                {team.project_count} project{team.project_count !== 1 ? "s" : ""}
              </Badge>
            )}
          </div>

          <p className="mt-1 text-sm text-muted-foreground font-mono">{team.slug}</p>

          <div className="mt-3 flex items-center gap-4 text-xs text-muted-foreground">
            <span className="flex items-center gap-1">
              <Calendar className="h-3 w-3" />
              {formatDateTime(team.created_at)}
            </span>
          </div>
        </CardContent>
      </Card>
    </Link>
  );
}

function TeamCardSkeleton() {
  return (
    <Card>
      <CardContent className="p-4">
        <div className="flex items-start justify-between gap-2">
          <div className="flex items-center gap-2">
            <Skeleton className="h-5 w-5 rounded" />
            <Skeleton className="h-5 w-32" />
          </div>
          <Skeleton className="h-5 w-20" />
        </div>
        <Skeleton className="mt-2 h-4 w-24" />
        <Skeleton className="mt-3 h-3 w-32" />
      </CardContent>
    </Card>
  );
}

export default function TeamsPage() {
  const [search, setSearch] = useState("");

  // Fetch organizations
  const {
    data: orgsData,
    isLoading: orgsLoading,
    error: orgsError,
  } = useQuery(organizationListOptions());

  const organizations = useMemo(() => orgsData?.data ?? [], [orgsData?.data]);

  // Fetch teams and projects for each organization
  const teamQueries = useQueries({
    queries: organizations.map((org) => ({
      ...teamListOptions({ path: { org_slug: org.slug } }),
      staleTime: 5 * 60 * 1000,
      enabled: organizations.length > 0,
    })),
  });

  const projectQueries = useQueries({
    queries: organizations.map((org) => ({
      ...projectListOptions({ path: { org_slug: org.slug } }),
      staleTime: 5 * 60 * 1000,
      enabled: organizations.length > 0,
    })),
  });

  // Count projects per team and build team list
  const teams = useMemo(() => {
    // Count projects per team
    const projectCounts = new Map<string, number>();
    for (const query of projectQueries) {
      for (const project of query.data?.data ?? []) {
        if (project.team_id) {
          projectCounts.set(project.team_id, (projectCounts.get(project.team_id) ?? 0) + 1);
        }
      }
    }

    const result: TeamWithProjects[] = [];
    for (let i = 0; i < organizations.length; i++) {
      const org = organizations[i];
      const teamsData = teamQueries[i]?.data?.data ?? [];
      for (const team of teamsData) {
        result.push({
          ...team,
          org_slug: org.slug,
          project_count: projectCounts.get(team.id) ?? 0,
        });
      }
    }
    result.sort((a, b) => a.name.localeCompare(b.name));
    return result;
  }, [organizations, teamQueries, projectQueries]);

  const isLoading =
    orgsLoading || teamQueries.some((q) => q.isLoading) || projectQueries.some((q) => q.isLoading);
  const error =
    orgsError ??
    teamQueries.find((q) => q.error)?.error ??
    projectQueries.find((q) => q.error)?.error;

  const filteredTeams = teams.filter(
    (t) =>
      t.name.toLowerCase().includes(search.toLowerCase()) ||
      t.slug.toLowerCase().includes(search.toLowerCase())
  );

  return (
    <div className="p-6 max-w-6xl mx-auto">
      {/* Header */}
      <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between mb-6">
        <div>
          <h1 className="text-2xl font-semibold">Teams</h1>
          <p className="text-sm text-muted-foreground mt-1">View and manage your teams</p>
        </div>
      </div>

      {/* Search */}
      <div className="mb-6">
        <Input
          placeholder="Search teams..."
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          className="max-w-sm"
        />
      </div>

      {/* Error state */}
      {error && (
        <div className="rounded-md bg-destructive/10 px-4 py-3 text-sm text-destructive mb-6">
          Failed to load teams. Please try again.
        </div>
      )}

      {/* Loading state */}
      {isLoading && (
        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {Array.from({ length: 6 }).map((_, i) => (
            <TeamCardSkeleton key={i} />
          ))}
        </div>
      )}

      {/* Empty state - no organizations */}
      {!isLoading && organizations.length === 0 && (
        <div className="text-center py-12">
          <Users className="h-12 w-12 text-muted-foreground mx-auto mb-4" />
          <h2 className="text-lg font-medium mb-2">No organization</h2>
          <p className="text-sm text-muted-foreground max-w-md mx-auto">
            You need to be a member of an organization to view teams. Contact your administrator to
            get access.
          </p>
        </div>
      )}

      {/* Empty state - no teams */}
      {!isLoading && organizations.length > 0 && teams.length === 0 && (
        <div className="text-center py-12">
          <UsersRound className="h-12 w-12 text-muted-foreground mx-auto mb-4" />
          <h2 className="text-lg font-medium mb-2">No teams yet</h2>
          <p className="text-sm text-muted-foreground max-w-md mx-auto">
            Teams help organize projects and members. Contact your administrator to create teams.
          </p>
        </div>
      )}

      {/* Empty state - no search results */}
      {!isLoading && teams.length > 0 && filteredTeams.length === 0 && (
        <div className="text-center py-12">
          <UsersRound className="h-12 w-12 text-muted-foreground mx-auto mb-4" />
          <h2 className="text-lg font-medium mb-2">No matching teams</h2>
          <p className="text-sm text-muted-foreground">
            Try adjusting your search terms or{" "}
            <button onClick={() => setSearch("")} className="text-primary hover:underline">
              clear the search
            </button>
          </p>
        </div>
      )}

      {/* Teams grid */}
      {!isLoading && filteredTeams.length > 0 && (
        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {filteredTeams.map((team) => (
            <TeamCard key={team.id} team={team} />
          ))}
        </div>
      )}
    </div>
  );
}
