import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { createColumnHelper } from "@tanstack/react-table";
import { MoreHorizontal, Trash2, Key } from "lucide-react";
import { useState, useEffect } from "react";

import {
  organizationListOptions,
  apiKeyListByOrgOptions,
  apiKeyCreateMutation,
  apiKeyRevokeMutation,
} from "@/api/generated/@tanstack/react-query.gen";
import type { ApiKey, CreateApiKey } from "@/api/generated/types.gen";
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
  ResourceTable,
  OrganizationSelect,
  OwnerBadge,
  ApiKeyStatusBadge,
  ApiKeyFormModal,
  ApiKeyCreatedModal,
} from "@/components/Admin";
import { useCursorPagination } from "@/hooks";
import { formatDateTime, formatCurrency } from "@/utils/formatters";

const columnHelper = createColumnHelper<ApiKey>();

export default function ApiKeysPage() {
  const { toast } = useToast();
  const confirm = useConfirm();
  const queryClient = useQueryClient();
  const [isCreateModalOpen, setIsCreateModalOpen] = useState(false);
  const [createdKey, setCreatedKey] = useState<string | null>(null);
  const [selectedOrg, setSelectedOrg] = useState<string | null>(null);

  const pagination = useCursorPagination({ defaultLimit: 25 });

  // Fetch organizations
  const { data: organizations } = useQuery(organizationListOptions());

  // Fetch API keys for selected org
  const effectiveOrg = selectedOrg || organizations?.data?.[0]?.slug;
  const {
    data: apiKeys,
    isLoading,
    error,
  } = useQuery({
    ...apiKeyListByOrgOptions({
      path: { org_slug: effectiveOrg || "" },
      query: pagination.queryParams,
    }),
    enabled: !!effectiveOrg,
  });

  // Reset pagination when organization changes
  useEffect(() => {
    pagination.actions.goToFirstPage();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedOrg]);

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
      toast({ title: "Failed to create API key", description: String(error), type: "error" });
    },
  });

  const revokeMutation = useMutation({
    ...apiKeyRevokeMutation(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "apiKeyListByOrg" }] });
      toast({ title: "API key revoked", type: "success" });
    },
    onError: (error) => {
      toast({ title: "Failed to revoke API key", description: String(error), type: "error" });
    },
  });

  const handleCreate = () => {
    setIsCreateModalOpen(true);
  };

  const handleCreateSubmit = (data: CreateApiKey) => {
    createMutation.mutate({ body: data });
  };

  const columns = [
    columnHelper.accessor("name", {
      header: "Name",
      cell: (info) => (
        <div className="flex items-center gap-2">
          <Key className="h-4 w-4 text-muted-foreground" />
          <span className="font-medium">{info.getValue()}</span>
        </div>
      ),
    }),
    columnHelper.accessor("key_prefix", {
      header: "Key Prefix",
      cell: (info) => <CodeBadge>{info.getValue()}...</CodeBadge>,
    }),
    columnHelper.accessor("owner", {
      header: "Owner",
      cell: (info) => <OwnerBadge owner={info.getValue()} showId />,
    }),
    columnHelper.accessor("budget_limit_cents", {
      header: "Budget",
      cell: (info) => {
        const limit = info.getValue();
        const period = info.row.original.budget_period;
        if (!limit) return <span className="text-muted-foreground">No limit</span>;
        return (
          <span>
            {formatCurrency(limit / 100)}
            {period && <span className="text-muted-foreground">/{period}</span>}
          </span>
        );
      },
    }),
    columnHelper.accessor("revoked_at", {
      header: "Status",
      cell: (info) => (
        <ApiKeyStatusBadge revokedAt={info.getValue()} expiresAt={info.row.original.expires_at} />
      ),
    }),
    columnHelper.accessor("last_used_at", {
      header: "Last Used",
      cell: (info) => {
        const lastUsed = info.getValue();
        return lastUsed ? (
          formatDateTime(lastUsed)
        ) : (
          <span className="text-muted-foreground">Never</span>
        );
      },
    }),
    columnHelper.accessor("created_at", {
      header: "Created",
      cell: (info) => formatDateTime(info.getValue()),
    }),
    columnHelper.display({
      id: "actions",
      cell: ({ row }) => {
        const isRevoked = !!row.original.revoked_at;
        return (
          <Dropdown>
            <DropdownTrigger aria-label="Actions" variant="ghost" className="h-8 w-8 p-0">
              <MoreHorizontal className="h-4.5 w-4.5" />
            </DropdownTrigger>
            <DropdownContent align="end">
              <DropdownItem
                className="text-destructive"
                disabled={isRevoked}
                onClick={async () => {
                  const confirmed = await confirm({
                    title: "Revoke API Key",
                    message: `Are you sure you want to revoke "${row.original.name}"? This action cannot be undone and the key will no longer work.`,
                    confirmLabel: "Revoke",
                    variant: "destructive",
                  });
                  if (confirmed) {
                    revokeMutation.mutate({ path: { key_id: row.original.id } });
                  }
                }}
              >
                <Trash2 className="mr-2 h-4 w-4" />
                {isRevoked ? "Already Revoked" : "Revoke"}
              </DropdownItem>
            </DropdownContent>
          </Dropdown>
        );
      },
    }),
  ];

  return (
    <div className="p-6">
      <PageHeader
        title="API Keys"
        description="Manage API keys and their permissions"
        actionLabel="New API Key"
        onAction={handleCreate}
      />

      {organizations?.data && (
        <OrganizationSelect
          organizations={organizations.data}
          value={selectedOrg}
          onChange={setSelectedOrg}
          className="mb-4"
        />
      )}

      <ResourceTable
        title="All API Keys"
        columns={columns}
        data={apiKeys?.data || []}
        isLoading={isLoading}
        error={error}
        emptyMessage="No API keys yet. Create one to get started."
        errorMessage="Failed to load API keys. Please try again."
        noDataMessage={
          !effectiveOrg ? "Create an organization first to manage API keys." : undefined
        }
        paginationProps={{
          pagination: apiKeys?.pagination,
          isFirstPage: pagination.info.isFirstPage,
          pageNumber: pagination.info.pageNumber,
          onPrevious: () => pagination.actions.goToPreviousPage(apiKeys!.pagination),
          onNext: () => pagination.actions.goToNextPage(apiKeys!.pagination),
          onFirst: () => pagination.actions.goToFirstPage(),
        }}
      />

      <ApiKeyFormModal
        isOpen={isCreateModalOpen}
        onClose={() => setIsCreateModalOpen(false)}
        onSubmit={handleCreateSubmit}
        isLoading={createMutation.isPending}
        organizations={organizations?.data}
      />

      <ApiKeyCreatedModal apiKey={createdKey} onClose={() => setCreatedKey(null)} />
    </div>
  );
}
