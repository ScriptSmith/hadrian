import { zodResolver } from "@hookform/resolvers/zod";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { createColumnHelper, type ColumnDef } from "@tanstack/react-table";
import { useParams, useNavigate, Link } from "react-router-dom";
import {
  Users,
  FolderKanban,
  Key,
  Server,
  DollarSign,
  Plus,
  Trash2,
  Shield,
  BarChart3,
} from "lucide-react";
import { useState } from "react";
import { useForm } from "react-hook-form";
import { z } from "zod";

import {
  organizationGetOptions,
  organizationUpdateMutation,
  projectListOptions,
  orgMemberListOptions,
  apiKeyListByOrgOptions,
  dynamicProviderListByOrgOptions,
  modelPricingListByOrgOptions,
  orgMemberAddMutation,
  orgMemberRemoveMutation,
  userListOptions,
} from "@/api/generated/@tanstack/react-query.gen";
import type {
  User,
  Project,
  ApiKey,
  DynamicProvider,
  DbModelPricing,
} from "@/api/generated/types.gen";
import { Button } from "@/components/Button/Button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/Card/Card";
import { CodeBadge } from "@/components/CodeBadge/CodeBadge";
import { DataTable } from "@/components/DataTable/DataTable";
import { FormField } from "@/components/FormField/FormField";
import { Input } from "@/components/Input/Input";
import { Modal, ModalHeader, ModalContent, ModalFooter } from "@/components/Modal/Modal";
import { Skeleton } from "@/components/Skeleton/Skeleton";
import { useToast } from "@/components/Toast/Toast";
import { useConfirm } from "@/components/ConfirmDialog/ConfirmDialog";
import {
  DetailPageHeader,
  TabNavigation,
  AddMemberModal,
  ApiKeyStatusBadge,
  EnabledStatusBadge,
  type Tab,
} from "@/components/Admin";
import { Badge } from "@/components/Badge/Badge";
import { formatDateTime, formatCurrency } from "@/utils/formatters";
import UsageDashboard from "@/components/UsageDashboard/UsageDashboard";

type TabId = "projects" | "members" | "api-keys" | "providers" | "pricing" | "sso" | "usage";

const tabs: Tab<TabId>[] = [
  { id: "projects", label: "Projects", icon: <FolderKanban className="h-4 w-4" /> },
  { id: "members", label: "Members", icon: <Users className="h-4 w-4" /> },
  { id: "api-keys", label: "API Keys", icon: <Key className="h-4 w-4" /> },
  { id: "providers", label: "Providers", icon: <Server className="h-4 w-4" /> },
  { id: "pricing", label: "Pricing", icon: <DollarSign className="h-4 w-4" /> },
  { id: "usage", label: "Usage", icon: <BarChart3 className="h-4 w-4" /> },
  { id: "sso", label: "SSO", icon: <Shield className="h-4 w-4" /> },
];

const projectColumnHelper = createColumnHelper<Project>();
const userColumnHelper = createColumnHelper<User>();
const apiKeyColumnHelper = createColumnHelper<ApiKey>();
const providerColumnHelper = createColumnHelper<DynamicProvider>();
const pricingColumnHelper = createColumnHelper<DbModelPricing>();

const editOrgSchema = z.object({
  name: z.string().min(1, "Name is required"),
});

type EditOrgForm = z.infer<typeof editOrgSchema>;

export default function OrganizationDetailPage() {
  const { slug } = useParams<{ slug: string }>();
  const navigate = useNavigate();
  const { toast } = useToast();
  const confirm = useConfirm();
  const queryClient = useQueryClient();

  const [activeTab, setActiveTab] = useState<TabId>("projects");
  const [isEditModalOpen, setIsEditModalOpen] = useState(false);
  const [isAddMemberModalOpen, setIsAddMemberModalOpen] = useState(false);

  const editForm = useForm<EditOrgForm>({
    resolver: zodResolver(editOrgSchema),
    defaultValues: { name: "" },
  });

  // Fetch organization details
  const {
    data: org,
    isLoading: orgLoading,
    error: orgError,
  } = useQuery(organizationGetOptions({ path: { slug: slug! } }));

  // Fetch projects
  const { data: projects, isLoading: projectsLoading } = useQuery({
    ...projectListOptions({ path: { org_slug: slug! } }),
    enabled: activeTab === "projects",
  });

  // Fetch members
  const { data: members, isLoading: membersLoading } = useQuery({
    ...orgMemberListOptions({ path: { org_slug: slug! } }),
    enabled: activeTab === "members",
  });

  // Fetch API keys
  const { data: apiKeys, isLoading: apiKeysLoading } = useQuery({
    ...apiKeyListByOrgOptions({ path: { org_slug: slug! } }),
    enabled: activeTab === "api-keys",
  });

  // Fetch providers
  const { data: providers, isLoading: providersLoading } = useQuery({
    ...dynamicProviderListByOrgOptions({ path: { org_slug: slug! } }),
    enabled: activeTab === "providers",
  });

  // Fetch pricing
  const { data: pricing, isLoading: pricingLoading } = useQuery({
    ...modelPricingListByOrgOptions({ path: { org_slug: slug! } }),
    enabled: activeTab === "pricing",
  });

  // Fetch all users for member selection
  const { data: allUsers } = useQuery({
    ...userListOptions(),
    enabled: isAddMemberModalOpen,
  });

  // Update mutation
  const updateMutation = useMutation({
    ...organizationUpdateMutation(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "organizationGet" }] });
      setIsEditModalOpen(false);
      toast({ title: "Organization updated", type: "success" });
    },
    onError: (error) => {
      toast({ title: "Failed to update organization", description: String(error), type: "error" });
    },
  });

  // Add member mutation
  const addMemberMutation = useMutation({
    ...orgMemberAddMutation(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "orgMemberList" }] });
      setIsAddMemberModalOpen(false);
      toast({ title: "Member added", type: "success" });
    },
    onError: (error) => {
      toast({ title: "Failed to add member", description: String(error), type: "error" });
    },
  });

  // Remove member mutation
  const removeMemberMutation = useMutation({
    ...orgMemberRemoveMutation(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "orgMemberList" }] });
      toast({ title: "Member removed", type: "success" });
    },
    onError: (error) => {
      toast({ title: "Failed to remove member", description: String(error), type: "error" });
    },
  });

  const onEditSubmit = (data: EditOrgForm) => {
    updateMutation.mutate({
      path: { slug: slug! },
      body: { name: data.name },
    });
  };

  const handleAddMember = (userId: string) => {
    addMemberMutation.mutate({
      path: { org_slug: slug! },
      body: { user_id: userId },
    });
  };

  const handleRemoveMember = async (userId: string, userName: string) => {
    const confirmed = await confirm({
      title: "Remove Member",
      message: `Are you sure you want to remove "${userName}" from this organization?`,
      confirmLabel: "Remove",
      variant: "destructive",
    });
    if (confirmed) {
      removeMemberMutation.mutate({
        path: { org_slug: slug!, user_id: userId },
      });
    }
  };

  // Column definitions
  const projectColumns = [
    projectColumnHelper.accessor("name", {
      header: "Name",
      cell: (info) => (
        <Link
          to={`/admin/organizations/${slug}/projects/${info.row.original.slug}`}
          className="font-medium text-primary hover:underline"
        >
          {info.getValue()}
        </Link>
      ),
    }),
    projectColumnHelper.accessor("slug", {
      header: "Slug",
      cell: (info) => <CodeBadge>{info.getValue()}</CodeBadge>,
    }),
    projectColumnHelper.accessor("created_at", {
      header: "Created",
      cell: (info) => formatDateTime(info.getValue()),
    }),
  ];

  const memberColumns = [
    userColumnHelper.accessor("name", {
      header: "Name",
      cell: (info) => info.getValue() || "-",
    }),
    userColumnHelper.accessor("email", {
      header: "Email",
      cell: (info) => info.getValue() || "-",
    }),
    userColumnHelper.accessor("external_id", {
      header: "External ID",
      cell: (info) => <CodeBadge>{info.getValue()}</CodeBadge>,
    }),
    userColumnHelper.display({
      id: "actions",
      cell: ({ row }) => (
        <Button
          variant="ghost"
          size="sm"
          className="text-destructive"
          aria-label="Remove member"
          onClick={() =>
            handleRemoveMember(
              row.original.id,
              row.original.name || row.original.email || row.original.external_id
            )
          }
        >
          <Trash2 className="h-4 w-4" />
        </Button>
      ),
    }),
  ];

  const apiKeyColumns = [
    apiKeyColumnHelper.accessor("name", {
      header: "Name",
      cell: (info) => <span className="font-medium">{info.getValue()}</span>,
    }),
    apiKeyColumnHelper.accessor("key_prefix", {
      header: "Key Prefix",
      cell: (info) => <CodeBadge>{info.getValue()}...</CodeBadge>,
    }),
    apiKeyColumnHelper.accessor("revoked_at", {
      header: "Status",
      cell: (info) => (
        <ApiKeyStatusBadge revokedAt={info.getValue()} expiresAt={info.row.original.expires_at} />
      ),
    }),
    apiKeyColumnHelper.accessor("created_at", {
      header: "Created",
      cell: (info) => formatDateTime(info.getValue()),
    }),
  ];

  const providerColumns = [
    providerColumnHelper.accessor("name", {
      header: "Name",
      cell: (info) => <span className="font-medium">{info.getValue()}</span>,
    }),
    providerColumnHelper.accessor("provider_type", {
      header: "Type",
      cell: (info) => <Badge variant="secondary">{info.getValue()}</Badge>,
    }),
    providerColumnHelper.accessor("base_url", {
      header: "Base URL",
      cell: (info) => <CodeBadge>{info.getValue()}</CodeBadge>,
    }),
    providerColumnHelper.accessor("is_enabled", {
      header: "Status",
      cell: (info) => <EnabledStatusBadge isEnabled={info.getValue()} />,
    }),
  ];

  const pricingColumns = [
    pricingColumnHelper.accessor("model", {
      header: "Model",
      cell: (info) => <span className="font-medium">{info.getValue()}</span>,
    }),
    pricingColumnHelper.accessor("provider", {
      header: "Provider",
      cell: (info) => <Badge variant="secondary">{info.getValue()}</Badge>,
    }),
    pricingColumnHelper.accessor("input_per_1m_tokens", {
      header: "Input/1M",
      cell: (info) => formatCurrency(info.getValue() / 1_000_000),
    }),
    pricingColumnHelper.accessor("output_per_1m_tokens", {
      header: "Output/1M",
      cell: (info) => formatCurrency(info.getValue() / 1_000_000),
    }),
    pricingColumnHelper.accessor("source", {
      header: "Source",
      cell: (info) => <Badge variant="outline">{info.getValue()}</Badge>,
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

  if (orgError || !org) {
    return (
      <div className="p-6">
        <div className="text-center py-12 text-destructive">
          Organization not found or failed to load.
          <br />
          <Button variant="ghost" onClick={() => navigate("/admin/organizations")} className="mt-4">
            Back to Organizations
          </Button>
        </div>
      </div>
    );
  }

  // Filter out users that are already members
  const availableUsers =
    allUsers?.data?.filter((user) => !members?.data?.some((member) => member.id === user.id)) || [];

  return (
    <div className="p-6 space-y-6">
      <DetailPageHeader
        title={org.name}
        slug={org.slug}
        createdAt={org.created_at}
        onBack={() => navigate("/admin/organizations")}
        onEdit={() => {
          editForm.reset({ name: org.name });
          setIsEditModalOpen(true);
        }}
      />

      <TabNavigation tabs={tabs} activeTab={activeTab} onTabChange={setActiveTab} />

      {/* Tab Content */}
      <Card role="tabpanel" id={`tabpanel-${activeTab}`} aria-labelledby={`tab-${activeTab}`}>
        <CardHeader className="flex flex-row items-center justify-between">
          <CardTitle>{tabs.find((t) => t.id === activeTab)?.label}</CardTitle>
          {activeTab === "members" && (
            <Button size="sm" onClick={() => setIsAddMemberModalOpen(true)}>
              <Plus className="mr-2 h-4 w-4" />
              Add Member
            </Button>
          )}
          {activeTab === "projects" && (
            <Button size="sm" onClick={() => navigate(`/admin/organizations/${slug}/projects/new`)}>
              <Plus className="mr-2 h-4 w-4" />
              New Project
            </Button>
          )}
        </CardHeader>
        <CardContent>
          {activeTab === "projects" && (
            <DataTable
              columns={projectColumns}
              data={projects?.data || []}
              isLoading={projectsLoading}
              emptyMessage="No projects in this organization."
              searchColumn="name"
              searchPlaceholder="Search projects..."
            />
          )}
          {activeTab === "members" && (
            <DataTable
              columns={memberColumns as ColumnDef<User>[]}
              data={members?.data || []}
              isLoading={membersLoading}
              emptyMessage="No members in this organization."
              searchColumn="name"
              searchPlaceholder="Search members..."
            />
          )}
          {activeTab === "api-keys" && (
            <DataTable
              columns={apiKeyColumns}
              data={apiKeys?.data || []}
              isLoading={apiKeysLoading}
              emptyMessage="No API keys for this organization."
              searchColumn="name"
              searchPlaceholder="Search API keys..."
            />
          )}
          {activeTab === "providers" && (
            <DataTable
              columns={providerColumns as ColumnDef<DynamicProvider>[]}
              data={providers?.data || []}
              isLoading={providersLoading}
              emptyMessage="No dynamic providers for this organization."
              searchColumn="name"
              searchPlaceholder="Search providers..."
            />
          )}
          {activeTab === "pricing" && (
            <DataTable
              columns={pricingColumns as ColumnDef<DbModelPricing>[]}
              data={pricing?.data || []}
              isLoading={pricingLoading}
              emptyMessage="No custom pricing for this organization."
              searchColumn="model"
              searchPlaceholder="Search models..."
            />
          )}
          {activeTab === "usage" && slug && (
            <UsageDashboard scope={{ type: "organization", slug }} />
          )}
          {activeTab === "sso" && (
            <div className="space-y-4">
              <p className="text-sm text-muted-foreground">
                Configure Single Sign-On (SSO) and SCIM provisioning for this organization. Users
                will be able to authenticate via your identity provider and be automatically
                provisioned.
              </p>
              <div className="flex gap-2 flex-wrap">
                <Link to={`/admin/organizations/${slug}/sso-config`}>
                  <Button>
                    <Shield className="mr-2 h-4 w-4" />
                    Manage SSO Configuration
                  </Button>
                </Link>
                <Link to={`/admin/organizations/${slug}/scim-config`}>
                  <Button variant="outline">
                    <Users className="mr-2 h-4 w-4" />
                    SCIM Provisioning
                  </Button>
                </Link>
                <Link to={`/admin/organizations/${slug}/sso-group-mappings`}>
                  <Button variant="outline">
                    <Users className="mr-2 h-4 w-4" />
                    SSO Group Mappings
                  </Button>
                </Link>
                <Link to={`/admin/organizations/${slug}/rbac-policies`}>
                  <Button variant="outline">
                    <Shield className="mr-2 h-4 w-4" />
                    RBAC Policies
                  </Button>
                </Link>
              </div>
            </div>
          )}
        </CardContent>
      </Card>

      {/* Edit Modal */}
      <Modal open={isEditModalOpen} onClose={() => setIsEditModalOpen(false)}>
        <form onSubmit={editForm.handleSubmit(onEditSubmit)}>
          <ModalHeader>Edit Organization</ModalHeader>
          <ModalContent>
            <FormField
              label="Name"
              htmlFor="name"
              required
              error={editForm.formState.errors.name?.message}
            >
              <Input id="name" {...editForm.register("name")} placeholder="Organization Name" />
            </FormField>
          </ModalContent>
          <ModalFooter>
            <Button type="button" variant="ghost" onClick={() => setIsEditModalOpen(false)}>
              Cancel
            </Button>
            <Button type="submit" isLoading={updateMutation.isPending}>
              Save
            </Button>
          </ModalFooter>
        </form>
      </Modal>

      {/* Add Member Modal */}
      <AddMemberModal
        open={isAddMemberModalOpen}
        onClose={() => setIsAddMemberModalOpen(false)}
        onSubmit={handleAddMember}
        availableUsers={availableUsers}
        isLoading={addMemberMutation.isPending}
        emptyMessage="All users are already members of this organization."
      />
    </div>
  );
}
