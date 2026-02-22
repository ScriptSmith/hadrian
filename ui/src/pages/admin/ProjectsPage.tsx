import { zodResolver } from "@hookform/resolvers/zod";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { createColumnHelper } from "@tanstack/react-table";
import { MoreHorizontal, Pencil, Trash2, FolderOpen } from "lucide-react";
import { useState } from "react";
import { useForm } from "react-hook-form";
import { z } from "zod";

import {
  organizationListOptions,
  projectListOptions,
  projectCreateMutation,
  projectDeleteMutation,
  projectUpdateMutation,
  teamListOptions,
} from "@/api/generated/@tanstack/react-query.gen";
import type { Project } from "@/api/generated/types.gen";
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
import { PageHeader, ResourceTable, OrganizationSelect, TeamSelect } from "@/components/Admin";
import { formatDateTime } from "@/utils/formatters";

const columnHelper = createColumnHelper<Project>();

const createProjectSchema = z.object({
  name: z.string().min(1, "Name is required"),
  slug: z
    .string()
    .min(1, "Slug is required")
    .regex(/^[a-z0-9-]+$/, "Slug must be lowercase alphanumeric with hyphens only"),
  team_id: z.string().nullable().optional(),
});

type CreateProjectForm = z.infer<typeof createProjectSchema>;

const editProjectSchema = z.object({
  name: z.string().min(1, "Name is required"),
});

type EditProjectForm = z.infer<typeof editProjectSchema>;

export default function ProjectsPage() {
  const { toast } = useToast();
  const confirm = useConfirm();
  const queryClient = useQueryClient();
  const [isCreateModalOpen, setIsCreateModalOpen] = useState(false);
  const [isEditModalOpen, setIsEditModalOpen] = useState(false);
  const [editingProject, setEditingProject] = useState<Project | null>(null);
  const [selectedOrg, setSelectedOrg] = useState<string | null>(null);

  const createForm = useForm<CreateProjectForm>({
    resolver: zodResolver(createProjectSchema),
    defaultValues: {
      name: "",
      slug: "",
      team_id: null,
    },
  });

  const editForm = useForm<EditProjectForm>({
    resolver: zodResolver(editProjectSchema),
    defaultValues: {
      name: "",
    },
  });

  // Fetch organizations
  const { data: organizations } = useQuery(organizationListOptions());
  const effectiveOrg = selectedOrg || organizations?.data?.[0]?.slug;

  // Fetch teams for selected org
  const { data: teams } = useQuery({
    ...teamListOptions({ path: { org_slug: effectiveOrg || "" } }),
    enabled: !!effectiveOrg,
  });

  // Fetch projects for selected org
  const {
    data: projects,
    isLoading,
    error,
  } = useQuery({
    ...projectListOptions({ path: { org_slug: effectiveOrg || "" } }),
    enabled: !!effectiveOrg,
  });

  const createMutation = useMutation({
    ...projectCreateMutation(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "projectList" }] });
      setIsCreateModalOpen(false);
      createForm.reset();
      toast({ title: "Project created", type: "success" });
    },
    onError: (error) => {
      toast({ title: "Failed to create project", description: String(error), type: "error" });
    },
  });

  const deleteMutation = useMutation({
    ...projectDeleteMutation(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "projectList" }] });
      toast({ title: "Project deleted", type: "success" });
    },
    onError: (error) => {
      toast({ title: "Failed to delete project", description: String(error), type: "error" });
    },
  });

  const updateMutation = useMutation({
    ...projectUpdateMutation(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "projectList" }] });
      setIsEditModalOpen(false);
      setEditingProject(null);
      toast({ title: "Project updated", type: "success" });
    },
    onError: (error) => {
      toast({ title: "Failed to update project", description: String(error), type: "error" });
    },
  });

  const handleEdit = (project: Project) => {
    setEditingProject(project);
    editForm.reset({ name: project.name });
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
          <FolderOpen className="h-4 w-4 text-muted-foreground" />
          <span className="font-medium">{info.getValue()}</span>
        </div>
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
                  title: "Delete Project",
                  message: `Are you sure you want to delete "${row.original.name}"? This action cannot be undone.`,
                  confirmLabel: "Delete",
                  variant: "destructive",
                });
                if (confirmed) {
                  deleteMutation.mutate({
                    path: { org_slug: effectiveOrg!, project_slug: row.original.slug },
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

  const onCreateSubmit = (data: CreateProjectForm) => {
    if (!effectiveOrg) return;
    createMutation.mutate({
      path: { org_slug: effectiveOrg },
      body: data,
    });
  };

  const onEditSubmit = (data: EditProjectForm) => {
    if (!editingProject || !effectiveOrg) return;
    updateMutation.mutate({
      path: { org_slug: effectiveOrg, project_slug: editingProject.slug },
      body: { name: data.name },
    });
  };

  return (
    <div className="p-6">
      <PageHeader
        title="Projects"
        description="Manage projects within organizations"
        actionLabel="New Project"
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
        title="All Projects"
        columns={columns}
        data={projects?.data || []}
        isLoading={isLoading}
        error={error}
        emptyMessage="No projects yet. Create one to get started."
        errorMessage="Failed to load projects. Please try again."
        noDataMessage={
          !effectiveOrg ? "Create an organization first to manage projects." : undefined
        }
      />

      {/* Create Project Modal */}
      <Modal open={isCreateModalOpen} onClose={() => setIsCreateModalOpen(false)}>
        <form onSubmit={createForm.handleSubmit(onCreateSubmit)}>
          <ModalHeader>Create Project</ModalHeader>
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
                  placeholder="My Project"
                />
              </FormField>
              <FormField
                label="Slug"
                htmlFor="slug"
                required
                helpText="Used in URLs and API paths"
                error={createForm.formState.errors.slug?.message}
              >
                <Input id="slug" {...createForm.register("slug")} placeholder="my-project" />
              </FormField>
              {teams?.data && teams.data.length > 0 && (
                <TeamSelect
                  teams={teams.data}
                  value={createForm.watch("team_id") ?? null}
                  onChange={(teamId) => createForm.setValue("team_id", teamId)}
                  label="Team (Optional)"
                  nonePlaceholder="None (Organization-level)"
                />
              )}
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

      {/* Edit Project Modal */}
      <Modal open={isEditModalOpen} onClose={() => setIsEditModalOpen(false)}>
        <form onSubmit={editForm.handleSubmit(onEditSubmit)}>
          <ModalHeader>Edit Project</ModalHeader>
          <ModalContent>
            <div className="space-y-4">
              {editingProject && (
                <div className="rounded-md bg-muted p-3">
                  <p className="text-sm">
                    Editing: <CodeBadge>{editingProject.slug}</CodeBadge>
                  </p>
                </div>
              )}
              <FormField
                label="Name"
                htmlFor="edit-name"
                error={editForm.formState.errors.name?.message}
              >
                <Input id="edit-name" {...editForm.register("name")} placeholder="My Project" />
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
