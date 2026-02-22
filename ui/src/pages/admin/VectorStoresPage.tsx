import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { createColumnHelper } from "@tanstack/react-table";
import { MoreHorizontal, Pencil, Trash2, Database, FileText } from "lucide-react";
import { useState, useEffect } from "react";
import { Link } from "react-router-dom";

import {
  organizationListOptions,
  vectorStoreListOptions,
  vectorStoreCreateMutation,
  vectorStoreDeleteMutation,
  vectorStoreModifyMutation,
} from "@/api/generated/@tanstack/react-query.gen";
import type { VectorStore, CreateVectorStore, UpdateVectorStore } from "@/api/generated/types.gen";
import { Badge } from "@/components/Badge/Badge";
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
  VectorStoreFormModal,
} from "@/components/Admin";
import { useOpenAIPagination } from "@/hooks";
import { formatDateTime, formatBytes } from "@/utils/formatters";

const columnHelper = createColumnHelper<VectorStore>();

/** Status badge for vector store status */
function VectorStoreStatusBadge({ status }: { status: string }) {
  const variants: Record<string, "default" | "secondary" | "destructive" | "outline"> = {
    completed: "default",
    in_progress: "secondary",
    expired: "outline",
  };

  const labels: Record<string, string> = {
    completed: "Ready",
    in_progress: "Processing",
    expired: "Expired",
  };

  return <Badge variant={variants[status] || "outline"}>{labels[status] || status}</Badge>;
}

export default function VectorStoresPage() {
  const { toast } = useToast();
  const confirm = useConfirm();
  const queryClient = useQueryClient();
  const [isModalOpen, setIsModalOpen] = useState(false);
  const [editingStore, setEditingStore] = useState<VectorStore | null>(null);
  const [selectedOrg, setSelectedOrg] = useState<string | null>(null);

  const pagination = useOpenAIPagination({ defaultLimit: 20 });

  // Fetch organizations
  const { data: organizations } = useQuery(organizationListOptions());

  // Get the selected organization's ID
  const effectiveOrg = selectedOrg || organizations?.data?.[0]?.slug;
  const selectedOrgData = organizations?.data?.find((org) => org.slug === effectiveOrg);

  // Fetch vector stores for selected org
  const {
    data: vectorStores,
    isLoading,
    error,
  } = useQuery({
    ...vectorStoreListOptions({
      query: {
        owner_type: "organization",
        owner_id: selectedOrgData?.id || "",
        ...pagination.queryParams,
      },
    }),
    enabled: !!selectedOrgData?.id,
  });

  // Reset pagination when organization changes
  useEffect(() => {
    pagination.actions.goToFirstPage();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedOrg]);

  const createMutation = useMutation({
    ...vectorStoreCreateMutation(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "vectorStoreList" }] });
      setIsModalOpen(false);
      toast({ title: "Knowledge base created", type: "success" });
    },
    onError: (error) => {
      toast({
        title: "Failed to create knowledge base",
        description: String(error),
        type: "error",
      });
    },
  });

  const deleteMutation = useMutation({
    ...vectorStoreDeleteMutation(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "vectorStoreList" }] });
      toast({ title: "Knowledge base deleted", type: "success" });
    },
    onError: (error) => {
      toast({
        title: "Failed to delete knowledge base",
        description: String(error),
        type: "error",
      });
    },
  });

  const updateMutation = useMutation({
    ...vectorStoreModifyMutation(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "vectorStoreList" }] });
      setIsModalOpen(false);
      setEditingStore(null);
      toast({ title: "Knowledge base updated", type: "success" });
    },
    onError: (error) => {
      toast({
        title: "Failed to update knowledge base",
        description: String(error),
        type: "error",
      });
    },
  });

  const handleEdit = (store: VectorStore) => {
    setEditingStore(store);
    setIsModalOpen(true);
  };

  const handleCreate = () => {
    setEditingStore(null);
    setIsModalOpen(true);
  };

  const handleClose = () => {
    setIsModalOpen(false);
    setEditingStore(null);
  };

  const handleCreateSubmit = (data: CreateVectorStore) => {
    createMutation.mutate({ body: data });
  };

  const handleEditSubmit = (data: UpdateVectorStore) => {
    if (!editingStore) return;
    updateMutation.mutate({
      path: { vector_store_id: editingStore.id },
      body: data,
    });
  };

  const columns = [
    columnHelper.accessor("name", {
      header: "Name",
      cell: (info) => (
        <Link
          to={`/admin/vector-stores/${info.row.original.id}`}
          className="flex items-center gap-2 hover:underline"
        >
          <Database className="h-4 w-4 text-muted-foreground" />
          <span className="font-medium">{info.getValue()}</span>
        </Link>
      ),
    }),
    columnHelper.accessor("id", {
      header: "ID",
      cell: (info) => <CodeBadge className="text-xs">{info.getValue().slice(0, 8)}...</CodeBadge>,
    }),
    columnHelper.accessor("status", {
      header: "Status",
      cell: (info) => <VectorStoreStatusBadge status={info.getValue()} />,
    }),
    columnHelper.accessor("file_counts", {
      header: "Files",
      cell: (info) => {
        const counts = info.getValue();
        return (
          <div className="flex items-center gap-1">
            <FileText className="h-4 w-4 text-muted-foreground" />
            <span>{counts.total}</span>
            {counts.in_progress > 0 && (
              <Badge variant="secondary" className="ml-1 text-xs">
                {counts.in_progress} processing
              </Badge>
            )}
          </div>
        );
      },
    }),
    columnHelper.accessor("usage_bytes", {
      header: "Size",
      cell: (info) => formatBytes(info.getValue()),
    }),
    columnHelper.accessor("embedding_model", {
      header: "Embedding Model",
      cell: (info) => <CodeBadge className="text-xs">{info.getValue()}</CodeBadge>,
    }),
    columnHelper.accessor("created_at", {
      header: "Created",
      cell: (info) => formatDateTime(info.getValue()),
    }),
    columnHelper.display({
      id: "actions",
      cell: ({ row }) => (
        <Dropdown>
          <DropdownTrigger aria-label="Actions" variant="ghost" className="h-8 w-8 p-0">
            <MoreHorizontal className="h-4.5 w-4.5" />
          </DropdownTrigger>
          <DropdownContent align="end">
            <DropdownItem onClick={() => handleEdit(row.original)}>
              <Pencil className="mr-2 h-4 w-4" />
              Edit
            </DropdownItem>
            <DropdownItem
              className="text-destructive"
              onClick={async () => {
                const confirmed = await confirm({
                  title: "Delete Knowledge Base",
                  message: `Are you sure you want to delete "${row.original.name}"? This will remove all files and embeddings. This action cannot be undone.`,
                  confirmLabel: "Delete",
                  variant: "destructive",
                });
                if (confirmed) {
                  deleteMutation.mutate({ path: { vector_store_id: row.original.id } });
                }
              }}
            >
              <Trash2 className="mr-2 h-4 w-4" />
              Delete
            </DropdownItem>
          </DropdownContent>
        </Dropdown>
      ),
    }),
  ];

  return (
    <div className="p-6">
      <PageHeader
        title="Knowledge Bases"
        description="Manage knowledge bases for RAG (Retrieval Augmented Generation)"
        actionLabel="New Knowledge Base"
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
        title="Knowledge Bases"
        columns={columns}
        data={vectorStores?.data || []}
        isLoading={isLoading}
        error={error}
        emptyMessage="No knowledge bases found. Create one to store document embeddings for RAG."
        errorMessage="Failed to load knowledge bases. Please try again."
        noDataMessage={
          !effectiveOrg ? "Create an organization first to manage knowledge bases." : undefined
        }
        paginationProps={{
          pagination: pagination.toPaginationMeta(vectorStores),
          isFirstPage: pagination.info.isFirstPage,
          pageNumber: pagination.info.pageNumber,
          onPrevious: () => pagination.actions.goToPreviousPage(vectorStores!),
          onNext: () => pagination.actions.goToNextPage(vectorStores!),
          onFirst: () => pagination.actions.goToFirstPage(),
        }}
      />

      <VectorStoreFormModal
        isOpen={isModalOpen}
        onClose={handleClose}
        onCreateSubmit={handleCreateSubmit}
        onEditSubmit={handleEditSubmit}
        isLoading={createMutation.isPending || updateMutation.isPending}
        editingStore={editingStore}
        organizations={organizations?.data}
      />
    </div>
  );
}
