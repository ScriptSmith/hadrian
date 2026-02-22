import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { useParams, useNavigate, Link } from "react-router-dom";
import {
  ArrowLeft,
  Shield,
  Pencil,
  Trash2,
  Plus,
  Info,
  CheckCircle2,
  XCircle,
  Clock,
  Users,
  ExternalLink,
} from "lucide-react";
import { useState } from "react";

import {
  orgSsoConfigGetOptions,
  orgSsoConfigCreateMutation,
  orgSsoConfigUpdateMutation,
  orgSsoConfigDeleteMutation,
  teamListOptions,
  organizationGetOptions,
} from "@/api/generated/@tanstack/react-query.gen";
import type { CreateOrgSsoConfig, UpdateOrgSsoConfig } from "@/api/generated/types.gen";
import { Button } from "@/components/Button/Button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/Card/Card";
import { Badge } from "@/components/Badge/Badge";
import { CodeBadge } from "@/components/CodeBadge/CodeBadge";
import { Skeleton } from "@/components/Skeleton/Skeleton";
import { useToast } from "@/components/Toast/Toast";
import { useConfirm } from "@/components/ConfirmDialog/ConfirmDialog";
import { OrgSsoConfigFormModal } from "@/components/OrgSsoConfigForm";
import {
  DomainVerificationList,
  AddDomainModal,
  VerificationInstructionsModal,
} from "@/components/DomainVerification";
import { formatDateTime } from "@/utils/formatters";

export default function OrgSsoConfigPage() {
  const { orgSlug } = useParams<{ orgSlug: string }>();
  const navigate = useNavigate();
  const { toast } = useToast();
  const confirm = useConfirm();
  const queryClient = useQueryClient();

  const [isFormModalOpen, setIsFormModalOpen] = useState(false);
  const [isAddDomainModalOpen, setIsAddDomainModalOpen] = useState(false);
  const [viewingDomainId, setViewingDomainId] = useState<string | null>(null);

  // Fetch organization details
  const { data: org, isLoading: orgLoading } = useQuery(
    organizationGetOptions({ path: { slug: orgSlug! } })
  );

  // Fetch SSO config for this org (returns 404 if none exists)
  const {
    data: config,
    isLoading: configLoading,
    error: configError,
  } = useQuery({
    ...orgSsoConfigGetOptions({ path: { org_slug: orgSlug! } }),
    retry: false, // Don't retry 404s
  });

  // Fetch teams for the form modal
  const { data: teams } = useQuery({
    ...teamListOptions({ path: { org_slug: orgSlug! } }),
    enabled: isFormModalOpen,
  });

  // Create mutation
  const createMutation = useMutation({
    ...orgSsoConfigCreateMutation(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "orgSsoConfigGet" }] });
      setIsFormModalOpen(false);
      toast({ title: "SSO configuration created", type: "success" });
    },
    onError: (error) => {
      toast({
        title: "Failed to create SSO configuration",
        description: String(error),
        type: "error",
      });
    },
  });

  // Update mutation
  const updateMutation = useMutation({
    ...orgSsoConfigUpdateMutation(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "orgSsoConfigGet" }] });
      setIsFormModalOpen(false);
      toast({ title: "SSO configuration updated", type: "success" });
    },
    onError: (error) => {
      toast({
        title: "Failed to update SSO configuration",
        description: String(error),
        type: "error",
      });
    },
  });

  // Delete mutation
  const deleteMutation = useMutation({
    ...orgSsoConfigDeleteMutation(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "orgSsoConfigGet" }] });
      toast({ title: "SSO configuration deleted", type: "success" });
    },
    onError: (error) => {
      toast({
        title: "Failed to delete SSO configuration",
        description: String(error),
        type: "error",
      });
    },
  });

  const handleCreate = (data: CreateOrgSsoConfig) => {
    createMutation.mutate({
      path: { org_slug: orgSlug! },
      body: data,
    });
  };

  const handleUpdate = (data: UpdateOrgSsoConfig) => {
    updateMutation.mutate({
      path: { org_slug: orgSlug! },
      body: data,
    });
  };

  const handleDelete = async () => {
    const confirmed = await confirm({
      title: "Delete SSO Configuration",
      message:
        "Are you sure you want to delete this SSO configuration? Users will no longer be able to authenticate via this IdP.",
      confirmLabel: "Delete",
      variant: "destructive",
    });
    if (confirmed) {
      deleteMutation.mutate({
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
            <Shield className="h-6 w-6" />
            SSO Configuration
          </h1>
          <p className="text-muted-foreground mt-1">
            {org?.name && <span>{org.name} • </span>}
            Configure Single Sign-On for this organization
          </p>
        </div>
        {config && !hasNoConfig && (
          <div className="flex gap-2">
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
          <p className="font-medium">Per-Organization SSO</p>
          <p className="text-muted-foreground">
            Configure your own identity provider (OIDC) for this organization. Users will be able to
            authenticate via your IdP and be automatically provisioned based on the JIT settings.
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
              <Shield className="h-6 w-6 text-muted-foreground" />
            </div>
            <h3 className="mt-4 text-lg font-medium">No SSO configured</h3>
            <p className="mt-2 max-w-md text-sm text-muted-foreground">
              This organization doesn't have SSO configured yet. Configure an identity provider to
              enable single sign-on for your users.
            </p>
            <Button className="mt-6" onClick={() => setIsFormModalOpen(true)}>
              <Plus className="mr-2 h-4 w-4" />
              Configure SSO
            </Button>
          </CardContent>
        </Card>
      )}

      {/* Config display */}
      {config && !hasNoConfig && (
        <div className="space-y-6">
          {/* Status overview */}
          <div className="grid grid-cols-4 gap-4">
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
                    <Shield className="h-5 w-5 text-blue-700" />
                  </div>
                  <div>
                    <p className="text-sm text-muted-foreground">Provider</p>
                    <p className="font-medium uppercase">{config.provider_type}</p>
                  </div>
                </div>
              </CardContent>
            </Card>

            <Card>
              <CardContent className="p-4">
                <div className="flex items-center gap-3">
                  <div className="flex h-10 w-10 items-center justify-center rounded-full bg-purple-100 dark:bg-purple-900/30">
                    <Users className="h-5 w-5 text-purple-600" />
                  </div>
                  <div>
                    <p className="text-sm text-muted-foreground">Provisioning</p>
                    <p className="font-medium">
                      {config.provisioning_enabled ? "Enabled" : "Disabled"}
                    </p>
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
                    <p className="text-sm text-muted-foreground">Enforcement</p>
                    <p className="font-medium capitalize">{config.enforcement_mode}</p>
                  </div>
                </div>
              </CardContent>
            </Card>
          </div>

          {/* Provider Settings */}
          <Card>
            <CardHeader>
              <CardTitle className="text-base">Provider Settings</CardTitle>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="grid grid-cols-2 gap-4">
                <div>
                  <p className="text-sm text-muted-foreground">Issuer URL</p>
                  <p className="font-mono text-sm">{config.issuer}</p>
                </div>
                <div>
                  <p className="text-sm text-muted-foreground">Client ID</p>
                  <CodeBadge>{config.client_id}</CodeBadge>
                </div>
              </div>
              {config.discovery_url && (
                <div>
                  <p className="text-sm text-muted-foreground">Discovery URL</p>
                  <p className="font-mono text-sm">{config.discovery_url}</p>
                </div>
              )}
              {config.redirect_uri && (
                <div>
                  <p className="text-sm text-muted-foreground">Redirect URI</p>
                  <p className="font-mono text-sm">{config.redirect_uri}</p>
                </div>
              )}
              <div>
                <p className="text-sm text-muted-foreground">Scopes</p>
                <div className="flex flex-wrap gap-1 mt-1">
                  {config.scopes.map((scope) => (
                    <Badge key={scope} variant="secondary">
                      {scope}
                    </Badge>
                  ))}
                </div>
              </div>
            </CardContent>
          </Card>

          {/* Token Claims */}
          <Card>
            <CardHeader>
              <CardTitle className="text-base">Token Claims</CardTitle>
            </CardHeader>
            <CardContent>
              <div className="grid grid-cols-3 gap-4">
                <div>
                  <p className="text-sm text-muted-foreground">Identity Claim</p>
                  <CodeBadge>{config.identity_claim}</CodeBadge>
                </div>
                <div>
                  <p className="text-sm text-muted-foreground">Org Claim</p>
                  {config.org_claim ? (
                    <CodeBadge>{config.org_claim}</CodeBadge>
                  ) : (
                    <span className="text-sm text-muted-foreground">Not configured</span>
                  )}
                </div>
                <div>
                  <p className="text-sm text-muted-foreground">Groups Claim</p>
                  {config.groups_claim ? (
                    <CodeBadge>{config.groups_claim}</CodeBadge>
                  ) : (
                    <span className="text-sm text-muted-foreground">Not configured</span>
                  )}
                </div>
              </div>
            </CardContent>
          </Card>

          {/* JIT Provisioning */}
          <Card>
            <CardHeader className="flex flex-row items-center justify-between">
              <CardTitle className="text-base">JIT Provisioning</CardTitle>
              <Badge variant={config.provisioning_enabled ? "default" : "secondary"}>
                {config.provisioning_enabled ? "Enabled" : "Disabled"}
              </Badge>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="grid grid-cols-3 gap-4">
                <div>
                  <p className="text-sm text-muted-foreground">Create Users</p>
                  <p className="font-medium">{config.create_users ? "Yes" : "No"}</p>
                </div>
                <div>
                  <p className="text-sm text-muted-foreground">Default Org Role</p>
                  <Badge variant="outline">{config.default_org_role}</Badge>
                </div>
                <div>
                  <p className="text-sm text-muted-foreground">Default Team Role</p>
                  <Badge variant="outline">{config.default_team_role}</Badge>
                </div>
              </div>
              {config.default_team_id && (
                <div>
                  <p className="text-sm text-muted-foreground">Default Team</p>
                  <p className="font-mono text-sm">{config.default_team_id}</p>
                </div>
              )}
              {config.allowed_email_domains.length > 0 && (
                <div>
                  <p className="text-sm text-muted-foreground">Allowed Email Domains</p>
                  <div className="flex flex-wrap gap-1 mt-1">
                    {config.allowed_email_domains.map((domain) => (
                      <Badge key={domain} variant="outline">
                        {domain}
                      </Badge>
                    ))}
                  </div>
                </div>
              )}
              <div className="grid grid-cols-2 gap-4">
                <div>
                  <p className="text-sm text-muted-foreground">Sync Attributes on Login</p>
                  <p className="font-medium">{config.sync_attributes_on_login ? "Yes" : "No"}</p>
                </div>
                <div>
                  <p className="text-sm text-muted-foreground">Sync Team Memberships on Login</p>
                  <p className="font-medium">{config.sync_memberships_on_login ? "Yes" : "No"}</p>
                </div>
              </div>
            </CardContent>
          </Card>

          {/* Domain Verification */}
          <DomainVerificationList
            orgSlug={orgSlug!}
            onAddDomain={() => setIsAddDomainModalOpen(true)}
            onViewInstructions={(domainId) => setViewingDomainId(domainId)}
          />

          {/* Timestamps */}
          <Card>
            <CardContent className="p-4">
              <div className="flex justify-between text-sm text-muted-foreground">
                <span>Created: {formatDateTime(config.created_at)}</span>
                <span>Updated: {formatDateTime(config.updated_at)}</span>
              </div>
            </CardContent>
          </Card>

          {/* Related Links */}
          <Card>
            <CardHeader>
              <CardTitle className="text-base">Related</CardTitle>
            </CardHeader>
            <CardContent>
              <Link
                to={`/admin/organizations/${orgSlug}/sso-group-mappings`}
                className="flex items-center gap-2 text-primary hover:underline"
              >
                <Users className="h-4 w-4" />
                Manage SSO Group Mappings
                <ExternalLink className="h-3 w-3" />
              </Link>
            </CardContent>
          </Card>
        </div>
      )}

      {/* Form Modal */}
      <OrgSsoConfigFormModal
        open={isFormModalOpen}
        onClose={() => setIsFormModalOpen(false)}
        onCreateSubmit={handleCreate}
        onUpdateSubmit={handleUpdate}
        isLoading={createMutation.isPending || updateMutation.isPending}
        editingConfig={config && !hasNoConfig ? config : null}
        teams={teams?.data ?? []}
        orgSlug={orgSlug!}
      />

      {/* Add Domain Modal */}
      <AddDomainModal
        open={isAddDomainModalOpen}
        onClose={() => setIsAddDomainModalOpen(false)}
        orgSlug={orgSlug!}
      />

      {/* Verification Instructions Modal */}
      {viewingDomainId && (
        <VerificationInstructionsModal
          open={!!viewingDomainId}
          onClose={() => setViewingDomainId(null)}
          orgSlug={orgSlug!}
          domainId={viewingDomainId}
        />
      )}
    </div>
  );
}
