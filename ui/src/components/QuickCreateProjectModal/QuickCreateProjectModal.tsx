import { useEffect, useCallback } from "react";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { FolderOpen } from "lucide-react";

import type { CreateProject } from "@/api/generated/types.gen";
import { projectCreate } from "@/api/generated/sdk.gen";
import { Button } from "@/components/Button/Button";
import { FormField } from "@/components/FormField/FormField";
import { Input } from "@/components/Input/Input";
import {
  Modal,
  ModalClose,
  ModalHeader,
  ModalTitle,
  ModalContent,
  ModalFooter,
} from "@/components/Modal/Modal";
import { useUserProjects } from "@/hooks/useUserProjects";

const createProjectSchema = z.object({
  name: z.string().min(1, "Name is required").max(100, "Name must be 100 characters or less"),
  slug: z
    .string()
    .min(1, "Slug is required")
    .max(100, "Slug must be 100 characters or less")
    .regex(/^[a-z0-9-]+$/, "Slug must be lowercase alphanumeric with hyphens only"),
});

type CreateProjectForm = z.infer<typeof createProjectSchema>;

export interface QuickCreateProjectModalProps {
  /** Whether the modal is open */
  open: boolean;
  /** Callback when modal is closed */
  onClose: () => void;
  /** Callback after successful creation */
  onCreated?: () => void;
}

export function QuickCreateProjectModal({
  open,
  onClose,
  onCreated,
}: QuickCreateProjectModalProps) {
  const queryClient = useQueryClient();
  const { organizations } = useUserProjects();

  const form = useForm<CreateProjectForm>({
    resolver: zodResolver(createProjectSchema),
    defaultValues: {
      name: "",
      slug: "",
    },
  });

  // Reset form when modal opens/closes
  useEffect(() => {
    if (open) {
      form.reset({
        name: "",
        slug: "",
      });
    }
  }, [open, form]);

  // Auto-generate slug from name
  const handleNameChange = (name: string) => {
    const slug = name
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-|-$/g, "");
    form.setValue("name", name);
    form.setValue("slug", slug);
  };

  const createMutation = useMutation({
    mutationFn: async (data: CreateProjectForm) => {
      const orgSlug = organizations[0]?.slug;
      if (!orgSlug) throw new Error("No organization available");
      const body: CreateProject = {
        name: data.name,
        slug: data.slug,
      };
      const response = await projectCreate({
        path: { org_slug: orgSlug },
        body,
      });
      if (response.error) {
        throw new Error(
          typeof response.error === "object" && "message" in response.error
            ? String(response.error.message)
            : "Failed to create project"
        );
      }
      return response.data;
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "projectList" }] });
      onCreated?.();
      onClose();
    },
  });

  const isLoading = createMutation.isPending;
  const error = createMutation.error;

  const handleSubmit = form.handleSubmit((data) => {
    createMutation.mutate(data);
  });

  const handleClose = useCallback(() => {
    if (!isLoading) {
      form.reset();
      createMutation.reset();
      onClose();
    }
  }, [isLoading, form, createMutation, onClose]);

  return (
    <Modal open={open} onClose={handleClose} className="max-w-md">
      <ModalClose onClose={handleClose} />
      <form onSubmit={handleSubmit}>
        <ModalHeader>
          <ModalTitle className="flex items-center gap-2">
            <FolderOpen className="h-5 w-5" />
            Create Project
          </ModalTitle>
        </ModalHeader>

        <ModalContent>
          <div className="space-y-4">
            {error && (
              <div className="rounded-md bg-destructive/10 px-3 py-2 text-sm text-destructive">
                {error.message}
              </div>
            )}

            <FormField
              label="Name"
              htmlFor="project-name"
              required
              error={form.formState.errors.name?.message}
            >
              <Input
                id="project-name"
                value={form.watch("name")}
                onChange={(e) => handleNameChange(e.target.value)}
                placeholder="My Project"
              />
            </FormField>

            <FormField
              label="Slug"
              htmlFor="project-slug"
              required
              helpText="Used in URLs and API paths"
              error={form.formState.errors.slug?.message}
            >
              <Input id="project-slug" {...form.register("slug")} placeholder="my-project" />
            </FormField>
          </div>
        </ModalContent>

        <ModalFooter>
          <Button type="button" variant="ghost" onClick={handleClose} disabled={isLoading}>
            Cancel
          </Button>
          <Button type="submit" isLoading={isLoading}>
            Create Project
          </Button>
        </ModalFooter>
      </form>
    </Modal>
  );
}
