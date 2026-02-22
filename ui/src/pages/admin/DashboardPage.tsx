import { useQuery } from "@tanstack/react-query";
import { Building2, FolderOpen, Users, Key, BarChart3 } from "lucide-react";
import { Link } from "react-router-dom";

import {
  organizationListOptions,
  projectListOptions,
  userListOptions,
} from "@/api/generated/@tanstack/react-query.gen";
import { Button } from "@/components/Button/Button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/Card/Card";
import { Skeleton } from "@/components/Skeleton/Skeleton";

export default function DashboardPage() {
  const { data: organizations, isLoading: orgsLoading } = useQuery(organizationListOptions());
  const { data: users, isLoading: usersLoading } = useQuery(userListOptions());

  // Get first org to fetch projects (we'll show aggregate count)
  const firstOrg = organizations?.data?.[0]?.slug;
  const { data: projects, isLoading: projectsLoading } = useQuery({
    ...projectListOptions({ path: { org_slug: firstOrg || "" } }),
    enabled: !!firstOrg,
  });

  const stats = [
    {
      title: "Organizations",
      value: orgsLoading ? null : (organizations?.data?.length ?? 0),
      icon: Building2,
      href: "/admin/organizations",
    },
    {
      title: "Projects",
      value: projectsLoading || !firstOrg ? null : (projects?.data?.length ?? 0),
      icon: FolderOpen,
      href: "/admin/projects",
    },
    {
      title: "Users",
      value: usersLoading ? null : (users?.data?.length ?? 0),
      icon: Users,
      href: "/admin/users",
    },
    {
      title: "API Keys",
      value: "—",
      icon: Key,
      href: "/admin/api-keys",
    },
  ];

  return (
    <div className="p-6">
      <div className="mb-6">
        <h1 className="text-2xl font-semibold">Dashboard</h1>
        <p className="text-muted-foreground">Overview of your Hadrian Gateway instance</p>
      </div>

      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
        {stats.map((stat) => (
          <Link key={stat.title} to={stat.href}>
            <Card className="transition-shadow hover:shadow-md">
              <CardHeader className="flex flex-row items-center justify-between pb-2">
                <CardTitle className="text-sm font-medium text-muted-foreground">
                  {stat.title}
                </CardTitle>
                <stat.icon className="h-4 w-4 text-muted-foreground" />
              </CardHeader>
              <CardContent>
                {stat.value === null ? (
                  <Skeleton className="h-8 w-16" />
                ) : (
                  <div className="text-2xl font-bold">{stat.value}</div>
                )}
              </CardContent>
            </Card>
          </Link>
        ))}
      </div>

      <div className="mt-8 grid gap-6 lg:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle>Quick Actions</CardTitle>
          </CardHeader>
          <CardContent className="space-y-4">
            <Link to="/admin/organizations">
              <Button variant="outline" className="w-full justify-start">
                <Building2 className="mr-2 h-4 w-4" />
                Create Organization
              </Button>
            </Link>
            <Link to="/admin/projects">
              <Button variant="outline" className="w-full justify-start">
                <FolderOpen className="mr-2 h-4 w-4" />
                Create Project
              </Button>
            </Link>
            <Link to="/admin/api-keys">
              <Button variant="outline" className="w-full justify-start">
                <Key className="mr-2 h-4 w-4" />
                Generate API Key
              </Button>
            </Link>
            <Link to="/chat">
              <Button variant="outline" className="w-full justify-start">
                <BarChart3 className="mr-2 h-4 w-4" />
                Start Chatting
              </Button>
            </Link>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Getting Started</CardTitle>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="space-y-2">
              <h3 className="font-medium">1. Create an Organization</h3>
              <p className="text-sm text-muted-foreground">
                Organizations are the top-level entity for multi-tenancy.
              </p>
            </div>
            <div className="space-y-2">
              <h3 className="font-medium">2. Add a Project</h3>
              <p className="text-sm text-muted-foreground">
                Projects belong to organizations and help separate workloads.
              </p>
            </div>
            <div className="space-y-2">
              <h3 className="font-medium">3. Generate an API Key</h3>
              <p className="text-sm text-muted-foreground">
                API keys authenticate requests to the Public API.
              </p>
            </div>
            <div className="space-y-2">
              <h3 className="font-medium">4. Start Using the API</h3>
              <p className="text-sm text-muted-foreground">
                Use the chat interface or integrate with your applications.
              </p>
            </div>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
