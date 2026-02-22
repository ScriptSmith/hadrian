import { useQuery } from "@tanstack/react-query";
import { useState } from "react";

import {
  organizationListOptions,
  teamListOptions,
  projectListOptions,
  userListOptions,
  apiKeyListByOrgOptions,
} from "@/api/generated/@tanstack/react-query.gen";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/Card/Card";
import { CodeBadge } from "@/components/CodeBadge/CodeBadge";
import {
  PageHeader,
  TeamSelect,
  ProjectSelect,
  UserSelect,
  ApiKeyStatusBadge,
} from "@/components/Admin";
import { formatCurrency } from "@/utils/formatters";
import UsageDashboard, { type UsageScope } from "@/components/UsageDashboard/UsageDashboard";

export default function UsagePage() {
  const [selectedOrg, setSelectedOrg] = useState<string | null>(null);
  const [selectedTeam, setSelectedTeam] = useState<string | null>(null);
  const [selectedProject, setSelectedProject] = useState<string | null>(null);
  const [selectedUser, setSelectedUser] = useState<string | null>(null);
  const [selectedApiKey, setSelectedApiKey] = useState<string | null>(null);

  const { data: organizations } = useQuery(organizationListOptions());

  // effectiveOrg is null when "All organizations" is selected (global view)
  const effectiveOrg = selectedOrg;

  const { data: teams } = useQuery({
    ...teamListOptions({ path: { org_slug: effectiveOrg || "" } }),
    enabled: !!effectiveOrg,
  });

  const { data: projects } = useQuery({
    ...projectListOptions({ path: { org_slug: effectiveOrg || "" } }),
    enabled: !!effectiveOrg,
  });

  const { data: users } = useQuery({
    ...userListOptions(),
    enabled: !!effectiveOrg,
  });

  const { data: apiKeys } = useQuery({
    ...apiKeyListByOrgOptions({ path: { org_slug: effectiveOrg || "" } }),
    enabled: !!effectiveOrg,
  });

  const selectedKeyInfo = apiKeys?.data?.find((k) => k.id === selectedApiKey);

  // Resolve the selected team slug from ID
  const selectedTeamSlug = selectedTeam
    ? teams?.data?.find((t) => t.id === selectedTeam)?.slug
    : null;

  // Scope resolution: first match wins (most specific -> least specific)
  // When no org is selected, use global scope
  const scope: UsageScope = selectedApiKey
    ? { type: "apiKey", keyId: selectedApiKey }
    : selectedUser
      ? { type: "user", userId: selectedUser }
      : selectedProject && effectiveOrg
        ? { type: "project", orgSlug: effectiveOrg, projectSlug: selectedProject }
        : selectedTeamSlug && effectiveOrg
          ? { type: "team", orgSlug: effectiveOrg, teamSlug: selectedTeamSlug }
          : effectiveOrg
            ? { type: "organization", slug: effectiveOrg }
            : { type: "global" };

  return (
    <div className="p-6">
      <PageHeader title="Usage Analytics" description="View usage statistics and analytics" />

      {/* Filters */}
      <div className="mb-6 flex flex-wrap items-end gap-4">
        {organizations?.data && (
          <div>
            <label htmlFor="usage-org" className="mb-1 block text-sm font-medium">
              Organization
            </label>
            <select
              id="usage-org"
              value={selectedOrg || ""}
              onChange={(e) => {
                const slug = e.target.value || null;
                setSelectedOrg(slug);
                setSelectedTeam(null);
                setSelectedProject(null);
                setSelectedUser(null);
                setSelectedApiKey(null);
              }}
              className="rounded-md border border-input bg-background px-3 py-2 text-sm"
            >
              <option value="">All organizations</option>
              {organizations.data.map((org) => (
                <option key={org.slug} value={org.slug}>
                  {org.name}
                </option>
              ))}
            </select>
          </div>
        )}

        {effectiveOrg && teams?.data && teams.data.length > 0 && (
          <TeamSelect
            teams={teams.data}
            value={selectedTeam}
            onChange={(teamId) => {
              setSelectedTeam(teamId);
              setSelectedProject(null);
              setSelectedUser(null);
              setSelectedApiKey(null);
            }}
            label="Team (optional)"
            nonePlaceholder="All teams"
          />
        )}

        {effectiveOrg && projects?.data && projects.data.length > 0 && (
          <ProjectSelect
            projects={projects.data}
            value={selectedProject}
            onChange={(slug) => {
              setSelectedProject(slug);
              setSelectedUser(null);
              setSelectedApiKey(null);
            }}
            label="Project (optional)"
          />
        )}

        {effectiveOrg && users?.data && users.data.length > 0 && (
          <UserSelect
            users={users.data}
            value={selectedUser}
            onChange={(userId) => {
              setSelectedUser(userId);
              setSelectedApiKey(null);
            }}
            label="User (optional)"
          />
        )}

        {effectiveOrg && apiKeys?.data && apiKeys.data.length > 0 && (
          <div>
            <label htmlFor="usage-api-key" className="mb-1 block text-sm font-medium">
              API Key (optional)
            </label>
            <select
              id="usage-api-key"
              value={selectedApiKey || ""}
              onChange={(e) => setSelectedApiKey(e.target.value || null)}
              className="rounded-md border border-input bg-background px-3 py-2 text-sm"
            >
              <option value="">All keys</option>
              {apiKeys.data.map((key) => (
                <option key={key.id} value={key.id}>
                  {key.name} ({key.key_prefix}...)
                </option>
              ))}
            </select>
          </div>
        )}
      </div>

      <UsageDashboard scope={scope} />

      {/* API Key Details (when drilled into a specific key) */}
      {selectedKeyInfo && (
        <Card className="mt-6">
          <CardHeader>
            <CardTitle>API Key Details</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
              <div>
                <div className="text-sm text-muted-foreground">Name</div>
                <div className="font-medium">{selectedKeyInfo.name}</div>
              </div>
              <div>
                <div className="text-sm text-muted-foreground">Key Prefix</div>
                <CodeBadge>{selectedKeyInfo.key_prefix}...</CodeBadge>
              </div>
              <div>
                <div className="text-sm text-muted-foreground">Budget</div>
                <div className="font-medium">
                  {selectedKeyInfo.budget_limit_cents ? (
                    <>
                      {formatCurrency(selectedKeyInfo.budget_limit_cents / 100)}
                      {selectedKeyInfo.budget_period && (
                        <span className="text-muted-foreground">
                          /{selectedKeyInfo.budget_period}
                        </span>
                      )}
                    </>
                  ) : (
                    <span className="text-muted-foreground">No limit</span>
                  )}
                </div>
              </div>
              <div>
                <div className="text-sm text-muted-foreground">Status</div>
                <div>
                  <ApiKeyStatusBadge
                    revokedAt={selectedKeyInfo.revoked_at}
                    expiresAt={selectedKeyInfo.expires_at}
                  />
                </div>
              </div>
            </div>
          </CardContent>
        </Card>
      )}
    </div>
  );
}
