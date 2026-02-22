import { zodResolver } from "@hookform/resolvers/zod";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { createColumnHelper, type ColumnDef } from "@tanstack/react-table";
import { useParams, useNavigate } from "react-router-dom";
import {
  ArrowLeft,
  Users,
  Key,
  Server,
  DollarSign,
  Plus,
  Trash2,
  BarChart3,
  Pencil,
  Wifi,
  WifiOff,
  MoreHorizontal,
  Power,
  PowerOff,
  Calendar,
} from "lucide-react";
import { useState } from "react";
import { useForm } from "react-hook-form";
import { z } from "zod";

import {
  projectGetOptions,
  projectUpdateMutation,
  projectMemberListOptions,
  apiKeyListByProjectOptions,
  dynamicProviderListByProjectOptions,
  modelPricingListByProjectOptions,
  projectMemberAddMutation,
  projectMemberRemoveMutation,
  dynamicProviderCreateMutation,
  dynamicProviderUpdateMutation,
  dynamicProviderDeleteMutation,
  dynamicProviderTestMutation,
  userListOptions,
  teamListOptions,
} from "@/api/generated/@tanstack/react-query.gen";
import type {
  User,
  ApiKey,
  DynamicProvider,
  DbModelPricing,
  CreateDynamicProvider,
  UpdateDynamicProvider,
  ConnectivityTestResponse,
} from "@/api/generated/types.gen";
import { Button } from "@/components/Button/Button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/Card/Card";
import { CodeBadge } from "@/components/CodeBadge/CodeBadge";
import { DataTable } from "@/components/DataTable/DataTable";
import {
  Dropdown,
  DropdownContent,
  DropdownItem,
  DropdownTrigger,
} from "@/components/Dropdown/Dropdown";
import { FormField } from "@/components/FormField/FormField";
import { Input } from "@/components/Input/Input";
import { Modal, ModalHeader, ModalContent, ModalFooter } from "@/components/Modal/Modal";
import { Skeleton } from "@/components/Skeleton/Skeleton";
import { Badge } from "@/components/Badge/Badge";
import { useToast } from "@/components/Toast/Toast";
import { useConfirm } from "@/components/ConfirmDialog/ConfirmDialog";
import {
  DetailPageHeader,
  TabNavigation,
  AddMemberModal,
  ApiKeyStatusBadge,
  EnabledStatusBadge,
  ProviderFormModal,
  TeamSelect,
  type Tab,
} from "@/components/Admin";
import { getProviderTypeLabel, TestResultDisplay } from "@/pages/providers/shared";
import { formatDateTime, formatCurrency } from "@/utils/formatters";
import UsageDashboard from "@/components/UsageDashboard/UsageDashboard";

type TabId = "members" | "api-keys" | "providers" | "pricing" | "usage";

const tabs: Tab<TabId>[] = [
  { id: "members", label: "Members", icon: <Users className="h-4 w-4" /> },
  { id: "api-keys", label: "API Keys", icon: <Key className="h-4 w-4" /> },
  { id: "providers", label: "Providers", icon: <Server className="h-4 w-4" /> },
  { id: "pricing", label: "Pricing", icon: <DollarSign className="h-4 w-4" /> },
  { id: "usage", label: "Usage", icon: <BarChart3 className="h-4 w-4" /> },
];

const userColumnHelper = createColumnHelper<User>();
const apiKeyColumnHelper = createColumnHelper<ApiKey>();
const pricingColumnHelper = createColumnHelper<DbModelPricing>();

function ProjectProviderCard({
  provider,
  onEdit,
  onDelete,
  onTest,
  onToggleEnabled,
  testResult,
  isTesting,
}: {
  provider: DynamicProvider;
  onEdit: (provider: DynamicProvider) => void;
  onDelete: (provider: DynamicProvider) => void;
  onTest: (provider: DynamicProvider) => void;
  onToggleEnabled: (provider: DynamicProvider) => void;
  testResult?: ConnectivityTestResponse | null;
  isTesting: boolean;
}) {
  const config = provider.config as Record<string, unknown> | null | undefined;

  return (
    <Card className="h-full">
      <CardContent className="p-4">
        <div className="flex items-start justify-between gap-2">
          <div className="flex items-center gap-2 min-w-0">
            <Server className="h-5 w-5 text-muted-foreground shrink-0" />
            <h3 className="font-medium truncate">{provider.name}</h3>
          </div>
          <div className="flex items-center gap-2 shrink-0">
            <EnabledStatusBadge isEnabled={provider.is_enabled} />
            <Dropdown>
              <DropdownTrigger
                aria-label="Provider actions"
                variant="ghost"
                className="h-8 w-8 p-0"
              >
                <MoreHorizontal className="h-4.5 w-4.5" />
              </DropdownTrigger>
              <DropdownContent align="end">
                <DropdownItem onClick={() => onEdit(provider)}>
                  <Pencil className="mr-2 h-4 w-4" />
                  Edit
                </DropdownItem>
                <DropdownItem onClick={() => onTest(provider)}>
                  <Wifi className="mr-2 h-4 w-4" />
                  Test Connection
                </DropdownItem>
                <DropdownItem onClick={() => onToggleEnabled(provider)}>
                  {provider.is_enabled ? (
                    <>
                      <PowerOff className="mr-2 h-4 w-4" />
                      Disable
                    </>
                  ) : (
                    <>
                      <Power className="mr-2 h-4 w-4" />
                      Enable
                    </>
                  )}
                </DropdownItem>
                <DropdownItem className="text-destructive" onClick={() => onDelete(provider)}>
                  <Trash2 className="mr-2 h-4 w-4" />
                  Delete
                </DropdownItem>
              </DropdownContent>
            </Dropdown>
          </div>
        </div>

        <div className="mt-2 flex items-center gap-2 flex-wrap">
          <Badge variant="outline">{getProviderTypeLabel(provider.provider_type)}</Badge>
          {config?.region ? (
            <CodeBadge className="text-xs">{String(config.region)}</CodeBadge>
          ) : null}
        </div>

        {provider.models.length > 0 && (
          <div className="mt-2 flex flex-wrap items-center gap-1.5">
            {provider.models.slice(0, 5).map((model) => (
              <Badge key={model} variant="secondary" className="text-xs">
                {model}
              </Badge>
            ))}
            {provider.models.length > 5 && (
              <Badge variant="secondary" className="text-xs">
                +{provider.models.length - 5} more
              </Badge>
            )}
          </div>
        )}

        <TestResultDisplay isTesting={isTesting} testResult={testResult} />

        <div className="mt-3 flex flex-wrap items-center gap-3 text-xs text-muted-foreground">
          {provider.base_url && (
            <span className="truncate max-w-[200px]" title={provider.base_url}>
              {provider.base_url}
            </span>
          )}
          <span className="flex items-center gap-1">
            <Calendar className="h-3 w-3" />
            {formatDateTime(provider.created_at)}
          </span>
        </div>
      </CardContent>
    </Card>
  );
}

function ProviderCardGrid({
  providers,
  isLoading,
  search,
  onSearchChange,
  onEdit,
  onDelete,
  onTest,
  onToggleEnabled,
  onCreate,
  testResults,
  testingIds,
}: {
  providers: DynamicProvider[];
  isLoading: boolean;
  search: string;
  onSearchChange: (v: string) => void;
  onEdit: (p: DynamicProvider) => void;
  onDelete: (p: DynamicProvider) => void;
  onTest: (p: DynamicProvider) => void;
  onToggleEnabled: (p: DynamicProvider) => void;
  onCreate: () => void;
  testResults: Record<string, ConnectivityTestResponse>;
  testingIds: Set<string>;
}) {
  const filtered = providers.filter(
    (p) =>
      p.name.toLowerCase().includes(search.toLowerCase()) ||
      p.provider_type.toLowerCase().includes(search.toLowerCase())
  );

  if (isLoading) {
    return (
      <div className="grid gap-4 sm:grid-cols-2">
        {Array.from({ length: 3 }).map((_, i) => (
          <Card key={i}>
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
              </div>
              <div className="mt-3 flex gap-3">
                <Skeleton className="h-3 w-32" />
                <Skeleton className="h-3 w-24" />
              </div>
            </CardContent>
          </Card>
        ))}
      </div>
    );
  }

  if (providers.length === 0) {
    return (
      <div className="text-center py-8">
        <WifiOff className="h-10 w-10 text-muted-foreground mx-auto mb-3" />
        <h3 className="text-base font-medium mb-1">No providers</h3>
        <p className="text-sm text-muted-foreground mb-4">
          Add a dynamic provider to connect custom LLM endpoints to this project.
        </p>
        <Button size="sm" onClick={onCreate}>
          <Plus className="h-4 w-4 mr-2" />
          New Provider
        </Button>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      {providers.length > 3 && (
        <Input
          placeholder="Search providers..."
          value={search}
          onChange={(e) => onSearchChange(e.target.value)}
          className="max-w-sm"
        />
      )}

      {filtered.length === 0 ? (
        <div className="text-center py-8 text-sm text-muted-foreground">
          No matching providers.{" "}
          <button onClick={() => onSearchChange("")} className="text-primary hover:underline">
            Clear search
          </button>
        </div>
      ) : (
        <div className="grid gap-4 sm:grid-cols-2">
          {filtered.map((provider) => (
            <ProjectProviderCard
              key={provider.id}
              provider={provider}
              onEdit={onEdit}
              onDelete={onDelete}
              onTest={onTest}
              onToggleEnabled={onToggleEnabled}
              testResult={testResults[provider.id]}
              isTesting={testingIds.has(provider.id)}
            />
          ))}
        </div>
      )}
    </div>
  );
}

const editProjectSchema = z.object({
  name: z.string().min(1, "Name is required"),
  team_id: z.string().nullable().optional(),
});

type EditProjectForm = z.infer<typeof editProjectSchema>;

export default function ProjectDetailPage() {
  const { orgSlug, projectSlug } = useParams<{ orgSlug: string; projectSlug: string }>();
  const navigate = useNavigate();
  const { toast } = useToast();
  const queryClient = useQueryClient();
  const confirm = useConfirm();

  const [activeTab, setActiveTab] = useState<TabId>("members");
  const [isEditModalOpen, setIsEditModalOpen] = useState(false);
  const [isAddMemberModalOpen, setIsAddMemberModalOpen] = useState(false);
  const [isProviderModalOpen, setIsProviderModalOpen] = useState(false);
  const [editingProvider, setEditingProvider] = useState<DynamicProvider | null>(null);
  const [providerSearch, setProviderSearch] = useState("");
  const [testResults, setTestResults] = useState<Record<string, ConnectivityTestResponse>>({});
  const [testingIds, setTestingIds] = useState<Set<string>>(new Set());

  const editForm = useForm<EditProjectForm>({
    resolver: zodResolver(editProjectSchema),
    defaultValues: { name: "", team_id: null },
  });

  // Fetch project details
  const {
    data: project,
    isLoading: projectLoading,
    error: projectError,
  } = useQuery(projectGetOptions({ path: { org_slug: orgSlug!, project_slug: projectSlug! } }));

  // Fetch members
  const { data: members, isLoading: membersLoading } = useQuery({
    ...projectMemberListOptions({
      path: { org_slug: orgSlug!, project_slug: projectSlug! },
    }),
    enabled: activeTab === "members",
  });

  // Fetch API keys
  const { data: apiKeys, isLoading: apiKeysLoading } = useQuery({
    ...apiKeyListByProjectOptions({
      path: { org_slug: orgSlug!, project_slug: projectSlug! },
    }),
    enabled: activeTab === "api-keys",
  });

  // Fetch providers
  const { data: providers, isLoading: providersLoading } = useQuery({
    ...dynamicProviderListByProjectOptions({
      path: { org_slug: orgSlug!, project_slug: projectSlug! },
    }),
    enabled: activeTab === "providers",
  });

  // Fetch pricing
  const { data: pricing, isLoading: pricingLoading } = useQuery({
    ...modelPricingListByProjectOptions({
      path: { org_slug: orgSlug!, project_slug: projectSlug! },
    }),
    enabled: activeTab === "pricing",
  });

  // Fetch all users for member selection
  const { data: allUsers } = useQuery({
    ...userListOptions(),
    enabled: isAddMemberModalOpen,
  });

  // Fetch teams for display and team selection in edit modal
  const { data: teams } = useQuery({
    ...teamListOptions({ path: { org_slug: orgSlug || "" } }),
    enabled: !!orgSlug && (!!project?.team_id || isEditModalOpen),
  });

  // Update mutation
  const updateMutation = useMutation({
    ...projectUpdateMutation(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "projectGet" }] });
      setIsEditModalOpen(false);
      toast({ title: "Project updated", type: "success" });
    },
    onError: (error) => {
      toast({ title: "Failed to update project", description: String(error), type: "error" });
    },
  });

  // Add member mutation
  const addMemberMutation = useMutation({
    ...projectMemberAddMutation(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "projectMemberList" }] });
      setIsAddMemberModalOpen(false);
      toast({ title: "Member added", type: "success" });
    },
    onError: (error) => {
      toast({ title: "Failed to add member", description: String(error), type: "error" });
    },
  });

  // Remove member mutation
  const removeMemberMutation = useMutation({
    ...projectMemberRemoveMutation(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "projectMemberList" }] });
      toast({ title: "Member removed", type: "success" });
    },
    onError: (error) => {
      toast({ title: "Failed to remove member", description: String(error), type: "error" });
    },
  });

  // Provider mutations
  const providerCreateMutation = useMutation({
    ...dynamicProviderCreateMutation(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "dynamicProviderListByProject" }] });
      setIsProviderModalOpen(false);
      toast({ title: "Provider created", type: "success" });
    },
    onError: (error) => {
      toast({ title: "Failed to create provider", description: String(error), type: "error" });
    },
  });

  const providerUpdateMutation = useMutation({
    ...dynamicProviderUpdateMutation(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "dynamicProviderListByProject" }] });
      setIsProviderModalOpen(false);
      setEditingProvider(null);
      toast({ title: "Provider updated", type: "success" });
    },
    onError: (error) => {
      toast({ title: "Failed to update provider", description: String(error), type: "error" });
    },
  });

  const providerDeleteMutation = useMutation({
    ...dynamicProviderDeleteMutation(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "dynamicProviderListByProject" }] });
      toast({ title: "Provider deleted", type: "success" });
    },
    onError: (error) => {
      toast({ title: "Failed to delete provider", description: String(error), type: "error" });
    },
  });

  const providerTestMutation = useMutation({
    ...dynamicProviderTestMutation(),
    onSuccess: (data, variables) => {
      const id = variables.path.id;
      setTestResults((prev) => ({ ...prev, [id]: data }));
      setTestingIds((prev) => {
        const next = new Set(prev);
        next.delete(id);
        return next;
      });
    },
    onError: (error, variables) => {
      const id = variables.path.id;
      setTestResults((prev) => ({
        ...prev,
        [id]: { status: "error", message: String(error), latency_ms: null },
      }));
      setTestingIds((prev) => {
        const next = new Set(prev);
        next.delete(id);
        return next;
      });
    },
  });

  const handleProviderCreate = () => {
    setEditingProvider(null);
    setIsProviderModalOpen(true);
  };

  const handleProviderEdit = (provider: DynamicProvider) => {
    setEditingProvider(provider);
    setIsProviderModalOpen(true);
  };

  const handleProviderDelete = async (provider: DynamicProvider) => {
    const confirmed = await confirm({
      title: "Delete Provider",
      message: `Are you sure you want to delete "${provider.name}"? This action cannot be undone.`,
      confirmLabel: "Delete",
      variant: "destructive",
    });
    if (confirmed) {
      providerDeleteMutation.mutate({ path: { id: provider.id } });
    }
  };

  const handleProviderToggleEnabled = (provider: DynamicProvider) => {
    providerUpdateMutation.mutate({
      path: { id: provider.id },
      body: { is_enabled: !provider.is_enabled },
    });
  };

  const handleProviderTest = (provider: DynamicProvider) => {
    setTestingIds((prev) => new Set(prev).add(provider.id));
    setTestResults((prev) => {
      const next = { ...prev };
      delete next[provider.id];
      return next;
    });
    providerTestMutation.mutate({ path: { id: provider.id } });
  };

  const onEditSubmit = (data: EditProjectForm) => {
    updateMutation.mutate({
      path: { org_slug: orgSlug!, project_slug: projectSlug! },
      body: { name: data.name, team_id: data.team_id },
    });
  };

  const handleAddMember = (userId: string) => {
    addMemberMutation.mutate({
      path: { org_slug: orgSlug!, project_slug: projectSlug! },
      body: { user_id: userId },
    });
  };

  const handleRemoveMember = async (userId: string, userName?: string) => {
    const confirmed = await confirm({
      title: "Remove Member",
      message: `Are you sure you want to remove ${userName || "this member"} from the project?`,
      confirmLabel: "Remove",
      variant: "destructive",
    });
    if (confirmed) {
      removeMemberMutation.mutate({
        path: { org_slug: orgSlug!, project_slug: projectSlug!, user_id: userId },
      });
    }
  };

  // Column definitions
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

  if (projectLoading) {
    return (
      <div className="p-6 space-y-6">
        <Skeleton className="h-8 w-64" />
        <Skeleton className="h-32 w-full" />
      </div>
    );
  }

  if (projectError || !project) {
    return (
      <div className="p-6">
        <div className="text-center py-12 text-destructive">
          Project not found or failed to load.
          <br />
          <Button
            variant="ghost"
            onClick={() => navigate(`/admin/organizations/${orgSlug}`)}
            className="mt-4"
          >
            <ArrowLeft className="mr-2 h-4 w-4" />
            Back to Organization
          </Button>
        </div>
      </div>
    );
  }

  // Filter out users that are already members
  const availableUsers = allUsers?.data?.filter(
    (user) => !members?.data?.some((member) => member.id === user.id)
  );

  return (
    <div className="p-6 space-y-6">
      <DetailPageHeader
        title={project.name}
        slug={project.slug}
        createdAt={project.created_at}
        onBack={() => navigate(`/admin/organizations/${orgSlug}`)}
        onEdit={() => {
          editForm.reset({ name: project.name, team_id: project.team_id ?? null });
          setIsEditModalOpen(true);
        }}
      />

      {project.team_id && teams?.data && (
        <div className="flex items-center gap-2 text-sm text-muted-foreground">
          <span>Team:</span>
          <Badge variant="secondary">
            {teams.data.find((t) => t.id === project.team_id)?.name ?? "Unknown Team"}
          </Badge>
        </div>
      )}

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
          {activeTab === "providers" && (
            <Button size="sm" onClick={handleProviderCreate}>
              <Plus className="mr-2 h-4 w-4" />
              New Provider
            </Button>
          )}
        </CardHeader>
        <CardContent>
          {activeTab === "members" && (
            <DataTable
              columns={memberColumns as ColumnDef<User>[]}
              data={members?.data || []}
              isLoading={membersLoading}
              emptyMessage="No members in this project."
              searchColumn="name"
              searchPlaceholder="Search members..."
            />
          )}
          {activeTab === "api-keys" && (
            <DataTable
              columns={apiKeyColumns as ColumnDef<ApiKey>[]}
              data={apiKeys?.data || []}
              isLoading={apiKeysLoading}
              emptyMessage="No API keys for this project."
              searchColumn="name"
              searchPlaceholder="Search API keys..."
            />
          )}
          {activeTab === "providers" && (
            <ProviderCardGrid
              providers={providers?.data ?? []}
              isLoading={providersLoading}
              search={providerSearch}
              onSearchChange={setProviderSearch}
              onEdit={handleProviderEdit}
              onDelete={handleProviderDelete}
              onTest={handleProviderTest}
              onToggleEnabled={handleProviderToggleEnabled}
              onCreate={handleProviderCreate}
              testResults={testResults}
              testingIds={testingIds}
            />
          )}
          {activeTab === "pricing" && (
            <DataTable
              columns={pricingColumns as ColumnDef<DbModelPricing>[]}
              data={pricing?.data || []}
              isLoading={pricingLoading}
              emptyMessage="No custom pricing for this project."
              searchColumn="model"
              searchPlaceholder="Search models..."
            />
          )}
          {activeTab === "usage" && orgSlug && projectSlug && (
            <UsageDashboard scope={{ type: "project", orgSlug, projectSlug }} />
          )}
        </CardContent>
      </Card>

      {/* Edit Modal */}
      <Modal open={isEditModalOpen} onClose={() => setIsEditModalOpen(false)}>
        <form onSubmit={editForm.handleSubmit(onEditSubmit)}>
          <ModalHeader>Edit Project</ModalHeader>
          <ModalContent>
            <div className="space-y-4">
              <FormField
                label="Name"
                htmlFor="name"
                required
                error={editForm.formState.errors.name?.message}
              >
                <Input id="name" {...editForm.register("name")} placeholder="Project Name" />
              </FormField>
              {teams?.data && (
                <TeamSelect
                  teams={teams.data}
                  value={editForm.watch("team_id") ?? null}
                  onChange={(teamId) => editForm.setValue("team_id", teamId)}
                  label="Team"
                  nonePlaceholder="None (Organization-level)"
                />
              )}
            </div>
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
        availableUsers={availableUsers || []}
        isLoading={addMemberMutation.isPending}
        emptyMessage="All users are already members of this project."
      />

      {/* Provider Form Modal */}
      <ProviderFormModal
        isOpen={isProviderModalOpen}
        onClose={() => {
          setIsProviderModalOpen(false);
          setEditingProvider(null);
        }}
        onCreateSubmit={(data: CreateDynamicProvider) => {
          providerCreateMutation.mutate({ body: data });
        }}
        onEditSubmit={(data: UpdateDynamicProvider) => {
          if (!editingProvider) return;
          providerUpdateMutation.mutate({ path: { id: editingProvider.id }, body: data });
        }}
        isLoading={providerCreateMutation.isPending || providerUpdateMutation.isPending}
        editingProvider={editingProvider}
        ownerOverride={{ type: "project", project_id: project.id }}
      />
    </div>
  );
}
