import { zodResolver } from "@hookform/resolvers/zod";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { createColumnHelper } from "@tanstack/react-table";
import { MoreHorizontal, Pencil, Trash2, Bot } from "lucide-react";
import { useState } from "react";
import { useForm } from "react-hook-form";
import { z } from "zod";

import {
  organizationListOptions,
  serviceAccountListOptions,
  serviceAccountCreateMutation,
  serviceAccountDeleteMutation,
  serviceAccountUpdateMutation,
} from "@/api/generated/@tanstack/react-query.gen";
import type { ServiceAccount } from "@/api/generated/types.gen";
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
import { Textarea } from "@/components/Textarea/Textarea";
import { Modal, ModalHeader, ModalContent, ModalFooter } from "@/components/Modal/Modal";
import { useToast } from "@/components/Toast/Toast";
import { useConfirm } from "@/components/ConfirmDialog/ConfirmDialog";
import { PageHeader, ResourceTable, OrganizationSelect } from "@/components/Admin";
import { formatDateTime } from "@/utils/formatters";

const columnHelper = createColumnHelper<ServiceAccount>();

const createServiceAccountSchema = z.object({
  name: z.string().min(1, "Name is required"),
  slug: z
    .string()
    .min(1, "Slug is required")
    .regex(/^[a-z0-9-]+$/, "Slug must be lowercase alphanumeric with hyphens only"),
  description: z.string().optional(),
  roles: z.string().optional(),
});

type CreateServiceAccountForm = z.infer<typeof createServiceAccountSchema>;

const editServiceAccountSchema = z.object({
  name: z.string().min(1, "Name is required"),
  description: z.string().optional(),
  roles: z.string().optional(),
});

type EditServiceAccountForm = z.infer<typeof editServiceAccountSchema>;

function parseRoles(rolesString: string | undefined): string[] {
  if (!rolesString) return [];
  return rolesString
    .split(",")
    .map((r) => r.trim())
    .filter((r) => r.length > 0);
}

function formatRoles(roles: string[] | undefined): string {
  return roles?.join(", ") ?? "";
}

export default function ServiceAccountsPage() {
  const { toast } = useToast();
  const confirm = useConfirm();
  const queryClient = useQueryClient();
  const [isCreateModalOpen, setIsCreateModalOpen] = useState(false);
  const [isEditModalOpen, setIsEditModalOpen] = useState(false);
  const [editingAccount, setEditingAccount] = useState<ServiceAccount | null>(null);
  const [selectedOrg, setSelectedOrg] = useState<string | null>(null);

  const createForm = useForm<CreateServiceAccountForm>({
    resolver: zodResolver(createServiceAccountSchema),
    defaultValues: {
      name: "",
      slug: "",
      description: "",
      roles: "",
    },
  });

  const editForm = useForm<EditServiceAccountForm>({
    resolver: zodResolver(editServiceAccountSchema),
    defaultValues: {
      name: "",
      description: "",
      roles: "",
    },
  });

  // Fetch organizations
  const { data: organizations } = useQuery(organizationListOptions());
  const effectiveOrg = selectedOrg || organizations?.data?.[0]?.slug;

  // Fetch service accounts for selected org
  const {
    data: serviceAccounts,
    isLoading,
    error,
  } = useQuery({
    ...serviceAccountListOptions({ path: { org_slug: effectiveOrg || "" } }),
    enabled: !!effectiveOrg,
  });

  const createMutation = useMutation({
    ...serviceAccountCreateMutation(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "serviceAccountList" }] });
      setIsCreateModalOpen(false);
      createForm.reset();
      toast({ title: "Service account created", type: "success" });
    },
    onError: (error) => {
      toast({
        title: "Failed to create service account",
        description: String(error),
        type: "error",
      });
    },
  });

  const deleteMutation = useMutation({
    ...serviceAccountDeleteMutation(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "serviceAccountList" }] });
      toast({ title: "Service account deleted", type: "success" });
    },
    onError: (error) => {
      toast({
        title: "Failed to delete service account",
        description: String(error),
        type: "error",
      });
    },
  });

  const updateMutation = useMutation({
    ...serviceAccountUpdateMutation(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "serviceAccountList" }] });
      setIsEditModalOpen(false);
      setEditingAccount(null);
      toast({ title: "Service account updated", type: "success" });
    },
    onError: (error) => {
      toast({
        title: "Failed to update service account",
        description: String(error),
        type: "error",
      });
    },
  });

  const handleEdit = (account: ServiceAccount) => {
    setEditingAccount(account);
    editForm.reset({
      name: account.name,
      description: account.description ?? "",
      roles: formatRoles(account.roles),
    });
    setIsEditModalOpen(true);
  };

  const handleSlugChange = (name: string) => {
    const slug = name
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-|-$/g, "");
    createForm.setValue("name", name);
    createForm.setValue("slug", slug);
  };

  const columns = [
    columnHelper.accessor("name", {
      header: "Name",
      cell: (info) => (
        <div className="flex items-center gap-2">
          <Bot className="h-4 w-4 text-muted-foreground" />
          <span className="font-medium">{info.getValue()}</span>
        </div>
      ),
    }),
    columnHelper.accessor("slug", {
      header: "Slug",
      cell: (info) => <CodeBadge>{info.getValue()}</CodeBadge>,
    }),
    columnHelper.accessor("roles", {
      header: "Roles",
      cell: (info) => {
        const roles = info.getValue();
        if (!roles || roles.length === 0) {
          return <span className="text-muted-foreground">No roles</span>;
        }
        return (
          <div className="flex flex-wrap gap-1">
            {roles.map((role) => (
              <CodeBadge key={role}>{role}</CodeBadge>
            ))}
          </div>
        );
      },
    }),
    columnHelper.accessor("description", {
      header: "Description",
      cell: (info) => <span className="text-muted-foreground">{info.getValue() || "-"}</span>,
    }),
    columnHelper.accessor("created_at", {
      header: "Created",
      cell: (info) => formatDateTime(info.getValue()),
    }),
    columnHelper.display({
      id: "actions",
      header: () => <span className="sr-only">Actions</span>,
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
                  title: "Delete Service Account",
                  message: `Are you sure you want to delete "${row.original.name}"? This action cannot be undone. All API keys owned by this service account will be revoked.`,
                  confirmLabel: "Delete",
                  variant: "destructive",
                });
                if (confirmed) {
                  deleteMutation.mutate({
                    path: { org_slug: effectiveOrg!, sa_slug: row.original.slug },
                  });
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

  const onCreateSubmit = (data: CreateServiceAccountForm) => {
    if (!effectiveOrg) return;
    createMutation.mutate({
      path: { org_slug: effectiveOrg },
      body: {
        name: data.name,
        slug: data.slug,
        description: data.description || undefined,
        roles: parseRoles(data.roles),
      },
    });
  };

  const onEditSubmit = (data: EditServiceAccountForm) => {
    if (!editingAccount || !effectiveOrg) return;
    updateMutation.mutate({
      path: { org_slug: effectiveOrg, sa_slug: editingAccount.slug },
      body: {
        name: data.name,
        description: data.description || undefined,
        roles: parseRoles(data.roles),
      },
    });
  };

  return (
    <div className="p-6">
      <PageHeader
        title="Service Accounts"
        description="Machine identities for API key authentication with role-based access control"
        actionLabel="New Service Account"
        onAction={() => setIsCreateModalOpen(true)}
        actionDisabled={!effectiveOrg}
      />

      {organizations?.data && (
        <OrganizationSelect
          organizations={organizations.data}
          value={selectedOrg}
          onChange={setSelectedOrg}
          label="Organization"
          className="mb-4"
        />
      )}

      <ResourceTable
        title="All Service Accounts"
        columns={columns}
        data={serviceAccounts?.data || []}
        isLoading={isLoading}
        error={error}
        emptyMessage="No service accounts yet. Create one to get started."
        errorMessage="Failed to load service accounts. Please try again."
        noDataMessage={
          !effectiveOrg ? "Create an organization first to manage service accounts." : undefined
        }
      />

      {/* Create Service Account Modal */}
      <Modal open={isCreateModalOpen} onClose={() => setIsCreateModalOpen(false)}>
        <form onSubmit={createForm.handleSubmit(onCreateSubmit)}>
          <ModalHeader>Create Service Account</ModalHeader>
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
                  placeholder="CI/CD Bot"
                />
              </FormField>
              <FormField
                label="Slug"
                htmlFor="slug"
                required
                helpText="Used in URLs and API paths"
                error={createForm.formState.errors.slug?.message}
              >
                <Input id="slug" {...createForm.register("slug")} placeholder="ci-cd-bot" />
              </FormField>
              <FormField
                label="Description"
                htmlFor="description"
                error={createForm.formState.errors.description?.message}
              >
                <Textarea
                  id="description"
                  {...createForm.register("description")}
                  placeholder="Automated deployment service account"
                  rows={2}
                />
              </FormField>
              <FormField
                label="Roles"
                htmlFor="roles"
                helpText="Comma-separated list of roles (e.g., admin, deployer, viewer)"
                error={createForm.formState.errors.roles?.message}
              >
                <Input
                  id="roles"
                  {...createForm.register("roles")}
                  placeholder="deployer, viewer"
                />
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

      {/* Edit Service Account Modal */}
      <Modal open={isEditModalOpen} onClose={() => setIsEditModalOpen(false)}>
        <form onSubmit={editForm.handleSubmit(onEditSubmit)}>
          <ModalHeader>Edit Service Account</ModalHeader>
          <ModalContent>
            <div className="space-y-4">
              {editingAccount && (
                <div className="rounded-md bg-muted p-3">
                  <p className="text-sm">
                    Editing: <CodeBadge>{editingAccount.slug}</CodeBadge>
                  </p>
                </div>
              )}
              <FormField
                label="Name"
                htmlFor="edit-name"
                error={editForm.formState.errors.name?.message}
              >
                <Input id="edit-name" {...editForm.register("name")} placeholder="CI/CD Bot" />
              </FormField>
              <FormField
                label="Description"
                htmlFor="edit-description"
                error={editForm.formState.errors.description?.message}
              >
                <Textarea
                  id="edit-description"
                  {...editForm.register("description")}
                  placeholder="Automated deployment service account"
                  rows={2}
                />
              </FormField>
              <FormField
                label="Roles"
                htmlFor="edit-roles"
                helpText="Comma-separated list of roles"
                error={editForm.formState.errors.roles?.message}
              >
                <Input
                  id="edit-roles"
                  {...editForm.register("roles")}
                  placeholder="deployer, viewer"
                />
              </FormField>
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
    </div>
  );
}
