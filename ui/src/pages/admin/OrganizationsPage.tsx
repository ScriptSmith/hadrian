import { zodResolver } from "@hookform/resolvers/zod";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { createColumnHelper } from "@tanstack/react-table";
import { MoreHorizontal, Pencil, Trash2 } from "lucide-react";
import { useState } from "react";
import { useForm } from "react-hook-form";
import { Link } from "react-router-dom";
import { z } from "zod";

import {
  organizationListOptions,
  organizationCreateMutation,
  organizationDeleteMutation,
} from "@/api/generated/@tanstack/react-query.gen";
import type { Organization } from "@/api/generated/types.gen";
import { Button } from "@/components/Button/Button";
import { CodeBadge } from "@/components/CodeBadge/CodeBadge";
import {
  Dropdown,
  DropdownContent,
  DropdownItem,
  DropdownTrigger,
} from "@/components/Dropdown/Dropdown";
import { FormField } from "@/components/FormField/FormField";
import { Input } from "@/components/Input/Input";
import { Modal, ModalHeader, ModalContent, ModalFooter } from "@/components/Modal/Modal";
import { useToast } from "@/components/Toast/Toast";
import { useConfirm } from "@/components/ConfirmDialog/ConfirmDialog";
import { PageHeader, ResourceTable } from "@/components/Admin";
import { useCursorPagination } from "@/hooks";
import { formatDateTime } from "@/utils/formatters";

const columnHelper = createColumnHelper<Organization>();

const createOrganizationSchema = z.object({
  name: z.string().min(1, "Name is required"),
  slug: z
    .string()
    .min(1, "Slug is required")
    .regex(/^[a-z0-9-]+$/, "Slug must be lowercase alphanumeric with hyphens only"),
});

type CreateOrganizationForm = z.infer<typeof createOrganizationSchema>;

export default function OrganizationsPage() {
  const { toast } = useToast();
  const confirm = useConfirm();
  const queryClient = useQueryClient();
  const [isCreateModalOpen, setIsCreateModalOpen] = useState(false);

  const createForm = useForm<CreateOrganizationForm>({
    resolver: zodResolver(createOrganizationSchema),
    defaultValues: {
      name: "",
      slug: "",
    },
  });

  const pagination = useCursorPagination({ defaultLimit: 25 });

  const {
    data: organizations,
    isLoading,
    error,
  } = useQuery(organizationListOptions({ query: pagination.queryParams }));

  const createMutation = useMutation({
    ...organizationCreateMutation(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "organizationList" }] });
      setIsCreateModalOpen(false);
      createForm.reset();
      toast({ title: "Organization created", type: "success" });
    },
    onError: (error) => {
      toast({ title: "Failed to create organization", description: String(error), type: "error" });
    },
  });

  const deleteMutation = useMutation({
    ...organizationDeleteMutation(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "organizationList" }] });
      toast({ title: "Organization deleted", type: "success" });
    },
    onError: (error) => {
      toast({ title: "Failed to delete organization", description: String(error), type: "error" });
    },
  });

  const columns = [
    columnHelper.accessor("name", {
      header: "Name",
      cell: (info) => (
        <Link
          to={`/admin/organizations/${info.row.original.slug}`}
          className="font-medium text-primary hover:underline"
        >
          {info.getValue()}
        </Link>
      ),
    }),
    columnHelper.accessor("slug", {
      header: "Slug",
      cell: (info) => <CodeBadge>{info.getValue()}</CodeBadge>,
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
            <DropdownItem>
              <Pencil className="mr-2 h-4 w-4" />
              Edit
            </DropdownItem>
            <DropdownItem
              className="text-destructive"
              onClick={async () => {
                const confirmed = await confirm({
                  title: "Delete Organization",
                  message: `Are you sure you want to delete "${row.original.name}"? This action cannot be undone.`,
                  confirmLabel: "Delete",
                  variant: "destructive",
                });
                if (confirmed) {
                  deleteMutation.mutate({ path: { slug: row.original.slug } });
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

  const handleSlugChange = (name: string) => {
    const slug = name
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-|-$/g, "");
    createForm.setValue("name", name);
    createForm.setValue("slug", slug);
  };

  const onCreateSubmit = (data: CreateOrganizationForm) => {
    createMutation.mutate({ body: data });
  };

  return (
    <div className="p-6">
      <PageHeader
        title="Organizations"
        description="Manage organizations and their settings"
        actionLabel="New Organization"
        onAction={() => setIsCreateModalOpen(true)}
      />

      <ResourceTable
        title="All Organizations"
        columns={columns}
        data={organizations?.data || []}
        isLoading={isLoading}
        error={error}
        emptyMessage="No organizations yet. Create one to get started."
        errorMessage="Failed to load organizations. Please try again."
        paginationProps={{
          pagination: organizations?.pagination,
          isFirstPage: pagination.info.isFirstPage,
          pageNumber: pagination.info.pageNumber,
          onPrevious: () => pagination.actions.goToPreviousPage(organizations!.pagination),
          onNext: () => pagination.actions.goToNextPage(organizations!.pagination),
          onFirst: () => pagination.actions.goToFirstPage(),
        }}
      />

      {/* Create Organization Modal */}
      <Modal open={isCreateModalOpen} onClose={() => setIsCreateModalOpen(false)}>
        <form onSubmit={createForm.handleSubmit(onCreateSubmit)}>
          <ModalHeader>Create Organization</ModalHeader>
          <ModalContent>
            <div className="space-y-4">
              <FormField
                label="Name"
                htmlFor="name"
                required
                error={createForm.formState.errors.name?.message}
              >
                <Input
                  id="name"
                  value={createForm.watch("name")}
                  onChange={(e) => handleSlugChange(e.target.value)}
                  placeholder="My Organization"
                />
              </FormField>
              <FormField
                label="Slug"
                htmlFor="slug"
                required
                helpText="Used in URLs and API paths"
                error={createForm.formState.errors.slug?.message}
              >
                <Input id="slug" {...createForm.register("slug")} placeholder="my-organization" />
              </FormField>
            </div>
          </ModalContent>
          <ModalFooter>
            <Button type="button" variant="ghost" onClick={() => setIsCreateModalOpen(false)}>
              Cancel
            </Button>
            <Button type="submit" isLoading={createMutation.isPending}>
              Create
            </Button>
          </ModalFooter>
        </form>
      </Modal>
    </div>
  );
}
