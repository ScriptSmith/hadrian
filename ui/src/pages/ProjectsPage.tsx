import { useState, useMemo } from "react";
import { Link } from "react-router-dom";
import { FolderOpen, Plus, Users, Calendar, UsersRound } from "lucide-react";
import { useQuery, useQueries } from "@tanstack/react-query";

import {
  organizationListOptions,
  projectListOptions,
  teamListOptions,
} from "@/api/generated/@tanstack/react-query.gen";
import type { Project, Team } from "@/api/generated/types.gen";
import { Button } from "@/components/Button/Button";
import { Badge } from "@/components/Badge/Badge";
import { Card, CardContent } from "@/components/Card/Card";
import { Input } from "@/components/Input/Input";
import { Skeleton } from "@/components/Skeleton/Skeleton";
import { QuickCreateProjectModal } from "@/components/QuickCreateProjectModal/QuickCreateProjectModal";
import { formatDateTime } from "@/utils/formatters";

interface ProjectWithTeam extends Project {
  org_slug: string;
  team_name?: string;
}

function ProjectCard({ project }: { project: ProjectWithTeam }) {
  return (
    <Link to={`/projects/${project.org_slug}/${project.slug}`} className="block">
      <Card className="h-full transition-colors hover:bg-muted/50">
        <CardContent className="p-4">
          <div className="flex items-start justify-between gap-2">
            <div className="flex items-center gap-2 min-w-0">
              <FolderOpen className="h-5 w-5 text-muted-foreground shrink-0" />
              <h2 className="font-medium truncate text-base">{project.name}</h2>
            </div>
            {project.team_name && (
              <Badge variant="secondary" className="shrink-0 text-xs">
                <UsersRound className="h-3 w-3 mr-1" />
                {project.team_name}
              </Badge>
            )}
          </div>

          <p className="mt-1 text-sm text-muted-foreground font-mono">{project.slug}</p>

          <div className="mt-3 flex items-center gap-4 text-xs text-muted-foreground">
            <span className="flex items-center gap-1">
              <Calendar className="h-3 w-3" />
              {formatDateTime(project.created_at)}
            </span>
          </div>
        </CardContent>
      </Card>
    </Link>
  );
}

function ProjectCardSkeleton() {
  return (
    <Card>
      <CardContent className="p-4">
        <div className="flex items-start justify-between gap-2">
          <div className="flex items-center gap-2">
            <Skeleton className="h-5 w-5 rounded" />
            <Skeleton className="h-5 w-32" />
          </div>
          <Skeleton className="h-5 w-16" />
        </div>
        <Skeleton className="mt-2 h-4 w-24" />
        <Skeleton className="mt-3 h-3 w-32" />
      </CardContent>
    </Card>
  );
}

export default function ProjectsPage() {
  const [search, setSearch] = useState("");
  const [isCreateModalOpen, setIsCreateModalOpen] = useState(false);

  // Fetch organizations
  const {
    data: orgsData,
    isLoading: orgsLoading,
    error: orgsError,
  } = useQuery(organizationListOptions());

  const organizations = useMemo(() => orgsData?.data ?? [], [orgsData?.data]);

  // Fetch projects and teams for each organization
  const projectQueries = useQueries({
    queries: organizations.map((org) => ({
      ...projectListOptions({ path: { org_slug: org.slug } }),
      staleTime: 5 * 60 * 1000,
      enabled: organizations.length > 0,
    })),
  });

  const teamQueries = useQueries({
    queries: organizations.map((org) => ({
      ...teamListOptions({ path: { org_slug: org.slug } }),
      staleTime: 5 * 60 * 1000,
      enabled: organizations.length > 0,
    })),
  });

  // Build team lookup map and combine projects with team names
  const projects = useMemo(() => {
    const teamMap = new Map<string, Team>();
    for (const query of teamQueries) {
      for (const team of query.data?.data ?? []) {
        teamMap.set(team.id, team);
      }
    }

    const result: ProjectWithTeam[] = [];
    for (let i = 0; i < organizations.length; i++) {
      const org = organizations[i];
      const projectsData = projectQueries[i]?.data?.data ?? [];
      for (const project of projectsData) {
        result.push({
          ...project,
          org_slug: org.slug,
          team_name: project.team_id ? teamMap.get(project.team_id)?.name : undefined,
        });
      }
    }
    result.sort((a, b) => a.name.localeCompare(b.name));
    return result;
  }, [organizations, projectQueries, teamQueries]);

  const isLoading =
    orgsLoading || projectQueries.some((q) => q.isLoading) || teamQueries.some((q) => q.isLoading);
  const error =
    orgsError ??
    projectQueries.find((q) => q.error)?.error ??
    teamQueries.find((q) => q.error)?.error;

  const filteredProjects = projects.filter(
    (p) =>
      p.name.toLowerCase().includes(search.toLowerCase()) ||
      p.slug.toLowerCase().includes(search.toLowerCase()) ||
      (p.team_name?.toLowerCase().includes(search.toLowerCase()) ?? false)
  );

  return (
    <div className="p-6 max-w-6xl mx-auto">
      {/* Header */}
      <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between mb-6">
        <div>
          <h1 className="text-2xl font-semibold">Projects</h1>
          <p className="text-sm text-muted-foreground mt-1">Manage your projects and teams</p>
        </div>
        <Button onClick={() => setIsCreateModalOpen(true)} disabled={organizations.length === 0}>
          <Plus className="h-4 w-4 mr-2" />
          New Project
        </Button>
      </div>

      {/* Search */}
      <div className="mb-6">
        <Input
          placeholder="Search projects..."
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          className="max-w-sm"
        />
      </div>

      {/* Error state */}
      {error && (
        <div className="rounded-md bg-destructive/10 px-4 py-3 text-sm text-destructive mb-6">
          Failed to load projects. Please try again.
        </div>
      )}

      {/* Loading state */}
      {isLoading && (
        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {Array.from({ length: 6 }).map((_, i) => (
            <ProjectCardSkeleton key={i} />
          ))}
        </div>
      )}

      {/* Empty state - no organizations */}
      {!isLoading && organizations.length === 0 && (
        <div className="text-center py-12">
          <Users className="h-12 w-12 text-muted-foreground mx-auto mb-4" />
          <h2 className="text-lg font-medium mb-2">No organizations</h2>
          <p className="text-sm text-muted-foreground max-w-md mx-auto">
            You need to be a member of an organization to create projects. Contact your
            administrator to get access.
          </p>
        </div>
      )}

      {/* Empty state - no projects */}
      {!isLoading && organizations.length > 0 && projects.length === 0 && (
        <div className="text-center py-12">
          <FolderOpen className="h-12 w-12 text-muted-foreground mx-auto mb-4" />
          <h2 className="text-lg font-medium mb-2">No projects yet</h2>
          <p className="text-sm text-muted-foreground max-w-md mx-auto mb-4">
            Create your first project to organize your work and manage API keys.
          </p>
          <Button onClick={() => setIsCreateModalOpen(true)}>
            <Plus className="h-4 w-4 mr-2" />
            Create Project
          </Button>
        </div>
      )}

      {/* Empty state - no search results */}
      {!isLoading && projects.length > 0 && filteredProjects.length === 0 && (
        <div className="text-center py-12">
          <FolderOpen className="h-12 w-12 text-muted-foreground mx-auto mb-4" />
          <h2 className="text-lg font-medium mb-2">No matching projects</h2>
          <p className="text-sm text-muted-foreground">
            Try adjusting your search terms or{" "}
            <button onClick={() => setSearch("")} className="text-primary hover:underline">
              clear the search
            </button>
          </p>
        </div>
      )}

      {/* Projects grid */}
      {!isLoading && filteredProjects.length > 0 && (
        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {filteredProjects.map((project) => (
            <ProjectCard key={project.id} project={project} />
          ))}
        </div>
      )}

      {/* Create project modal */}
      <QuickCreateProjectModal
        open={isCreateModalOpen}
        onClose={() => setIsCreateModalOpen(false)}
      />
    </div>
  );
}
