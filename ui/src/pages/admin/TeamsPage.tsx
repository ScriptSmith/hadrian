import { zodResolver } from "@hookform/resolvers/zod";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { createColumnHelper } from "@tanstack/react-table";
import { MoreHorizontal, Pencil, Trash2, Users2 } from "lucide-react";
import { useState } from "react";
import { useForm } from "react-hook-form";
import { Link } from "react-router-dom";
import { z } from "zod";

import {
  organizationListOptions,
  teamListOptions,
  teamCreateMutation,
  teamDeleteMutation,
  teamUpdateMutation,
} from "@/api/generated/@tanstack/react-query.gen";
import type { Team } from "@/api/generated/types.gen";
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
import { PageHeader, ResourceTable, OrganizationSelect } from "@/components/Admin";
import { formatDateTime } from "@/utils/formatters";

const columnHelper = createColumnHelper<Team>();

const createTeamSchema = z.object({
  name: z.string().min(1, "Name is required"),
  slug: z
    .string()
    .min(1, "Slug is required")
    .regex(/^[a-z0-9-]+$/, "Slug must be lowercase alphanumeric with hyphens only"),
});

type CreateTeamForm = z.infer<typeof createTeamSchema>;

const editTeamSchema = z.object({
  name: z.string().min(1, "Name is required"),
});

type EditTeamForm = z.infer<typeof editTeamSchema>;

export default function TeamsPage() {
  const { toast } = useToast();
  const confirm = useConfirm();
  const queryClient = useQueryClient();
  const [isCreateModalOpen, setIsCreateModalOpen] = useState(false);
  const [isEditModalOpen, setIsEditModalOpen] = useState(false);
  const [editingTeam, setEditingTeam] = useState<Team | null>(null);
  const [selectedOrg, setSelectedOrg] = useState<string | null>(null);

  const createForm = useForm<CreateTeamForm>({
    resolver: zodResolver(createTeamSchema),
    defaultValues: {
      name: "",
      slug: "",
    },
  });

  const editForm = useForm<EditTeamForm>({
    resolver: zodResolver(editTeamSchema),
    defaultValues: {
      name: "",
    },
  });

  // Fetch organizations
  const { data: organizations } = useQuery(organizationListOptions());
  const effectiveOrg = selectedOrg || organizations?.data?.[0]?.slug;

  // Fetch teams for selected org
  const {
    data: teams,
    isLoading,
    error,
  } = useQuery({
    ...teamListOptions({ path: { org_slug: effectiveOrg || "" } }),
    enabled: !!effectiveOrg,
  });

  const createMutation = useMutation({
    ...teamCreateMutation(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "teamList" }] });
      setIsCreateModalOpen(false);
      createForm.reset();
      toast({ title: "Team created", type: "success" });
    },
    onError: (error) => {
      toast({ title: "Failed to create team", description: String(error), type: "error" });
    },
  });

  const deleteMutation = useMutation({
    ...teamDeleteMutation(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "teamList" }] });
      toast({ title: "Team deleted", type: "success" });
    },
    onError: (error) => {
      toast({ title: "Failed to delete team", description: String(error), type: "error" });
    },
  });

  const updateMutation = useMutation({
    ...teamUpdateMutation(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "teamList" }] });
      setIsEditModalOpen(false);
      setEditingTeam(null);
      toast({ title: "Team updated", type: "success" });
    },
    onError: (error) => {
      toast({ title: "Failed to update team", description: String(error), type: "error" });
    },
  });

  const handleEdit = (team: Team) => {
    setEditingTeam(team);
    editForm.reset({ name: team.name });
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
        <Link
          to={`/admin/organizations/${effectiveOrg}/teams/${info.row.original.slug}`}
          className="flex items-center gap-2 text-primary hover:underline"
        >
          <Users2 className="h-4 w-4 text-muted-foreground" />
          <span className="font-medium">{info.getValue()}</span>
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
                  title: "Delete Team",
                  message: `Are you sure you want to delete "${row.original.name}"? This action cannot be undone.`,
                  confirmLabel: "Delete",
                  variant: "destructive",
                });
                if (confirmed) {
                  deleteMutation.mutate({
                    path: { org_slug: effectiveOrg!, team_slug: row.original.slug },
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

  const onCreateSubmit = (data: CreateTeamForm) => {
    if (!effectiveOrg) return;
    createMutation.mutate({
      path: { org_slug: effectiveOrg },
      body: data,
    });
  };

  const onEditSubmit = (data: EditTeamForm) => {
    if (!editingTeam || !effectiveOrg) return;
    updateMutation.mutate({
      path: { org_slug: effectiveOrg, team_slug: editingTeam.slug },
      body: { name: data.name },
    });
  };

  return (
    <div className="p-6">
      <PageHeader
        title="Teams"
        description="Manage teams within organizations"
        actionLabel="New Team"
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
        title="All Teams"
        columns={columns}
        data={teams?.data || []}
        isLoading={isLoading}
        error={error}
        emptyMessage="No teams yet. Create one to get started."
        errorMessage="Failed to load teams. Please try again."
        noDataMessage={!effectiveOrg ? "Create an organization first to manage teams." : undefined}
      />

      {/* Create Team Modal */}
      <Modal open={isCreateModalOpen} onClose={() => setIsCreateModalOpen(false)}>
        <form onSubmit={createForm.handleSubmit(onCreateSubmit)}>
          <ModalHeader>Create Team</ModalHeader>
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
                  placeholder="Engineering"
                />
              </FormField>
              <FormField
                label="Slug"
                htmlFor="slug"
                required
                helpText="Used in URLs and API paths"
                error={createForm.formState.errors.slug?.message}
              >
                <Input id="slug" {...createForm.register("slug")} placeholder="engineering" />
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

      {/* Edit Team Modal */}
      <Modal open={isEditModalOpen} onClose={() => setIsEditModalOpen(false)}>
        <form onSubmit={editForm.handleSubmit(onEditSubmit)}>
          <ModalHeader>Edit Team</ModalHeader>
          <ModalContent>
            <div className="space-y-4">
              {editingTeam && (
                <div className="rounded-md bg-muted p-3">
                  <p className="text-sm">
                    Editing: <CodeBadge>{editingTeam.slug}</CodeBadge>
                  </p>
                </div>
              )}
              <FormField
                label="Name"
                htmlFor="edit-name"
                error={editForm.formState.errors.name?.message}
              >
                <Input id="edit-name" {...editForm.register("name")} placeholder="Engineering" />
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
