import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { useParams, useNavigate } from "react-router-dom";
import {
  ArrowLeft,
  Users,
  Pencil,
  Trash2,
  Plus,
  Info,
  CheckCircle2,
  XCircle,
  Key,
  RefreshCw,
  Clock,
} from "lucide-react";
import { useState } from "react";

import {
  orgScimConfigGetOptions,
  orgScimConfigCreateMutation,
  orgScimConfigUpdateMutation,
  orgScimConfigDeleteMutation,
  orgScimConfigRotateTokenMutation,
  teamListOptions,
  organizationGetOptions,
} from "@/api/generated/@tanstack/react-query.gen";
import type { CreateOrgScimConfig, UpdateOrgScimConfig } from "@/api/generated/types.gen";
import { Button } from "@/components/Button/Button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/Card/Card";
import { Badge } from "@/components/Badge/Badge";
import { CodeBadge } from "@/components/CodeBadge/CodeBadge";
import { Skeleton } from "@/components/Skeleton/Skeleton";
import { useToast } from "@/components/Toast/Toast";
import { useConfirm } from "@/components/ConfirmDialog/ConfirmDialog";
import { ScimConfigFormModal, ScimTokenCreatedModal } from "@/components/ScimConfig";
import { formatDateTime } from "@/utils/formatters";

export default function ScimConfigPage() {
  const { orgSlug } = useParams<{ orgSlug: string }>();
  const navigate = useNavigate();
  const { toast } = useToast();
  const confirm = useConfirm();
  const queryClient = useQueryClient();

  const [isFormModalOpen, setIsFormModalOpen] = useState(false);
  const [createdToken, setCreatedToken] = useState<string | null>(null);

  // Fetch organization details
  const { data: org, isLoading: orgLoading } = useQuery(
    organizationGetOptions({ path: { slug: orgSlug! } })
  );

  // Fetch SCIM config for this org (returns 404 if none exists)
  const {
    data: config,
    isLoading: configLoading,
    error: configError,
  } = useQuery({
    ...orgScimConfigGetOptions({ path: { org_slug: orgSlug! } }),
    retry: false, // Don't retry 404s
  });

  // Fetch teams for the form modal
  const { data: teams } = useQuery({
    ...teamListOptions({ path: { org_slug: orgSlug! } }),
    enabled: isFormModalOpen,
  });

  // Create mutation
  const createMutation = useMutation({
    ...orgScimConfigCreateMutation(),
    onSuccess: (data) => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "orgScimConfigGet" }] });
      setIsFormModalOpen(false);
      setCreatedToken(data.token);
      toast({ title: "SCIM configuration created", type: "success" });
    },
    onError: (error) => {
      toast({
        title: "Failed to create SCIM configuration",
        description: String(error),
        type: "error",
      });
    },
  });

  // Update mutation
  const updateMutation = useMutation({
    ...orgScimConfigUpdateMutation(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "orgScimConfigGet" }] });
      setIsFormModalOpen(false);
      toast({ title: "SCIM configuration updated", type: "success" });
    },
    onError: (error) => {
      toast({
        title: "Failed to update SCIM configuration",
        description: String(error),
        type: "error",
      });
    },
  });

  // Delete mutation
  const deleteMutation = useMutation({
    ...orgScimConfigDeleteMutation(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "orgScimConfigGet" }] });
      toast({ title: "SCIM configuration deleted", type: "success" });
    },
    onError: (error) => {
      toast({
        title: "Failed to delete SCIM configuration",
        description: String(error),
        type: "error",
      });
    },
  });

  // Rotate token mutation
  const rotateTokenMutation = useMutation({
    ...orgScimConfigRotateTokenMutation(),
    onSuccess: (data) => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "orgScimConfigGet" }] });
      setCreatedToken(data.token);
      toast({ title: "SCIM token rotated", type: "success" });
    },
    onError: (error) => {
      toast({
        title: "Failed to rotate SCIM token",
        description: String(error),
        type: "error",
      });
    },
  });

  const handleCreate = (data: CreateOrgScimConfig) => {
    createMutation.mutate({
      path: { org_slug: orgSlug! },
      body: data,
    });
  };

  const handleUpdate = (data: UpdateOrgScimConfig) => {
    updateMutation.mutate({
      path: { org_slug: orgSlug! },
      body: data,
    });
  };

  const handleDelete = async () => {
    const confirmed = await confirm({
      title: "Delete SCIM Configuration",
      message:
        "Are you sure you want to delete this SCIM configuration? User provisioning from your IdP will stop working immediately.",
      confirmLabel: "Delete",
      variant: "destructive",
    });
    if (confirmed) {
      deleteMutation.mutate({
        path: { org_slug: orgSlug! },
      });
    }
  };

  const handleRotateToken = async () => {
    const confirmed = await confirm({
      title: "Rotate SCIM Token",
      message:
        "This will immediately invalidate the current token. You'll need to update your IdP with the new token. Continue?",
      confirmLabel: "Rotate Token",
      variant: "destructive",
    });
    if (confirmed) {
      rotateTokenMutation.mutate({
        path: { org_slug: orgSlug! },
      });
    }
  };

  // Check if config doesn't exist (404)
  const hasNoConfig =
    configError &&
    "status" in (configError as { status?: number }) &&
    (configError as { status?: number }).status === 404;

  if (orgLoading) {
    return (
      <div className="p-6 space-y-6">
        <Skeleton className="h-8 w-64" />
        <Skeleton className="h-32 w-full" />
      </div>
    );
  }

  return (
    <div className="p-6 space-y-6">
      {/* Header */}
      <div className="flex items-center gap-4">
        <Button
          variant="ghost"
          size="sm"
          onClick={() => navigate(`/admin/organizations/${orgSlug}`)}
        >
          <ArrowLeft className="mr-2 h-4 w-4" />
          Back to Organization
        </Button>
      </div>

      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold flex items-center gap-2">
            <Users className="h-6 w-6" />
            SCIM Configuration
          </h1>
          <p className="text-muted-foreground mt-1">
            {org?.name && <span>{org.name} • </span>}
            Configure SCIM 2.0 user provisioning for this organization
          </p>
        </div>
        {config && !hasNoConfig && (
          <div className="flex gap-2">
            <Button variant="outline" onClick={handleRotateToken}>
              <RefreshCw className="mr-2 h-4 w-4" />
              Rotate Token
            </Button>
            <Button variant="outline" onClick={() => setIsFormModalOpen(true)}>
              <Pencil className="mr-2 h-4 w-4" />
              Edit
            </Button>
            <Button variant="danger" onClick={handleDelete}>
              <Trash2 className="mr-2 h-4 w-4" />
              Delete
            </Button>
          </div>
        )}
      </div>

      {/* Info banner */}
      <div className="flex items-start gap-3 rounded-lg border bg-muted/30 p-4">
        <Info className="h-5 w-5 text-muted-foreground mt-0.5" />
        <div className="text-sm">
          <p className="font-medium">SCIM 2.0 Provisioning</p>
          <p className="text-muted-foreground">
            SCIM (System for Cross-domain Identity Management) enables automatic user provisioning
            from your identity provider. When users are added, modified, or removed in your IdP,
            changes are automatically synchronized to Hadrian.
          </p>
        </div>
      </div>

      {/* Loading state */}
      {configLoading && (
        <Card>
          <CardContent className="p-6">
            <div className="space-y-4">
              <Skeleton className="h-6 w-48" />
              <Skeleton className="h-4 w-full" />
              <Skeleton className="h-4 w-3/4" />
            </div>
          </CardContent>
        </Card>
      )}

      {/* Empty state */}
      {hasNoConfig && (
        <Card>
          <CardContent className="flex flex-col items-center justify-center p-12 text-center">
            <div className="flex h-12 w-12 items-center justify-center rounded-full bg-muted">
              <Users className="h-6 w-6 text-muted-foreground" />
            </div>
            <h3 className="mt-4 text-lg font-medium">No SCIM configured</h3>
            <p className="mt-2 max-w-md text-sm text-muted-foreground">
              This organization doesn't have SCIM configured yet. Enable SCIM to automatically
              provision and deprovision users from your identity provider.
            </p>
            <Button className="mt-6" onClick={() => setIsFormModalOpen(true)}>
              <Plus className="mr-2 h-4 w-4" />
              Enable SCIM
            </Button>
          </CardContent>
        </Card>
      )}

      {/* Config display */}
      {config && !hasNoConfig && (
        <div className="space-y-6">
          {/* Status overview */}
          <div className="grid grid-cols-3 gap-4">
            <Card>
              <CardContent className="p-4">
                <div className="flex items-center gap-3">
                  {config.enabled ? (
                    <div className="flex h-10 w-10 items-center justify-center rounded-full bg-green-100 dark:bg-green-900/30">
                      <CheckCircle2 className="h-5 w-5 text-green-700" />
                    </div>
                  ) : (
                    <div className="flex h-10 w-10 items-center justify-center rounded-full bg-gray-100 dark:bg-gray-800">
                      <XCircle className="h-5 w-5 text-gray-500" />
                    </div>
                  )}
                  <div>
                    <p className="text-sm text-muted-foreground">Status</p>
                    <p className="font-medium">{config.enabled ? "Enabled" : "Disabled"}</p>
                  </div>
                </div>
              </CardContent>
            </Card>

            <Card>
              <CardContent className="p-4">
                <div className="flex items-center gap-3">
                  <div className="flex h-10 w-10 items-center justify-center rounded-full bg-blue-100 dark:bg-blue-900/30">
                    <Key className="h-5 w-5 text-blue-700" />
                  </div>
                  <div>
                    <p className="text-sm text-muted-foreground">Token Prefix</p>
                    <CodeBadge>{config.token_prefix}...</CodeBadge>
                  </div>
                </div>
              </CardContent>
            </Card>

            <Card>
              <CardContent className="p-4">
                <div className="flex items-center gap-3">
                  <div className="flex h-10 w-10 items-center justify-center rounded-full bg-orange-100 dark:bg-orange-900/30">
                    <Clock className="h-5 w-5 text-orange-800" />
                  </div>
                  <div>
                    <p className="text-sm text-muted-foreground">Last Used</p>
                    <p className="font-medium">
                      {config.token_last_used_at
                        ? formatDateTime(config.token_last_used_at)
                        : "Never"}
                    </p>
                  </div>
                </div>
              </CardContent>
            </Card>
          </div>

          {/* Provisioning Settings */}
          <Card>
            <CardHeader className="flex flex-row items-center justify-between">
              <CardTitle className="text-base">Provisioning Settings</CardTitle>
              <Badge variant={config.create_users ? "default" : "secondary"}>
                {config.create_users ? "Auto-create enabled" : "Auto-create disabled"}
              </Badge>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="grid grid-cols-3 gap-4">
                <div>
                  <p className="text-sm text-muted-foreground">Create Users</p>
                  <p className="font-medium">{config.create_users ? "Yes" : "No"}</p>
                </div>
                <div>
                  <p className="text-sm text-muted-foreground">Sync Display Name</p>
                  <p className="font-medium">{config.sync_display_name ? "Yes" : "No"}</p>
                </div>
                <div>
                  <p className="text-sm text-muted-foreground">Default Team</p>
                  <p className="font-medium">
                    {config.default_team_id ? (
                      <CodeBadge>{config.default_team_id}</CodeBadge>
                    ) : (
                      <span className="text-muted-foreground">None</span>
                    )}
                  </p>
                </div>
              </div>
              <div className="grid grid-cols-2 gap-4">
                <div>
                  <p className="text-sm text-muted-foreground">Default Org Role</p>
                  <Badge variant="outline">{config.default_org_role}</Badge>
                </div>
                <div>
                  <p className="text-sm text-muted-foreground">Default Team Role</p>
                  <Badge variant="outline">{config.default_team_role}</Badge>
                </div>
              </div>
            </CardContent>
          </Card>

          {/* Deprovisioning Settings */}
          <Card>
            <CardHeader className="flex flex-row items-center justify-between">
              <CardTitle className="text-base">Deprovisioning Settings</CardTitle>
              <Badge variant={config.revoke_api_keys_on_deactivate ? "destructive" : "secondary"}>
                {config.revoke_api_keys_on_deactivate
                  ? "API keys revoked on deactivation"
                  : "API keys preserved"}
              </Badge>
            </CardHeader>
            <CardContent>
              <div className="grid grid-cols-2 gap-4">
                <div>
                  <p className="text-sm text-muted-foreground">Delete Users on Deactivation</p>
                  <p className="font-medium">{config.deactivate_deletes_user ? "Yes" : "No"}</p>
                  <p className="text-xs text-muted-foreground mt-1">
                    {config.deactivate_deletes_user
                      ? "Users are permanently deleted when deactivated"
                      : "Users are marked inactive, not deleted"}
                  </p>
                </div>
                <div>
                  <p className="text-sm text-muted-foreground">Revoke API Keys</p>
                  <p className="font-medium">
                    {config.revoke_api_keys_on_deactivate ? "Yes" : "No"}
                  </p>
                  <p className="text-xs text-muted-foreground mt-1">
                    {config.revoke_api_keys_on_deactivate
                      ? "All API keys are revoked when user is deactivated"
                      : "API keys remain active when user is deactivated"}
                  </p>
                </div>
              </div>
            </CardContent>
          </Card>

          {/* SCIM Endpoint Info */}
          <Card>
            <CardHeader>
              <CardTitle className="text-base">SCIM Endpoint</CardTitle>
            </CardHeader>
            <CardContent className="space-y-4">
              <div>
                <p className="text-sm text-muted-foreground">Base URL</p>
                <code className="text-sm bg-muted px-2 py-1 rounded">
                  {window.location.origin}/scim/v2/
                </code>
              </div>
              <div>
                <p className="text-sm text-muted-foreground mb-2">Supported Resources</p>
                <div className="flex gap-2">
                  <Badge variant="secondary">Users</Badge>
                  <Badge variant="secondary">Groups</Badge>
                </div>
              </div>
            </CardContent>
          </Card>

          {/* Timestamps */}
          <Card>
            <CardContent className="p-4">
              <div className="flex justify-between text-sm text-muted-foreground">
                <span>Created: {formatDateTime(config.created_at)}</span>
                <span>Updated: {formatDateTime(config.updated_at)}</span>
              </div>
            </CardContent>
          </Card>
        </div>
      )}

      {/* Form Modal */}
      <ScimConfigFormModal
        open={isFormModalOpen}
        onClose={() => setIsFormModalOpen(false)}
        onCreateSubmit={handleCreate}
        onUpdateSubmit={handleUpdate}
        isLoading={createMutation.isPending || updateMutation.isPending}
        editingConfig={config && !hasNoConfig ? config : null}
        teams={teams?.data ?? []}
      />

      {/* Token Created Modal */}
      <ScimTokenCreatedModal token={createdToken} onClose={() => setCreatedToken(null)} />
    </div>
  );
}
