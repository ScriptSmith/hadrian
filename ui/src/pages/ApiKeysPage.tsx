import { useState, useMemo } from "react";
import {
  Key,
  Plus,
  Users,
  Calendar,
  Clock,
  DollarSign,
  Shield,
  Network,
  Cpu,
  RotateCw,
  Lock,
} from "lucide-react";
import { useQuery, useQueries, useMutation, useQueryClient } from "@tanstack/react-query";

import {
  organizationListOptions,
  apiKeyListByOrgOptions,
  apiKeyCreateMutation,
  apiKeyRevokeMutation,
  apiKeyRotateMutation,
} from "@/api/generated/@tanstack/react-query.gen";
import type { ApiKey, CreateApiKey } from "@/api/generated/types.gen";
import { Button } from "@/components/Button/Button";
import { Badge } from "@/components/Badge/Badge";
import { Card, CardContent } from "@/components/Card/Card";
import { Input } from "@/components/Input/Input";
import { Skeleton } from "@/components/Skeleton/Skeleton";
import { CodeBadge } from "@/components/CodeBadge/CodeBadge";
import {
  ApiKeyStatusBadge,
  OwnerBadge,
  ApiKeyFormModal,
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

interface ApiKeyWithOrg extends ApiKey {
  org_slug: string;
  org_name: string;
}

function ApiKeyCard({
  apiKey,
  onRevoke,
  onRotate,
}: {
  apiKey: ApiKeyWithOrg;
  onRevoke: (key: ApiKey) => void;
  onRotate: (key: ApiKey) => void;
}) {
  const isRevoked = !!apiKey.revoked_at;
  const isRotating = !!apiKey.rotation_grace_until;
  const graceEndTime = apiKey.rotation_grace_until ? new Date(apiKey.rotation_grace_until) : null;
  const isGraceExpired = graceEndTime && graceEndTime < new Date();

  // Compute badges for advanced settings
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
            <h3 className="font-medium truncate">{apiKey.name}</h3>
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
            <Dropdown>
              <DropdownTrigger aria-label="Actions" variant="ghost" className="h-8 w-8 p-0">
                <MoreHorizontal className="h-4.5 w-4.5" />
              </DropdownTrigger>
              <DropdownContent align="end">
                <DropdownItem disabled={isRevoked || isRotating} onClick={() => onRotate(apiKey)}>
                  <RotateCw className="mr-2 h-4 w-4" />
                  {isRotating ? "Already Rotating" : "Rotate"}
                </DropdownItem>
                <DropdownItem
                  className="text-destructive"
                  disabled={isRevoked}
                  onClick={() => onRevoke(apiKey)}
                >
                  <Trash2 className="mr-2 h-4 w-4" />
                  {isRevoked ? "Already Revoked" : "Revoke"}
                </DropdownItem>
              </DropdownContent>
            </Dropdown>
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

export default function ApiKeysPage() {
  const [search, setSearch] = useState("");
  const [isCreateModalOpen, setIsCreateModalOpen] = useState(false);
  const [createdKey, setCreatedKey] = useState<string | null>(null);
  const { toast } = useToast();
  const confirm = useConfirm();
  const queryClient = useQueryClient();

  // Fetch organizations
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

  // Combine API keys with org info
  const apiKeys = useMemo(() => {
    const result: ApiKeyWithOrg[] = [];
    for (let i = 0; i < organizations.length; i++) {
      const org = organizations[i];
      const keysData = apiKeyQueries[i]?.data?.data ?? [];
      for (const key of keysData) {
        result.push({
          ...key,
          org_slug: org.slug,
          org_name: org.name,
        });
      }
    }
    // Sort by created_at descending (newest first)
    result.sort((a, b) => new Date(b.created_at).getTime() - new Date(a.created_at).getTime());
    return result;
  }, [organizations, apiKeyQueries]);

  const isLoading = orgsLoading || apiKeyQueries.some((q) => q.isLoading);
  const error = orgsError ?? apiKeyQueries.find((q) => q.error)?.error;

  const filteredApiKeys = apiKeys.filter(
    (key) =>
      key.name.toLowerCase().includes(search.toLowerCase()) ||
      key.key_prefix.toLowerCase().includes(search.toLowerCase())
  );

  const createMutation = useMutation({
    ...apiKeyCreateMutation(),
    onSuccess: (data) => {
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
    ...apiKeyRevokeMutation(),
    onSuccess: () => {
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
    ...apiKeyRotateMutation(),
    onSuccess: (data) => {
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

  const handleCreateSubmit = (data: CreateApiKey) => {
    createMutation.mutate({ body: data });
  };

  const handleRevoke = async (key: ApiKey) => {
    const confirmed = await confirm({
      title: "Revoke API Key",
      message: `Are you sure you want to revoke "${key.name}"? This action cannot be undone and the key will no longer work.`,
      confirmLabel: "Revoke",
      variant: "destructive",
    });
    if (confirmed) {
      revokeMutation.mutate({ path: { key_id: key.id } });
    }
  };

  const handleRotate = async (key: ApiKey) => {
    const confirmed = await confirm({
      title: "Rotate API Key",
      message: `This will create a new API key and mark the old one "${key.name}" for expiration after a 24-hour grace period. During this time, both keys will work.`,
      confirmLabel: "Rotate",
    });
    if (confirmed) {
      rotateMutation.mutate({
        path: { key_id: key.id },
        body: { grace_period_seconds: 86400 }, // 24 hours
      });
    }
  };

  // Count active vs revoked keys
  const activeCount = apiKeys.filter((k) => !k.revoked_at).length;
  const revokedCount = apiKeys.filter((k) => !!k.revoked_at).length;

  return (
    <div className="p-6 max-w-6xl mx-auto">
      {/* Header */}
      <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between mb-6">
        <div>
          <h1 className="text-2xl font-semibold">API Keys</h1>
          <p className="text-sm text-muted-foreground mt-1">
            Manage API keys for programmatic access to your resources
          </p>
        </div>
        <Button onClick={() => setIsCreateModalOpen(true)} disabled={organizations.length === 0}>
          <Plus className="h-4 w-4 mr-2" />
          New API Key
        </Button>
      </div>

      {/* Stats */}
      {!isLoading && apiKeys.length > 0 && (
        <div className="flex items-center gap-4 mb-6">
          <Badge variant="secondary">{activeCount} active</Badge>
          {revokedCount > 0 && <Badge variant="outline">{revokedCount} revoked</Badge>}
        </div>
      )}

      {/* Search */}
      <div className="mb-6">
        <Input
          placeholder="Search API keys..."
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          className="max-w-sm"
        />
      </div>

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

      {/* Empty state - no organizations */}
      {!isLoading && organizations.length === 0 && (
        <div className="text-center py-12">
          <Users className="h-12 w-12 text-muted-foreground mx-auto mb-4" />
          <h2 className="text-lg font-medium mb-2">No organizations</h2>
          <p className="text-sm text-muted-foreground max-w-md mx-auto">
            You need to be a member of an organization to create API keys. Contact your
            administrator to get access.
          </p>
        </div>
      )}

      {/* Empty state - no API keys */}
      {!isLoading && organizations.length > 0 && apiKeys.length === 0 && (
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

      {/* Empty state - no search results */}
      {!isLoading && apiKeys.length > 0 && filteredApiKeys.length === 0 && (
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

      {/* API keys grid */}
      {!isLoading && filteredApiKeys.length > 0 && (
        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {filteredApiKeys.map((key) => (
            <ApiKeyCard key={key.id} apiKey={key} onRevoke={handleRevoke} onRotate={handleRotate} />
          ))}
        </div>
      )}

      {/* Create modal */}
      <ApiKeyFormModal
        isOpen={isCreateModalOpen}
        onClose={() => setIsCreateModalOpen(false)}
        onSubmit={handleCreateSubmit}
        isLoading={createMutation.isPending}
        organizations={organizations}
      />

      {/* Created key modal */}
      <ApiKeyCreatedModal apiKey={createdKey} onClose={() => setCreatedKey(null)} />
    </div>
  );
}
