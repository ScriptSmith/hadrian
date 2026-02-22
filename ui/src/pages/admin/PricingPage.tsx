import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { createColumnHelper } from "@tanstack/react-table";
import { MoreHorizontal, Pencil, Trash2, DollarSign } from "lucide-react";
import { useState } from "react";

import {
  modelPricingListGlobalOptions,
  modelPricingCreateMutation,
  modelPricingDeleteMutation,
  modelPricingUpdateMutation,
} from "@/api/generated/@tanstack/react-query.gen";
import type {
  DbModelPricing,
  CreateModelPricing,
  UpdateModelPricing,
} from "@/api/generated/types.gen";
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
  OwnerBadge,
  PricingFormModal,
  microcentsToDollars,
} from "@/components/Admin";
import { useCursorPagination } from "@/hooks";
import { formatDateTime } from "@/utils/formatters";

const columnHelper = createColumnHelper<DbModelPricing>();

export default function PricingPage() {
  const { toast } = useToast();
  const confirm = useConfirm();
  const queryClient = useQueryClient();
  const [isModalOpen, setIsModalOpen] = useState(false);
  const [editingPricing, setEditingPricing] = useState<DbModelPricing | null>(null);

  const pagination = useCursorPagination({ defaultLimit: 25 });

  const {
    data: pricing,
    isLoading,
    error,
  } = useQuery(modelPricingListGlobalOptions({ query: pagination.queryParams }));

  const createMutation = useMutation({
    ...modelPricingCreateMutation(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "modelPricingListGlobal" }] });
      setIsModalOpen(false);
      toast({ title: "Pricing created", type: "success" });
    },
    onError: (error) => {
      toast({ title: "Failed to create pricing", description: String(error), type: "error" });
    },
  });

  const deleteMutation = useMutation({
    ...modelPricingDeleteMutation(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "modelPricingListGlobal" }] });
      toast({ title: "Pricing deleted", type: "success" });
    },
    onError: (error) => {
      toast({ title: "Failed to delete pricing", description: String(error), type: "error" });
    },
  });

  const updateMutation = useMutation({
    ...modelPricingUpdateMutation(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "modelPricingListGlobal" }] });
      setIsModalOpen(false);
      setEditingPricing(null);
      toast({ title: "Pricing updated", type: "success" });
    },
    onError: (error) => {
      toast({ title: "Failed to update pricing", description: String(error), type: "error" });
    },
  });

  const handleEdit = (pricingRow: DbModelPricing) => {
    setEditingPricing(pricingRow);
    setIsModalOpen(true);
  };

  const handleCreate = () => {
    setEditingPricing(null);
    setIsModalOpen(true);
  };

  const handleClose = () => {
    setIsModalOpen(false);
    setEditingPricing(null);
  };

  const handleCreateSubmit = (data: CreateModelPricing) => {
    createMutation.mutate({ body: data });
  };

  const handleEditSubmit = (data: UpdateModelPricing) => {
    if (!editingPricing) return;
    updateMutation.mutate({
      path: { id: editingPricing.id },
      body: data,
    });
  };

  const columns = [
    columnHelper.accessor("provider", {
      header: "Provider",
      cell: (info) => <span className="font-medium">{info.getValue()}</span>,
    }),
    columnHelper.accessor("model", {
      header: "Model",
      cell: (info) => <CodeBadge>{info.getValue()}</CodeBadge>,
    }),
    columnHelper.accessor("input_per_1m_tokens", {
      header: "Input/1M",
      cell: (info) => (
        <span className="font-mono text-sm">${microcentsToDollars(info.getValue())}</span>
      ),
    }),
    columnHelper.accessor("output_per_1m_tokens", {
      header: "Output/1M",
      cell: (info) => (
        <span className="font-mono text-sm">${microcentsToDollars(info.getValue())}</span>
      ),
    }),
    columnHelper.accessor("cached_input_per_1m_tokens", {
      header: "Cached Input/1M",
      cell: (info) => {
        const value = info.getValue();
        return value ? (
          <span className="font-mono text-sm">${microcentsToDollars(value)}</span>
        ) : (
          <span className="text-muted-foreground">-</span>
        );
      },
    }),
    columnHelper.accessor("owner", {
      header: "Scope",
      cell: (info) => <OwnerBadge owner={info.getValue()} />,
    }),
    columnHelper.accessor("source", {
      header: "Source",
      cell: (info) => {
        const source = info.getValue();
        const variant =
          source === "manual" ? "secondary" : source === "provider_api" ? "outline" : "secondary";
        return <Badge variant={variant}>{source}</Badge>;
      },
    }),
    columnHelper.accessor("updated_at", {
      header: "Updated",
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
                  title: "Delete Pricing",
                  message: `Are you sure you want to delete pricing for "${row.original.model}"? This action cannot be undone.`,
                  confirmLabel: "Delete",
                  variant: "destructive",
                });
                if (confirmed) {
                  deleteMutation.mutate({ path: { id: row.original.id } });
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
        title="Model Pricing"
        description="Configure pricing for models and providers"
        actionLabel="Add Pricing"
        onAction={handleCreate}
        actionIcon={<DollarSign className="mr-2 h-4 w-4" />}
      />

      <ResourceTable
        title="Pricing Configuration"
        columns={columns}
        data={pricing?.data || []}
        isLoading={isLoading}
        error={error}
        emptyMessage="No pricing configured yet. Add pricing to track costs."
        errorMessage="Failed to load pricing. Please try again."
        paginationProps={{
          pagination: pricing?.pagination,
          isFirstPage: pagination.info.isFirstPage,
          pageNumber: pagination.info.pageNumber,
          onPrevious: () => pagination.actions.goToPreviousPage(pricing!.pagination),
          onNext: () => pagination.actions.goToNextPage(pricing!.pagination),
          onFirst: () => pagination.actions.goToFirstPage(),
        }}
      />

      <PricingFormModal
        isOpen={isModalOpen}
        onClose={handleClose}
        onCreateSubmit={handleCreateSubmit}
        onEditSubmit={handleEditSubmit}
        isLoading={createMutation.isPending || updateMutation.isPending}
        editingPricing={editingPricing}
      />
    </div>
  );
}
