import { useEffect, useState } from "react";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { ClipboardPenLine } from "lucide-react";

import type { CreateTemplate, TemplateOwner, Template } from "@/api/generated/types.gen";
import { templateCreate, templateUpdate } from "@/api/generated/sdk.gen";
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
import { TemplateVariableEditor } from "@/components/TemplateVariableEditor/TemplateVariableEditor";
import { type TemplateVariable, parseVariables } from "@/lib/templateVariables";

const promptFormSchema = z.object({
  name: z.string().min(1, "Name is required").max(100, "Name must be 100 characters or less"),
  description: z.string().max(500, "Description must be 500 characters or less").optional(),
  content: z.string().min(1, "Prompt content is required"),
});

type PromptFormValues = z.infer<typeof promptFormSchema>;

export interface AdminPromptFormModalProps {
  open: boolean;
  onClose: () => void;
  editingPrompt?: Template | null;
  ownerOverride: TemplateOwner;
  onSaved?: (template: Template) => void;
}

export function AdminPromptFormModal({
  open,
  onClose,
  editingPrompt,
  ownerOverride,
  onSaved,
}: AdminPromptFormModalProps) {
  const queryClient = useQueryClient();
  const isEditing = !!editingPrompt;
  const [variables, setVariables] = useState<TemplateVariable[]>([]);

  const form = useForm<PromptFormValues>({
    resolver: zodResolver(promptFormSchema),
    defaultValues: { name: "", description: "", content: "" },
  });

  useEffect(() => {
    if (open) {
      if (editingPrompt) {
        form.reset({
          name: editingPrompt.name,
          description: editingPrompt.description || "",
          content: editingPrompt.content,
        });
        setVariables(parseVariables(editingPrompt.metadata));
      } else {
        form.reset({ name: "", description: "", content: "" });
        setVariables([]);
      }
    }
  }, [open, editingPrompt, form]);

  const createMutation = useMutation({
    mutationFn: async (data: CreateTemplate) => {
      const response = await templateCreate({ body: data });
      if (response.error) {
        throw new Error(
          typeof response.error === "object" && "message" in response.error
            ? String(response.error.message)
            : "Failed to create prompt"
        );
      }
      return response.data as Template;
    },
    onSuccess: (prompt) => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "templateListByOrg" }] });
      queryClient.invalidateQueries({ queryKey: [{ _id: "templateListByTeam" }] });
      queryClient.invalidateQueries({ queryKey: [{ _id: "templateListByProject" }] });
      onSaved?.(prompt);
      onClose();
    },
  });

  const updateMutation = useMutation({
    mutationFn: async ({ id, data }: { id: string; data: Partial<CreateTemplate> }) => {
      const response = await templateUpdate({
        path: { id },
        body: {
          name: data.name,
          description: data.description,
          content: data.content,
          metadata: data.metadata,
        },
      });
      if (response.error) {
        throw new Error(
          typeof response.error === "object" && "message" in response.error
            ? String(response.error.message)
            : "Failed to update prompt"
        );
      }
      return response.data as Template;
    },
    onSuccess: (prompt) => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "templateListByOrg" }] });
      queryClient.invalidateQueries({ queryKey: [{ _id: "templateListByTeam" }] });
      queryClient.invalidateQueries({ queryKey: [{ _id: "templateListByProject" }] });
      onSaved?.(prompt);
      onClose();
    },
  });

  const isLoading = createMutation.isPending || updateMutation.isPending;
  const error = createMutation.error || updateMutation.error;

  const handleSubmit = form.handleSubmit((data) => {
    const metadata = variables.length > 0 ? { variables } : undefined;

    if (isEditing && editingPrompt) {
      updateMutation.mutate({
        id: editingPrompt.id,
        data: {
          name: data.name,
          description: data.description || undefined,
          content: data.content,
          metadata,
        },
      });
    } else {
      createMutation.mutate({
        name: data.name,
        description: data.description || undefined,
        content: data.content,
        owner: ownerOverride,
        metadata,
      });
    }
  });

  const handleClose = () => {
    if (!isLoading) {
      form.reset();
      createMutation.reset();
      updateMutation.reset();
      onClose();
    }
  };

  return (
    <Modal open={open} onClose={handleClose} className="max-w-lg">
      <ModalClose onClose={handleClose} />
      <form onSubmit={handleSubmit}>
        <ModalHeader>
          <ModalTitle className="flex items-center gap-2">
            <ClipboardPenLine className="h-5 w-5" />
            {isEditing ? "Edit Template" : "New Template"}
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
              htmlFor="admin-prompt-name"
              required
              error={form.formState.errors.name?.message}
            >
              <Input
                id="admin-prompt-name"
                {...form.register("name")}
                placeholder="e.g., Code Review Assistant"
              />
            </FormField>

            <FormField
              label="Description"
              htmlFor="admin-prompt-description"
              helpText="Optional description to help identify this template"
              error={form.formState.errors.description?.message}
            >
              <Input
                id="admin-prompt-description"
                {...form.register("description")}
                placeholder="e.g., Reviews code for best practices and potential issues"
              />
            </FormField>

            <FormField
              label="Content"
              htmlFor="admin-prompt-content"
              required
              error={form.formState.errors.content?.message}
            >
              <textarea
                id="admin-prompt-content"
                {...form.register("content")}
                placeholder="Enter the system prompt content..."
                className="w-full min-h-[150px] rounded-md border bg-background px-3 py-2 text-sm placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2 resize-y"
              />
            </FormField>

            <TemplateVariableEditor variables={variables} onChange={setVariables} />
          </div>
        </ModalContent>

        <ModalFooter>
          <Button type="button" variant="ghost" onClick={handleClose} disabled={isLoading}>
            Cancel
          </Button>
          <Button type="submit" isLoading={isLoading}>
            {isEditing ? "Save Changes" : "Create Template"}
          </Button>
        </ModalFooter>
      </form>
    </Modal>
  );
}
