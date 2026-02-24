import { useState, useEffect } from "react";
import {
  Plus,
  Server,
  Calendar,
  Pencil,
  Trash2,
  Wifi,
  WifiOff,
  Settings2,
  MoreHorizontal,
  Power,
  PowerOff,
} from "lucide-react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";

import {
  organizationListOptions,
  dynamicProviderListByOrgOptions,
  dynamicProviderCreateMutation,
  dynamicProviderDeleteMutation,
  dynamicProviderUpdateMutation,
  dynamicProviderTestMutation,
  meBuiltInProvidersListOptions,
} from "@/api/generated/@tanstack/react-query.gen";
import type {
  DynamicProvider,
  CreateDynamicProvider,
  UpdateDynamicProvider,
  ConnectivityTestResponse,
  BuiltInProvider,
} from "@/api/generated/types.gen";
import { Button } from "@/components/Button/Button";
import { Badge } from "@/components/Badge/Badge";
import { Card, CardContent } from "@/components/Card/Card";
import { Input } from "@/components/Input/Input";
import { Skeleton } from "@/components/Skeleton/Skeleton";
import { CodeBadge } from "@/components/CodeBadge/CodeBadge";
import {
  Dropdown,
  DropdownContent,
  DropdownItem,
  DropdownTrigger,
} from "@/components/Dropdown/Dropdown";
import { useToast } from "@/components/Toast/Toast";
import { useConfirm } from "@/components/ConfirmDialog/ConfirmDialog";
import {
  PageHeader,
  OrganizationSelect,
  OwnerBadge,
  EnabledStatusBadge,
  ProviderFormModal,
} from "@/components/Admin";
import { getProviderTypeLabel, TestResultDisplay } from "@/pages/providers/shared";
import { formatDateTime } from "@/utils/formatters";

// -- Provider Card --

function ProviderCard({
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
            <p className="font-medium truncate">{provider.name}</p>
          </div>
          <div className="flex items-center gap-2 shrink-0">
            <EnabledStatusBadge isEnabled={provider.is_enabled} />
            <Dropdown>
              <DropdownTrigger aria-label="Actions" variant="ghost" className="h-8 w-8 p-0">
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
          <OwnerBadge owner={provider.owner} />
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

function ProviderCardSkeleton() {
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
          <Skeleton className="h-3 w-32" />
          <Skeleton className="h-3 w-24" />
        </div>
      </CardContent>
    </Card>
  );
}

// -- Built-in Provider Card --

function BuiltInProviderCard({ provider }: { provider: BuiltInProvider }) {
  return (
    <Card className="h-full">
      <CardContent className="p-4">
        <div className="flex items-start justify-between gap-2">
          <div className="flex items-center gap-2 min-w-0">
            <Settings2 className="h-5 w-5 text-muted-foreground shrink-0" />
            <p className="font-medium truncate">{provider.name}</p>
          </div>
          <Badge variant="outline" className="text-xs shrink-0">
            Built-in
          </Badge>
        </div>
        <div className="mt-2">
          <Badge variant="outline">{getProviderTypeLabel(provider.provider_type)}</Badge>
        </div>
        {provider.base_url && (
          <div className="mt-3 text-xs text-muted-foreground truncate" title={provider.base_url}>
            {provider.base_url}
          </div>
        )}
      </CardContent>
    </Card>
  );
}

// -- Main Page --

export default function ProvidersPage() {
  const [search, setSearch] = useState("");
  const [isModalOpen, setIsModalOpen] = useState(false);
  const [editingProvider, setEditingProvider] = useState<DynamicProvider | null>(null);
  const [selectedOrg, setSelectedOrg] = useState<string | null>(null);
  const [testResults, setTestResults] = useState<Record<string, ConnectivityTestResponse>>({});
  const [testingIds, setTestingIds] = useState<Set<string>>(new Set());
  const { toast } = useToast();
  const confirm = useConfirm();
  const queryClient = useQueryClient();

  // Fetch organizations
  const { data: organizations } = useQuery(organizationListOptions());

  const effectiveOrg = selectedOrg || organizations?.data?.[0]?.slug;

  // Fetch providers for selected org
  const {
    data: providersData,
    isLoading,
    error,
  } = useQuery({
    ...dynamicProviderListByOrgOptions({
      path: { org_slug: effectiveOrg || "" },
      query: { limit: 100 },
    }),
    enabled: !!effectiveOrg,
  });

  // Fetch built-in providers
  const { data: builtInData } = useQuery(meBuiltInProvidersListOptions());

  const providers = providersData?.data ?? [];
  const builtInProviders = builtInData?.data ?? [];

  const filteredProviders = providers.filter(
    (p) =>
      p.name.toLowerCase().includes(search.toLowerCase()) ||
      p.provider_type.toLowerCase().includes(search.toLowerCase())
  );

  // Reset search when org changes
  useEffect(() => {
    setSearch("");
  }, [selectedOrg]);

  const createMutation = useMutation({
    ...dynamicProviderCreateMutation(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "dynamicProviderListByOrg" }] });
      setIsModalOpen(false);
      toast({ title: "Provider created", type: "success" });
    },
    onError: (error) => {
      toast({ title: "Failed to create provider", description: String(error), type: "error" });
    },
  });

  const updateMutation = useMutation({
    ...dynamicProviderUpdateMutation(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "dynamicProviderListByOrg" }] });
      setIsModalOpen(false);
      setEditingProvider(null);
      toast({ title: "Provider updated", type: "success" });
    },
    onError: (error) => {
      toast({ title: "Failed to update provider", description: String(error), type: "error" });
    },
  });

  const deleteMutation = useMutation({
    ...dynamicProviderDeleteMutation(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "dynamicProviderListByOrg" }] });
      toast({ title: "Provider deleted", type: "success" });
    },
    onError: (error) => {
      toast({ title: "Failed to delete provider", description: String(error), type: "error" });
    },
  });

  const testMutation = useMutation({
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

  const handleCreate = () => {
    setEditingProvider(null);
    setIsModalOpen(true);
  };

  const handleEdit = (provider: DynamicProvider) => {
    setEditingProvider(provider);
    setIsModalOpen(true);
  };

  const handleClose = () => {
    setIsModalOpen(false);
    setEditingProvider(null);
  };

  const handleCreateSubmit = (data: CreateDynamicProvider) => {
    createMutation.mutate({ body: data });
  };

  const handleEditSubmit = (data: UpdateDynamicProvider) => {
    if (!editingProvider) return;
    updateMutation.mutate({ path: { id: editingProvider.id }, body: data });
  };

  const handleDelete = async (provider: DynamicProvider) => {
    const confirmed = await confirm({
      title: "Delete Provider",
      message: `Are you sure you want to delete "${provider.name}"? This action cannot be undone.`,
      confirmLabel: "Delete",
      variant: "destructive",
    });
    if (confirmed) {
      deleteMutation.mutate({ path: { id: provider.id } });
    }
  };

  const handleToggleEnabled = (provider: DynamicProvider) => {
    updateMutation.mutate({
      path: { id: provider.id },
      body: { is_enabled: !provider.is_enabled },
    });
  };

  const handleTest = (provider: DynamicProvider) => {
    setTestingIds((prev) => new Set(prev).add(provider.id));
    setTestResults((prev) => {
      const next = { ...prev };
      delete next[provider.id];
      return next;
    });
    testMutation.mutate({ path: { id: provider.id } });
  };

  const enabledCount = providers.filter((p) => p.is_enabled).length;
  const disabledCount = providers.filter((p) => !p.is_enabled).length;

  return (
    <div className="p-6">
      <PageHeader
        title="Providers"
        description="Manage built-in and dynamic LLM providers"
        actionLabel="New Provider"
        onAction={handleCreate}
      />

      {organizations?.data && (
        <OrganizationSelect
          organizations={organizations.data}
          value={selectedOrg}
          onChange={setSelectedOrg}
          className="mb-6"
        />
      )}

      {/* Built-in providers section */}
      {builtInProviders.length > 0 && (
        <div className="mb-8">
          <h2 className="text-lg font-medium mb-3">Built-in Providers</h2>
          <p className="text-sm text-muted-foreground mb-4">
            Configured in the gateway deployment. These are available to all users.
          </p>
          <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
            {builtInProviders.map((provider) => (
              <BuiltInProviderCard key={provider.name} provider={provider} />
            ))}
          </div>
        </div>
      )}

      {/* Dynamic providers section */}
      <div>
        <h2 className="text-lg font-medium mb-3">Dynamic Providers</h2>

        {/* Stats */}
        {!isLoading && providers.length > 0 && (
          <div className="flex items-center gap-4 mb-4">
            <Badge variant="secondary">{enabledCount} enabled</Badge>
            {disabledCount > 0 && <Badge variant="outline">{disabledCount} disabled</Badge>}
          </div>
        )}

        {/* Search */}
        {providers.length > 0 && (
          <div className="mb-4">
            <Input
              placeholder="Search providers..."
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              className="max-w-sm"
            />
          </div>
        )}

        {/* Error state */}
        {error && (
          <div className="rounded-md bg-destructive/10 px-4 py-3 text-sm text-destructive mb-4">
            Failed to load providers. Please try again.
          </div>
        )}

        {/* Loading state */}
        {isLoading && (
          <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
            {Array.from({ length: 3 }).map((_, i) => (
              <ProviderCardSkeleton key={i} />
            ))}
          </div>
        )}

        {/* Empty state */}
        {!isLoading && !error && effectiveOrg && providers.length === 0 && (
          <div className="text-center py-12">
            <WifiOff className="h-12 w-12 text-muted-foreground mx-auto mb-4" />
            <h2 className="text-lg font-medium mb-2">No dynamic providers</h2>
            <p className="text-sm text-muted-foreground max-w-md mx-auto mb-4">
              Create a dynamic provider to add custom LLM endpoints for this organization.
            </p>
            <Button onClick={handleCreate}>
              <Plus className="h-4 w-4 mr-2" />
              New Provider
            </Button>
          </div>
        )}

        {/* No org selected */}
        {!effectiveOrg && !isLoading && (
          <div className="text-center py-12">
            <Server className="h-12 w-12 text-muted-foreground mx-auto mb-4" />
            <h2 className="text-lg font-medium mb-2">No organization selected</h2>
            <p className="text-sm text-muted-foreground">
              Create an organization first to manage providers.
            </p>
          </div>
        )}

        {/* Empty search results */}
        {!isLoading && providers.length > 0 && filteredProviders.length === 0 && (
          <div className="text-center py-12">
            <Server className="h-12 w-12 text-muted-foreground mx-auto mb-4" />
            <h2 className="text-lg font-medium mb-2">No matching providers</h2>
            <p className="text-sm text-muted-foreground">
              Try adjusting your search terms or{" "}
              <button onClick={() => setSearch("")} className="text-primary hover:underline">
                clear the search
              </button>
            </p>
          </div>
        )}

        {/* Provider cards grid */}
        {!isLoading && filteredProviders.length > 0 && (
          <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
            {filteredProviders.map((provider) => (
              <ProviderCard
                key={provider.id}
                provider={provider}
                onEdit={handleEdit}
                onDelete={handleDelete}
                onTest={handleTest}
                onToggleEnabled={handleToggleEnabled}
                testResult={testResults[provider.id]}
                isTesting={testingIds.has(provider.id)}
              />
            ))}
          </div>
        )}
      </div>

      <ProviderFormModal
        isOpen={isModalOpen}
        onClose={handleClose}
        onCreateSubmit={handleCreateSubmit}
        onEditSubmit={handleEditSubmit}
        isLoading={createMutation.isPending || updateMutation.isPending}
        editingProvider={editingProvider}
        organizations={organizations?.data}
      />
    </div>
  );
}
