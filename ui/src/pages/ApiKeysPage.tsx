import { useState, useMemo, useCallback } from "react";
import { Link, useNavigate } from "react-router-dom";
import {
  Key,
  Plus,
  Calendar,
  Clock,
  DollarSign,
  Shield,
  Network,
  Cpu,
  RotateCw,
  Lock,
  ChevronDown,
  ExternalLink,
} from "lucide-react";
import { useQuery, useQueries, useMutation, useQueryClient } from "@tanstack/react-query";

import {
  organizationListOptions,
  apiKeyListByOrgOptions,
  meApiKeysListOptions,
  meApiKeysCreateMutation,
  meApiKeysRevokeMutation,
  meApiKeysRotateMutation,
} from "@/api/generated/@tanstack/react-query.gen";
import type { ApiKey, CreateSelfServiceApiKey } from "@/api/generated/types.gen";
import { Button } from "@/components/Button/Button";
import { Badge } from "@/components/Badge/Badge";
import { Card, CardContent } from "@/components/Card/Card";
import { Input } from "@/components/Input/Input";
import { Skeleton } from "@/components/Skeleton/Skeleton";
import { CodeBadge } from "@/components/CodeBadge/CodeBadge";
import {
  ApiKeyStatusBadge,
  OwnerBadge,
  SelfServiceApiKeyFormModal,
  ApiKeyCreatedModal,
} from "@/components/Admin";
import {
  Dropdown,
  DropdownContent,
  DropdownItem,
  DropdownTrigger,
} from "@/components/Dropdown/Dropdown";
import { useToast } from "@/components/Toast/Toast";
import { useConfirm } from "@/components/ConfirmDialog/ConfirmDialog";
import { formatDateTime, formatCurrency, formatRelativeTime } from "@/utils/formatters";
import { MoreHorizontal, Trash2 } from "lucide-react";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/Tooltip/Tooltip";
import { cn } from "@/utils/cn";

function ApiKeyCard({
  apiKey,
  readOnly,
  onRevoke,
  onRotate,
}: {
  apiKey: ApiKey;
  readOnly?: boolean;
  onRevoke?: (key: ApiKey) => void;
  onRotate?: (key: ApiKey) => void;
}) {
  const navigate = useNavigate();
  const isRevoked = !!apiKey.revoked_at;
  const isRotating = !!apiKey.rotation_grace_until;
  const graceEndTime = apiKey.rotation_grace_until ? new Date(apiKey.rotation_grace_until) : null;
  const isGraceExpired = graceEndTime && graceEndTime < new Date();

  const hasScopes = apiKey.scopes && apiKey.scopes.length > 0;
  const hasModelRestrictions = apiKey.allowed_models && apiKey.allowed_models.length > 0;
  const hasIpRestrictions = apiKey.ip_allowlist && apiKey.ip_allowlist.length > 0;
  const hasRateLimits = apiKey.rate_limit_rpm || apiKey.rate_limit_tpm;

  return (
    <Card className="h-full">
      <CardContent className="p-4">
        <div className="flex items-start justify-between gap-2">
          <div className="flex items-center gap-2 min-w-0">
            <Key className="h-5 w-5 text-muted-foreground shrink-0" />
            {readOnly ? (
              <h3 className="font-medium truncate">{apiKey.name}</h3>
            ) : (
              <Link
                to={`/api-keys/${apiKey.id}`}
                className="font-medium truncate hover:underline hover:text-primary"
              >
                {apiKey.name}
              </Link>
            )}
          </div>
          <div className="flex items-center gap-2 shrink-0">
            <ApiKeyStatusBadge revokedAt={apiKey.revoked_at} expiresAt={apiKey.expires_at} />
            {isRotating && !isGraceExpired && (
              <Tooltip>
                <TooltipTrigger>
                  <Badge variant="warning" className="gap-1">
                    <RotateCw className="h-3 w-3" />
                    Rotating
                  </Badge>
                </TooltipTrigger>
                <TooltipContent>
                  Grace period ends {formatRelativeTime(graceEndTime!)}
                </TooltipContent>
              </Tooltip>
            )}
            {!readOnly && (
              <Dropdown>
                <DropdownTrigger aria-label="Actions" variant="ghost" className="h-8 w-8 p-0">
                  <MoreHorizontal className="h-4.5 w-4.5" />
                </DropdownTrigger>
                <DropdownContent align="end">
                  <DropdownItem onClick={() => navigate(`/api-keys/${apiKey.id}`)}>
                    <ExternalLink className="mr-2 h-4 w-4" />
                    View Details
                  </DropdownItem>
                  <DropdownItem
                    disabled={isRevoked || isRotating}
                    onClick={() => onRotate?.(apiKey)}
                  >
                    <RotateCw className="mr-2 h-4 w-4" />
                    {isRotating ? "Already Rotating" : "Rotate"}
                  </DropdownItem>
                  <DropdownItem
                    className="text-destructive"
                    disabled={isRevoked}
                    onClick={() => onRevoke?.(apiKey)}
                  >
                    <Trash2 className="mr-2 h-4 w-4" />
                    {isRevoked ? "Already Revoked" : "Revoke"}
                  </DropdownItem>
                </DropdownContent>
              </Dropdown>
            )}
          </div>
        </div>

        <div className="mt-2 flex items-center gap-2 flex-wrap">
          <CodeBadge className="text-xs">{apiKey.key_prefix}...</CodeBadge>
          <OwnerBadge owner={apiKey.owner} />
        </div>

        {/* Advanced settings badges */}
        {(hasScopes || hasModelRestrictions || hasIpRestrictions || hasRateLimits) && (
          <div className="mt-2 flex flex-wrap items-center gap-1.5">
            {hasScopes && (
              <Tooltip>
                <TooltipTrigger>
                  <Badge variant="outline" className="gap-1 text-xs">
                    <Shield className="h-3 w-3" />
                    {apiKey.scopes!.length} scope{apiKey.scopes!.length !== 1 ? "s" : ""}
                  </Badge>
                </TooltipTrigger>
                <TooltipContent>
                  <div className="font-medium mb-1">Permission Scopes</div>
                  <div className="text-xs">{apiKey.scopes!.join(", ")}</div>
                </TooltipContent>
              </Tooltip>
            )}
            {hasModelRestrictions && (
              <Tooltip>
                <TooltipTrigger>
                  <Badge variant="outline" className="gap-1 text-xs">
                    <Cpu className="h-3 w-3" />
                    {apiKey.allowed_models!.length} model
                    {apiKey.allowed_models!.length !== 1 ? "s" : ""}
                  </Badge>
                </TooltipTrigger>
                <TooltipContent>
                  <div className="font-medium mb-1">Model Restrictions</div>
                  <div className="text-xs">{apiKey.allowed_models!.join(", ")}</div>
                </TooltipContent>
              </Tooltip>
            )}
            {hasIpRestrictions && (
              <Tooltip>
                <TooltipTrigger>
                  <Badge variant="outline" className="gap-1 text-xs">
                    <Lock className="h-3 w-3" />
                    IP restricted
                  </Badge>
                </TooltipTrigger>
                <TooltipContent>
                  <div className="font-medium mb-1">IP Allowlist</div>
                  <div className="text-xs font-mono">{apiKey.ip_allowlist!.join(", ")}</div>
                </TooltipContent>
              </Tooltip>
            )}
            {hasRateLimits && (
              <Tooltip>
                <TooltipTrigger>
                  <Badge variant="outline" className="gap-1 text-xs">
                    <Network className="h-3 w-3" />
                    Rate limited
                  </Badge>
                </TooltipTrigger>
                <TooltipContent>
                  <div className="font-medium mb-1">Custom Rate Limits</div>
                  <div className="text-xs">
                    {apiKey.rate_limit_rpm && <div>RPM: {apiKey.rate_limit_rpm}</div>}
                    {apiKey.rate_limit_tpm && <div>TPM: {apiKey.rate_limit_tpm}</div>}
                  </div>
                </TooltipContent>
              </Tooltip>
            )}
          </div>
        )}

        <div className="mt-3 flex flex-wrap items-center gap-3 text-xs text-muted-foreground">
          {apiKey.budget_limit_cents && (
            <span className="flex items-center gap-1">
              <DollarSign className="h-3 w-3" />
              {formatCurrency(apiKey.budget_limit_cents / 100)}
              {apiKey.budget_period && (
                <span className="text-muted-foreground">/{apiKey.budget_period}</span>
              )}
            </span>
          )}
          <span className="flex items-center gap-1">
            <Clock className="h-3 w-3" />
            {apiKey.last_used_at ? `Used ${formatDateTime(apiKey.last_used_at)}` : "Never used"}
          </span>
          <span className="flex items-center gap-1">
            <Calendar className="h-3 w-3" />
            {formatDateTime(apiKey.created_at)}
          </span>
        </div>
      </CardContent>
    </Card>
  );
}

function ApiKeyCardSkeleton() {
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
        <div className="mt-2 flex items-center gap-2">
          <Skeleton className="h-5 w-24" />
          <Skeleton className="h-5 w-20" />
        </div>
        <div className="mt-3 flex gap-3">
          <Skeleton className="h-3 w-20" />
          <Skeleton className="h-3 w-24" />
        </div>
      </CardContent>
    </Card>
  );
}

function KeySection({
  title,
  keys,
  readOnly,
  onRevoke,
  onRotate,
  defaultOpen = true,
}: {
  title: string;
  keys: ApiKey[];
  readOnly?: boolean;
  onRevoke?: (key: ApiKey) => void;
  onRotate?: (key: ApiKey) => void;
  defaultOpen?: boolean;
}) {
  const [isOpen, setIsOpen] = useState(defaultOpen);

  if (keys.length === 0) return null;

  return (
    <div>
      <button
        type="button"
        onClick={() => setIsOpen(!isOpen)}
        className="flex w-full items-center gap-2 mb-3 text-sm font-medium text-muted-foreground hover:text-foreground"
      >
        <ChevronDown className={cn("h-4 w-4 transition-transform", !isOpen && "-rotate-90")} />
        {title}
        <Badge variant="secondary" className="text-xs">
          {keys.length}
        </Badge>
      </button>

      {isOpen && (
        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3 mb-6">
          {keys.map((key) => (
            <ApiKeyCard
              key={key.id}
              apiKey={key}
              readOnly={readOnly}
              onRevoke={onRevoke}
              onRotate={onRotate}
            />
          ))}
        </div>
      )}
    </div>
  );
}

export default function ApiKeysPage() {
  const [search, setSearch] = useState("");
  const [isCreateModalOpen, setIsCreateModalOpen] = useState(false);
  const [createdKey, setCreatedKey] = useState<string | null>(null);
  const { toast } = useToast();
  const confirm = useConfirm();
  const queryClient = useQueryClient();

  // Fetch current user's keys
  const {
    data: myKeysData,
    isLoading: myKeysLoading,
    error: myKeysError,
  } = useQuery(meApiKeysListOptions({ query: { limit: 100 } }));

  const myKeys = useMemo(() => myKeysData?.data ?? [], [myKeysData?.data]);
  const myKeyIds = useMemo(() => new Set(myKeys.map((k) => k.id)), [myKeys]);

  // Fetch organizations for org-level keys
  const {
    data: orgsData,
    isLoading: orgsLoading,
    error: orgsError,
  } = useQuery(organizationListOptions());

  const organizations = useMemo(() => orgsData?.data ?? [], [orgsData?.data]);

  // Fetch API keys for each organization
  const apiKeyQueries = useQueries({
    queries: organizations.map((org) => ({
      ...apiKeyListByOrgOptions({
        path: { org_slug: org.slug },
        query: { limit: 100 },
      }),
      staleTime: 5 * 60 * 1000,
      enabled: organizations.length > 0,
    })),
  });

  // Combine all org keys, excluding user-owned ones (those come from /me/api-keys)
  const orgApiKeys = useMemo(() => {
    const result: ApiKey[] = [];
    for (let i = 0; i < organizations.length; i++) {
      const keysData = apiKeyQueries[i]?.data?.data ?? [];
      for (const key of keysData) {
        // Skip user-owned keys (shown in "My Keys" section)
        if (myKeyIds.has(key.id)) continue;
        result.push(key);
      }
    }
    result.sort((a, b) => new Date(b.created_at).getTime() - new Date(a.created_at).getTime());
    return result;
  }, [organizations, apiKeyQueries, myKeyIds]);

  // Group org keys by owner type
  const { orgKeys, teamKeys, projectKeys, serviceAccountKeys } = useMemo(() => {
    const org: ApiKey[] = [];
    const team: ApiKey[] = [];
    const project: ApiKey[] = [];
    const sa: ApiKey[] = [];
    for (const key of orgApiKeys) {
      switch (key.owner.type) {
        case "organization":
          org.push(key);
          break;
        case "team":
          team.push(key);
          break;
        case "project":
          project.push(key);
          break;
        case "service_account":
          sa.push(key);
          break;
      }
    }
    return { orgKeys: org, teamKeys: team, projectKeys: project, serviceAccountKeys: sa };
  }, [orgApiKeys]);

  const isLoading = myKeysLoading || orgsLoading || apiKeyQueries.some((q) => q.isLoading);
  const error = myKeysError ?? orgsError ?? apiKeyQueries.find((q) => q.error)?.error;

  // Filter across all keys
  const filterKeys = useCallback(
    (keys: ApiKey[]) =>
      search
        ? keys.filter(
            (key) =>
              key.name.toLowerCase().includes(search.toLowerCase()) ||
              key.key_prefix.toLowerCase().includes(search.toLowerCase())
          )
        : keys,
    [search]
  );

  const filteredMyKeys = useMemo(() => filterKeys(myKeys), [filterKeys, myKeys]);
  const filteredOrgKeys = useMemo(() => filterKeys(orgKeys), [filterKeys, orgKeys]);
  const filteredTeamKeys = useMemo(() => filterKeys(teamKeys), [filterKeys, teamKeys]);
  const filteredProjectKeys = useMemo(() => filterKeys(projectKeys), [filterKeys, projectKeys]);
  const filteredSaKeys = useMemo(
    () => filterKeys(serviceAccountKeys),
    [filterKeys, serviceAccountKeys]
  );

  const totalKeys = myKeys.length + orgApiKeys.length;
  const totalFiltered =
    filteredMyKeys.length +
    filteredOrgKeys.length +
    filteredTeamKeys.length +
    filteredProjectKeys.length +
    filteredSaKeys.length;
  const activeCount = [...myKeys, ...orgApiKeys].filter((k) => !k.revoked_at).length;
  const revokedCount = [...myKeys, ...orgApiKeys].filter((k) => !!k.revoked_at).length;

  const createMutation = useMutation({
    ...meApiKeysCreateMutation(),
    onSuccess: (data) => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "meApiKeysList" }] });
      queryClient.invalidateQueries({ queryKey: [{ _id: "apiKeyListByOrg" }] });
      setIsCreateModalOpen(false);
      if (data && "key" in data) {
        setCreatedKey(data.key);
      }
      toast({ title: "API key created", type: "success" });
    },
    onError: (error) => {
      toast({
        title: "Failed to create API key",
        description: String(error),
        type: "error",
      });
    },
  });

  const revokeMutation = useMutation({
    ...meApiKeysRevokeMutation(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "meApiKeysList" }] });
      queryClient.invalidateQueries({ queryKey: [{ _id: "apiKeyListByOrg" }] });
      toast({ title: "API key revoked", type: "success" });
    },
    onError: (error) => {
      toast({
        title: "Failed to revoke API key",
        description: String(error),
        type: "error",
      });
    },
  });

  const rotateMutation = useMutation({
    ...meApiKeysRotateMutation(),
    onSuccess: (data) => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "meApiKeysList" }] });
      queryClient.invalidateQueries({ queryKey: [{ _id: "apiKeyListByOrg" }] });
      if (data && "key" in data) {
        setCreatedKey(data.key);
      }
      toast({ title: "API key rotated", type: "success" });
    },
    onError: (error) => {
      toast({
        title: "Failed to rotate API key",
        description: String(error),
        type: "error",
      });
    },
  });

  const handleCreateSubmit = useCallback(
    (data: CreateSelfServiceApiKey) => {
      createMutation.mutate({ body: data });
    },
    [createMutation]
  );

  const handleRevoke = useCallback(
    async (key: ApiKey) => {
      const confirmed = await confirm({
        title: "Revoke API Key",
        message: `Are you sure you want to revoke "${key.name}"? This action cannot be undone and the key will no longer work.`,
        confirmLabel: "Revoke",
        variant: "destructive",
      });
      if (confirmed) {
        revokeMutation.mutate({ path: { key_id: key.id } });
      }
    },
    [confirm, revokeMutation]
  );

  const handleRotate = useCallback(
    async (key: ApiKey) => {
      const confirmed = await confirm({
        title: "Rotate API Key",
        message: `This will create a new API key and mark the old one "${key.name}" for expiration after a 24-hour grace period. During this time, both keys will work.`,
        confirmLabel: "Rotate",
      });
      if (confirmed) {
        rotateMutation.mutate({
          path: { key_id: key.id },
          body: { grace_period_seconds: 86400 },
        });
      }
    },
    [confirm, rotateMutation]
  );

  return (
    <div className="p-6 max-w-6xl mx-auto">
      {/* Header */}
      <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between mb-6">
        <div>
          <h1 className="text-2xl font-semibold">API Keys</h1>
          <p className="text-sm text-muted-foreground mt-1">
            Manage your API keys for programmatic access
          </p>
        </div>
        <Button onClick={() => setIsCreateModalOpen(true)}>
          <Plus className="h-4 w-4 mr-2" />
          New API Key
        </Button>
      </div>

      {/* Stats */}
      {!isLoading && totalKeys > 0 && (
        <div className="flex items-center gap-4 mb-6">
          <Badge variant="secondary">{activeCount} active</Badge>
          {revokedCount > 0 && <Badge variant="outline">{revokedCount} revoked</Badge>}
        </div>
      )}

      {/* Search */}
      {!isLoading && totalKeys > 0 && (
        <div className="mb-6">
          <Input
            placeholder="Search API keys..."
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            className="max-w-sm"
          />
        </div>
      )}

      {/* Error state */}
      {error && (
        <div className="rounded-md bg-destructive/10 px-4 py-3 text-sm text-destructive mb-6">
          Failed to load API keys. Please try again.
        </div>
      )}

      {/* Loading state */}
      {isLoading && (
        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {Array.from({ length: 6 }).map((_, i) => (
            <ApiKeyCardSkeleton key={i} />
          ))}
        </div>
      )}

      {/* Empty state - no keys at all */}
      {!isLoading && totalKeys === 0 && !error && (
        <div className="text-center py-12">
          <Key className="h-12 w-12 text-muted-foreground mx-auto mb-4" />
          <h2 className="text-lg font-medium mb-2">No API keys yet</h2>
          <p className="text-sm text-muted-foreground max-w-md mx-auto mb-4">
            Create an API key to access the API programmatically. Keys can have budget limits and
            expiration dates.
          </p>
          <Button onClick={() => setIsCreateModalOpen(true)}>
            <Plus className="h-4 w-4 mr-2" />
            Create API Key
          </Button>
        </div>
      )}

      {/* No search results */}
      {!isLoading && totalKeys > 0 && totalFiltered === 0 && search && (
        <div className="text-center py-12">
          <Key className="h-12 w-12 text-muted-foreground mx-auto mb-4" />
          <h2 className="text-lg font-medium mb-2">No matching API keys</h2>
          <p className="text-sm text-muted-foreground">
            Try adjusting your search terms or{" "}
            <button onClick={() => setSearch("")} className="text-primary hover:underline">
              clear the search
            </button>
          </p>
        </div>
      )}

      {/* Sections */}
      {!isLoading && totalFiltered > 0 && (
        <div className="space-y-2">
          <KeySection
            title="My Keys"
            keys={filteredMyKeys}
            onRevoke={handleRevoke}
            onRotate={handleRotate}
          />

          <KeySection title="Organization Keys" keys={filteredOrgKeys} readOnly />

          <KeySection title="Team Keys" keys={filteredTeamKeys} readOnly />

          <KeySection title="Project Keys" keys={filteredProjectKeys} readOnly />

          <KeySection title="Service Account Keys" keys={filteredSaKeys} readOnly />
        </div>
      )}

      {/* Create modal */}
      <SelfServiceApiKeyFormModal
        isOpen={isCreateModalOpen}
        onClose={() => setIsCreateModalOpen(false)}
        onSubmit={handleCreateSubmit}
        isLoading={createMutation.isPending}
      />

      {/* Created key modal */}
      <ApiKeyCreatedModal apiKey={createdKey} onClose={() => setCreatedKey(null)} />
    </div>
  );
}
