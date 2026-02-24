import { useState, useCallback } from "react";
import { useParams, Link } from "react-router-dom";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import {
  ArrowLeft,
  Key,
  BarChart3,
  RotateCw,
  Trash2,
  Calendar,
  Clock,
  DollarSign,
  Shield,
  Cpu,
  Lock,
  Network,
} from "lucide-react";

import {
  meApiKeysGetOptions,
  meApiKeysRevokeMutation,
  meApiKeysRotateMutation,
} from "@/api/generated/@tanstack/react-query.gen";
import type { ApiKey } from "@/api/generated/types.gen";
import { Button } from "@/components/Button/Button";
import { Badge } from "@/components/Badge/Badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/Card/Card";
import { CodeBadge } from "@/components/CodeBadge/CodeBadge";
import { Skeleton } from "@/components/Skeleton/Skeleton";
import { ApiKeyStatusBadge, TabNavigation, ApiKeyCreatedModal, type Tab } from "@/components/Admin";
import { useToast } from "@/components/Toast/Toast";
import { useConfirm } from "@/components/ConfirmDialog/ConfirmDialog";
import { formatDateTime, formatCurrency, formatRelativeTime } from "@/utils/formatters";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/Tooltip/Tooltip";
import UsageDashboard from "@/components/UsageDashboard/UsageDashboard";

type TabId = "overview" | "usage";

const tabs: Tab<TabId>[] = [
  { id: "overview", label: "Overview", icon: <Key className="h-4 w-4" /> },
  { id: "usage", label: "Usage", icon: <BarChart3 className="h-4 w-4" /> },
];

export default function ApiKeyDetailPage() {
  const { keyId } = useParams<{ keyId: string }>();
  const { toast } = useToast();
  const confirm = useConfirm();
  const queryClient = useQueryClient();

  const [activeTab, setActiveTab] = useState<TabId>("overview");
  const [createdKey, setCreatedKey] = useState<string | null>(null);

  const {
    data: apiKey,
    isLoading,
    error,
  } = useQuery(meApiKeysGetOptions({ path: { key_id: keyId! } }));

  const revokeMutation = useMutation({
    ...meApiKeysRevokeMutation(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "meApiKeysGet" }] });
      queryClient.invalidateQueries({ queryKey: [{ _id: "meApiKeysList" }] });
      queryClient.invalidateQueries({ queryKey: [{ _id: "apiKeyListByOrg" }] });
      toast({ title: "API key revoked", type: "success" });
    },
    onError: (err) => {
      toast({ title: "Failed to revoke API key", description: String(err), type: "error" });
    },
  });

  const rotateMutation = useMutation({
    ...meApiKeysRotateMutation(),
    onSuccess: (data) => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "meApiKeysGet" }] });
      queryClient.invalidateQueries({ queryKey: [{ _id: "meApiKeysList" }] });
      queryClient.invalidateQueries({ queryKey: [{ _id: "apiKeyListByOrg" }] });
      if (data && "key" in data) {
        setCreatedKey(data.key);
      }
      toast({ title: "API key rotated", type: "success" });
    },
    onError: (err) => {
      toast({ title: "Failed to rotate API key", description: String(err), type: "error" });
    },
  });

  const handleRevoke = useCallback(async () => {
    if (!apiKey) return;
    const confirmed = await confirm({
      title: "Revoke API Key",
      message: `Are you sure you want to revoke "${apiKey.name}"? This action cannot be undone and the key will no longer work.`,
      confirmLabel: "Revoke",
      variant: "destructive",
    });
    if (confirmed) {
      revokeMutation.mutate({ path: { key_id: keyId! } });
    }
  }, [apiKey, confirm, revokeMutation, keyId]);

  const handleRotate = useCallback(async () => {
    if (!apiKey) return;
    const confirmed = await confirm({
      title: "Rotate API Key",
      message: `This will create a new API key and mark "${apiKey.name}" for expiration after a 24-hour grace period. During this time, both keys will work.`,
      confirmLabel: "Rotate",
    });
    if (confirmed) {
      rotateMutation.mutate({
        path: { key_id: keyId! },
        body: { grace_period_seconds: 86400 },
      });
    }
  }, [apiKey, confirm, rotateMutation, keyId]);

  if (isLoading) {
    return (
      <div className="p-6 max-w-6xl mx-auto space-y-6">
        <Skeleton className="h-4 w-32" />
        <Skeleton className="h-8 w-64" />
        <Skeleton className="h-32 w-full" />
      </div>
    );
  }

  if (error || !apiKey) {
    return (
      <div className="p-6 max-w-6xl mx-auto">
        <div className="text-center py-12 text-destructive">
          API key not found or failed to load.
          <br />
          <Link to="/api-keys" className="mt-4 inline-flex items-center gap-1 text-primary text-sm">
            <ArrowLeft className="h-4 w-4" />
            Back to API Keys
          </Link>
        </div>
      </div>
    );
  }

  const isRevoked = !!apiKey.revoked_at;
  const isRotating = !!apiKey.rotation_grace_until;
  const graceEndTime = apiKey.rotation_grace_until ? new Date(apiKey.rotation_grace_until) : null;
  const isGraceExpired = graceEndTime && graceEndTime < new Date();

  return (
    <div className="p-6 max-w-6xl mx-auto space-y-6">
      {/* Breadcrumb */}
      <Link
        to="/api-keys"
        className="inline-flex items-center gap-1 text-sm text-muted-foreground hover:text-foreground"
      >
        <ArrowLeft className="h-4 w-4" />
        Back to API Keys
      </Link>

      {/* Header */}
      <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
        <div>
          <div className="flex items-center gap-3">
            <h1 className="text-2xl font-semibold">{apiKey.name}</h1>
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
          </div>
          <div className="flex items-center gap-2 mt-1">
            <CodeBadge className="text-xs">{apiKey.key_prefix}...</CodeBadge>
            <span className="text-sm text-muted-foreground">
              Created {formatDateTime(apiKey.created_at)}
            </span>
          </div>
        </div>
        <div className="flex gap-2">
          <Button
            variant="outline"
            disabled={isRevoked || isRotating}
            onClick={handleRotate}
            isLoading={rotateMutation.isPending}
          >
            <RotateCw className="mr-2 h-4 w-4" />
            Rotate
          </Button>
          <Button
            variant="danger"
            disabled={isRevoked}
            onClick={handleRevoke}
            isLoading={revokeMutation.isPending}
          >
            <Trash2 className="mr-2 h-4 w-4" />
            Revoke
          </Button>
        </div>
      </div>

      {/* Tab navigation */}
      <TabNavigation tabs={tabs} activeTab={activeTab} onTabChange={setActiveTab} />

      {/* Tab content */}
      {activeTab === "overview" && <OverviewTab apiKey={apiKey} />}
      {activeTab === "usage" && (
        <div role="tabpanel" id="tabpanel-usage" aria-labelledby="tab-usage">
          <UsageDashboard scope={{ type: "apiKey", keyId: keyId! }} />
        </div>
      )}

      {/* Created key modal (shown after rotation) */}
      <ApiKeyCreatedModal apiKey={createdKey} onClose={() => setCreatedKey(null)} />
    </div>
  );
}

function OverviewTab({ apiKey }: { apiKey: ApiKey }) {
  const hasScopes = apiKey.scopes && apiKey.scopes.length > 0;
  const hasModelRestrictions = apiKey.allowed_models && apiKey.allowed_models.length > 0;
  const hasIpRestrictions = apiKey.ip_allowlist && apiKey.ip_allowlist.length > 0;
  const hasRateLimits = apiKey.rate_limit_rpm || apiKey.rate_limit_tpm;

  return (
    <div
      role="tabpanel"
      id="tabpanel-overview"
      aria-labelledby="tab-overview"
      className="grid gap-6 sm:grid-cols-2"
    >
      {/* Details Card */}
      <Card>
        <CardHeader>
          <CardTitle>Details</CardTitle>
        </CardHeader>
        <CardContent className="space-y-3 text-sm">
          <div className="flex justify-between">
            <span className="text-muted-foreground">Last Used</span>
            <span className="flex items-center gap-1">
              <Clock className="h-3.5 w-3.5 text-muted-foreground" />
              {apiKey.last_used_at ? formatDateTime(apiKey.last_used_at) : "Never"}
            </span>
          </div>
          <div className="flex justify-between">
            <span className="text-muted-foreground">Created</span>
            <span className="flex items-center gap-1">
              <Calendar className="h-3.5 w-3.5 text-muted-foreground" />
              {formatDateTime(apiKey.created_at)}
            </span>
          </div>
          {apiKey.expires_at && (
            <div className="flex justify-between">
              <span className="text-muted-foreground">Expires</span>
              <span>{formatDateTime(apiKey.expires_at)}</span>
            </div>
          )}
          {apiKey.budget_limit_cents != null && (
            <div className="flex justify-between">
              <span className="text-muted-foreground">Budget</span>
              <span className="flex items-center gap-1">
                <DollarSign className="h-3.5 w-3.5 text-muted-foreground" />
                {formatCurrency(apiKey.budget_limit_cents / 100)}
                {apiKey.budget_period && (
                  <span className="text-muted-foreground">/{apiKey.budget_period}</span>
                )}
              </span>
            </div>
          )}
          {apiKey.rotated_from_key_id && (
            <div className="flex justify-between">
              <span className="text-muted-foreground">Rotated From</span>
              <CodeBadge className="text-xs">{apiKey.rotated_from_key_id}</CodeBadge>
            </div>
          )}
        </CardContent>
      </Card>

      {/* Restrictions Card */}
      <Card>
        <CardHeader>
          <CardTitle>Restrictions</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4 text-sm">
          {!hasScopes && !hasModelRestrictions && !hasIpRestrictions && !hasRateLimits && (
            <p className="text-muted-foreground">No restrictions configured. Full access.</p>
          )}

          {hasScopes && (
            <div>
              <div className="flex items-center gap-1.5 font-medium mb-1.5">
                <Shield className="h-4 w-4 text-muted-foreground" />
                Permission Scopes
              </div>
              <div className="flex flex-wrap gap-1.5">
                {apiKey.scopes!.map((scope: string) => (
                  <Badge key={scope} variant="secondary">
                    {scope}
                  </Badge>
                ))}
              </div>
            </div>
          )}

          {hasModelRestrictions && (
            <div>
              <div className="flex items-center gap-1.5 font-medium mb-1.5">
                <Cpu className="h-4 w-4 text-muted-foreground" />
                Allowed Models
              </div>
              <div className="flex flex-wrap gap-1.5">
                {apiKey.allowed_models!.map((model: string) => (
                  <CodeBadge key={model}>{model}</CodeBadge>
                ))}
              </div>
            </div>
          )}

          {hasIpRestrictions && (
            <div>
              <div className="flex items-center gap-1.5 font-medium mb-1.5">
                <Lock className="h-4 w-4 text-muted-foreground" />
                IP Allowlist
              </div>
              <div className="flex flex-wrap gap-1.5">
                {apiKey.ip_allowlist!.map((ip: string) => (
                  <CodeBadge key={ip}>{ip}</CodeBadge>
                ))}
              </div>
            </div>
          )}

          {hasRateLimits && (
            <div>
              <div className="flex items-center gap-1.5 font-medium mb-1.5">
                <Network className="h-4 w-4 text-muted-foreground" />
                Rate Limits
              </div>
              <div className="space-y-1">
                {apiKey.rate_limit_rpm && (
                  <div className="flex justify-between">
                    <span className="text-muted-foreground">Requests/min</span>
                    <span>{apiKey.rate_limit_rpm.toLocaleString()}</span>
                  </div>
                )}
                {apiKey.rate_limit_tpm && (
                  <div className="flex justify-between">
                    <span className="text-muted-foreground">Tokens/min</span>
                    <span>{apiKey.rate_limit_tpm.toLocaleString()}</span>
                  </div>
                )}
              </div>
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
