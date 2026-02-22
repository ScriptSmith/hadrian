import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { createColumnHelper, type ColumnDef } from "@tanstack/react-table";
import { useParams, useNavigate } from "react-router-dom";
import { ArrowLeft, Shield, Plus, Pencil, Trash2, History, Info } from "lucide-react";
import { useState } from "react";

import {
  orgRbacPolicyListOptions,
  orgRbacPolicyCreateMutation,
  orgRbacPolicyUpdateMutation,
  orgRbacPolicyDeleteMutation,
  organizationGetOptions,
} from "@/api/generated/@tanstack/react-query.gen";
import type {
  OrgRbacPolicy,
  CreateOrgRbacPolicy,
  UpdateOrgRbacPolicy,
} from "@/api/generated/types.gen";
import { Button } from "@/components/Button/Button";
import { Badge } from "@/components/Badge/Badge";
import { DataTable } from "@/components/DataTable/DataTable";
import { Skeleton } from "@/components/Skeleton/Skeleton";
import { Switch } from "@/components/Switch/Switch";
import { useToast } from "@/components/Toast/Toast";
import { useConfirm } from "@/components/ConfirmDialog/ConfirmDialog";
import {
  RbacPolicyFormModal,
  RbacPolicyVersionHistoryModal,
  RbacPolicySimulator,
} from "@/components/RbacPolicy";
import { formatDateTime } from "@/utils/formatters";

const columnHelper = createColumnHelper<OrgRbacPolicy>();

export default function OrgRbacPoliciesPage() {
  const { orgSlug } = useParams<{ orgSlug: string }>();
  const navigate = useNavigate();
  const { toast } = useToast();
  const confirm = useConfirm();
  const queryClient = useQueryClient();

  const [isFormModalOpen, setIsFormModalOpen] = useState(false);
  const [editingPolicy, setEditingPolicy] = useState<OrgRbacPolicy | null>(null);
  const [historyPolicy, setHistoryPolicy] = useState<OrgRbacPolicy | null>(null);

  // Fetch organization details
  const { data: org, isLoading: orgLoading } = useQuery(
    organizationGetOptions({ path: { slug: orgSlug! } })
  );

  // Fetch RBAC policies
  const { data: policies, isLoading: policiesLoading } = useQuery(
    orgRbacPolicyListOptions({ path: { org_slug: orgSlug! } })
  );

  // Create mutation
  const createMutation = useMutation({
    ...orgRbacPolicyCreateMutation(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "orgRbacPolicyList" }] });
      setIsFormModalOpen(false);
      toast({ title: "RBAC policy created", type: "success" });
    },
    onError: (error) => {
      toast({
        title: "Failed to create policy",
        description: String(error),
        type: "error",
      });
    },
  });

  // Update mutation
  const updateMutation = useMutation({
    ...orgRbacPolicyUpdateMutation(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "orgRbacPolicyList" }] });
      setIsFormModalOpen(false);
      setEditingPolicy(null);
      toast({ title: "RBAC policy updated", type: "success" });
    },
    onError: (error) => {
      toast({
        title: "Failed to update policy",
        description: String(error),
        type: "error",
      });
    },
  });

  // Delete mutation
  const deleteMutation = useMutation({
    ...orgRbacPolicyDeleteMutation(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "orgRbacPolicyList" }] });
      toast({ title: "RBAC policy deleted", type: "success" });
    },
    onError: (error) => {
      toast({
        title: "Failed to delete policy",
        description: String(error),
        type: "error",
      });
    },
  });

  // Toggle enabled mutation
  const toggleEnabledMutation = useMutation({
    ...orgRbacPolicyUpdateMutation(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "orgRbacPolicyList" }] });
    },
    onError: (error) => {
      toast({
        title: "Failed to update policy",
        description: String(error),
        type: "error",
      });
    },
  });

  const handleCreate = (data: CreateOrgRbacPolicy) => {
    createMutation.mutate({
      path: { org_slug: orgSlug! },
      body: data,
    });
  };

  const handleUpdate = (data: UpdateOrgRbacPolicy) => {
    if (!editingPolicy) return;
    updateMutation.mutate({
      path: { org_slug: orgSlug!, policy_id: editingPolicy.id },
      body: data,
    });
  };

  const handleDelete = async (policy: OrgRbacPolicy) => {
    const confirmed = await confirm({
      title: "Delete RBAC Policy",
      message: `Are you sure you want to delete "${policy.name}"? This action cannot be undone.`,
      confirmLabel: "Delete",
      variant: "destructive",
    });
    if (confirmed) {
      deleteMutation.mutate({
        path: { org_slug: orgSlug!, policy_id: policy.id },
      });
    }
  };

  const handleToggleEnabled = (policy: OrgRbacPolicy) => {
    toggleEnabledMutation.mutate({
      path: { org_slug: orgSlug!, policy_id: policy.id },
      body: { enabled: !policy.enabled },
    });
  };

  const handleEdit = (policy: OrgRbacPolicy) => {
    setEditingPolicy(policy);
    setIsFormModalOpen(true);
  };

  const handleCloseForm = () => {
    setIsFormModalOpen(false);
    setEditingPolicy(null);
  };

  const columns = [
    columnHelper.accessor("name", {
      header: "Name",
      cell: (info) => (
        <div>
          <span className="font-medium">{info.getValue()}</span>
          {info.row.original.description && (
            <p className="text-xs text-muted-foreground truncate max-w-xs">
              {info.row.original.description}
            </p>
          )}
        </div>
      ),
    }),
    columnHelper.accessor("resource", {
      header: "Resource",
      cell: (info) => (
        <code className="text-xs bg-muted px-1.5 py-0.5 rounded">{info.getValue()}</code>
      ),
    }),
    columnHelper.accessor("action", {
      header: "Action",
      cell: (info) => (
        <code className="text-xs bg-muted px-1.5 py-0.5 rounded">{info.getValue()}</code>
      ),
    }),
    columnHelper.accessor("effect", {
      header: "Effect",
      cell: (info) => (
        <Badge variant={info.getValue() === "allow" ? "default" : "destructive"}>
          {info.getValue()}
        </Badge>
      ),
    }),
    columnHelper.accessor("priority", {
      header: "Priority",
      cell: (info) => <span className="text-sm">{info.getValue()}</span>,
    }),
    columnHelper.accessor("enabled", {
      header: "Enabled",
      cell: (info) => (
        <Switch
          checked={info.getValue()}
          onChange={() => handleToggleEnabled(info.row.original)}
          disabled={toggleEnabledMutation.isPending}
          aria-label={`Toggle "${info.row.original.name}" policy`}
        />
      ),
    }),
    columnHelper.accessor("version", {
      header: "Version",
      cell: (info) => <span className="text-sm text-muted-foreground">v{info.getValue()}</span>,
    }),
    columnHelper.accessor("updated_at", {
      header: "Updated",
      cell: (info) => (
        <span className="text-sm text-muted-foreground">{formatDateTime(info.getValue())}</span>
      ),
    }),
    columnHelper.display({
      id: "actions",
      header: () => <span className="sr-only">Actions</span>,
      cell: ({ row }) => (
        <div className="flex items-center gap-1">
          <Button
            variant="ghost"
            size="sm"
            onClick={() => setHistoryPolicy(row.original)}
            title="View version history"
          >
            <History className="h-4 w-4" />
          </Button>
          <Button
            variant="ghost"
            size="sm"
            onClick={() => handleEdit(row.original)}
            title="Edit policy"
          >
            <Pencil className="h-4 w-4" />
          </Button>
          <Button
            variant="ghost"
            size="sm"
            className="text-destructive"
            onClick={() => handleDelete(row.original)}
            title="Delete policy"
          >
            <Trash2 className="h-4 w-4" />
          </Button>
        </div>
      ),
    }),
  ];

  if (orgLoading) {
    return (
      <div className="p-6 space-y-6">
        <Skeleton className="h-8 w-64" />
        <Skeleton className="h-32 w-full" />
      </div>
    );
  }

  const policyList = policies?.data ?? [];

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
            RBAC Policies
          </h1>
          <p className="text-muted-foreground mt-1">
            {org?.name && <span>{org.name} • </span>}
            Configure role-based access control policies for this organization
          </p>
        </div>
        <Button onClick={() => setIsFormModalOpen(true)}>
          <Plus className="mr-2 h-4 w-4" />
          Create Policy
        </Button>
      </div>

      {/* Info banner */}
      <div className="flex items-start gap-3 rounded-lg border bg-muted/30 p-4">
        <Info className="h-5 w-5 text-muted-foreground mt-0.5" />
        <div className="text-sm">
          <p className="font-medium">Per-Organization RBAC Policies</p>
          <p className="text-muted-foreground">
            Define custom access control rules using CEL expressions. Policies are evaluated in
            priority order (highest first). The first matching policy determines the access
            decision. If no policy matches, the configured default effect applies.
          </p>
        </div>
      </div>

      {/* Policies Table */}
      <DataTable
        columns={columns as ColumnDef<OrgRbacPolicy>[]}
        data={policyList}
        isLoading={policiesLoading}
        emptyMessage="No RBAC policies configured for this organization."
        searchColumn="name"
        searchPlaceholder="Search policies..."
      />

      {/* Policy Simulator */}
      {policyList.length > 0 && <RbacPolicySimulator orgSlug={orgSlug!} policies={policyList} />}

      {/* Form Modal */}
      <RbacPolicyFormModal
        open={isFormModalOpen}
        onClose={handleCloseForm}
        onCreateSubmit={handleCreate}
        onUpdateSubmit={handleUpdate}
        isLoading={createMutation.isPending || updateMutation.isPending}
        editingPolicy={editingPolicy}
      />

      {/* Version History Modal */}
      {historyPolicy && (
        <RbacPolicyVersionHistoryModal
          open={!!historyPolicy}
          onClose={() => setHistoryPolicy(null)}
          policy={historyPolicy}
          orgSlug={orgSlug!}
        />
      )}
    </div>
  );
}
