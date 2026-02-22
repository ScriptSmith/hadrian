import { useEffect } from "react";
import { useForm, Controller } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Sparkles } from "lucide-react";

import type { CreatePrompt, PromptOwner, Prompt } from "@/api/generated/types.gen";
import { promptCreate, promptUpdate } from "@/api/generated/sdk.gen";
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
import { useUserPrompts } from "@/hooks/useUserPrompts";

const promptFormSchema = z.object({
  name: z.string().min(1, "Name is required").max(100, "Name must be 100 characters or less"),
  description: z.string().max(500, "Description must be 500 characters or less").optional(),
  content: z.string().min(1, "Prompt content is required"),
  organization_id: z.string().min(1, "Organization is required"),
});

type PromptFormValues = z.infer<typeof promptFormSchema>;

export interface PromptFormModalProps {
  /** Whether the modal is open */
  open: boolean;
  /** Callback when modal is closed */
  onClose: () => void;
  /** Initial content to pre-fill (e.g., from current system prompt) */
  initialContent?: string;
  /** Existing prompt to edit (if editing) */
  editingPrompt?: Prompt | null;
  /** Callback after successful save */
  onSaved?: (prompt: Prompt) => void;
}

export function PromptFormModal({
  open,
  onClose,
  initialContent = "",
  editingPrompt,
  onSaved,
}: PromptFormModalProps) {
  const queryClient = useQueryClient();
  const { organizations } = useUserPrompts();

  const isEditing = !!editingPrompt;

  const form = useForm<PromptFormValues>({
    resolver: zodResolver(promptFormSchema),
    defaultValues: {
      name: "",
      description: "",
      content: initialContent,
      organization_id: "",
    },
  });

  // Reset form when modal opens/closes or editing prompt changes
  useEffect(() => {
    if (open) {
      if (editingPrompt) {
        // Editing mode: populate with existing prompt data
        form.reset({
          name: editingPrompt.name,
          description: editingPrompt.description || "",
          content: editingPrompt.content,
          // For editing, we need to find the org ID from the prompt's owner
          organization_id:
            editingPrompt.owner_type === "organization"
              ? editingPrompt.owner_id
              : organizations[0]?.id || "",
        });
      } else {
        // Create mode: use initial content if provided
        form.reset({
          name: "",
          description: "",
          content: initialContent,
          organization_id: organizations[0]?.id || "",
        });
      }
    }
  }, [open, editingPrompt, initialContent, form, organizations]);

  const createMutation = useMutation({
    mutationFn: async (data: CreatePrompt) => {
      const response = await promptCreate({ body: data });
      if (response.error) {
        throw new Error(
          typeof response.error === "object" && "message" in response.error
            ? String(response.error.message)
            : "Failed to create prompt"
        );
      }
      return response.data as Prompt;
    },
    onSuccess: (prompt) => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "promptListByOrg" }] });
      onSaved?.(prompt);
      onClose();
    },
  });

  const updateMutation = useMutation({
    mutationFn: async ({ id, data }: { id: string; data: Partial<CreatePrompt> }) => {
      const response = await promptUpdate({
        path: { id },
        body: {
          name: data.name,
          description: data.description,
          content: data.content,
        },
      });
      if (response.error) {
        throw new Error(
          typeof response.error === "object" && "message" in response.error
            ? String(response.error.message)
            : "Failed to update prompt"
        );
      }
      return response.data as Prompt;
    },
    onSuccess: (prompt) => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "promptListByOrg" }] });
      onSaved?.(prompt);
      onClose();
    },
  });

  const isLoading = createMutation.isPending || updateMutation.isPending;
  const error = createMutation.error || updateMutation.error;

  const handleSubmit = form.handleSubmit((data) => {
    const owner: PromptOwner = {
      type: "organization",
      organization_id: data.organization_id,
    };

    if (isEditing && editingPrompt) {
      updateMutation.mutate({
        id: editingPrompt.id,
        data: {
          name: data.name,
          description: data.description || undefined,
          content: data.content,
        },
      });
    } else {
      const body: CreatePrompt = {
        name: data.name,
        description: data.description || undefined,
        content: data.content,
        owner,
      };
      createMutation.mutate(body);
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
            <Sparkles className="h-5 w-5" />
            {isEditing ? "Edit Prompt Template" : "Save as Template"}
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
              htmlFor="prompt-name"
              required
              error={form.formState.errors.name?.message}
            >
              <Input
                id="prompt-name"
                {...form.register("name")}
                placeholder="e.g., Code Review Assistant"
              />
            </FormField>

            <FormField
              label="Description"
              htmlFor="prompt-description"
              helpText="Optional description to help identify this template"
              error={form.formState.errors.description?.message}
            >
              <Input
                id="prompt-description"
                {...form.register("description")}
                placeholder="e.g., Reviews code for best practices and potential issues"
              />
            </FormField>

            <FormField
              label="Content"
              htmlFor="prompt-content"
              required
              error={form.formState.errors.content?.message}
            >
              <textarea
                id="prompt-content"
                {...form.register("content")}
                placeholder="Enter the system prompt content..."
                className="w-full min-h-[150px] rounded-md border bg-background px-3 py-2 text-sm placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2 resize-y"
              />
            </FormField>

            {!isEditing && (
              <FormField
                label="Organization"
                htmlFor="prompt-org"
                required
                error={form.formState.errors.organization_id?.message}
              >
                <Controller
                  name="organization_id"
                  control={form.control}
                  render={({ field }) => (
                    <select
                      id="prompt-org"
                      {...field}
                      className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
                    >
                      <option value="">Select organization...</option>
                      {organizations.map((org) => (
                        <option key={org.id} value={org.id}>
                          {org.name}
                        </option>
                      ))}
                    </select>
                  )}
                />
              </FormField>
            )}
          </div>
        </ModalContent>

        <ModalFooter>
          <Button type="button" variant="ghost" onClick={handleClose} disabled={isLoading}>
            Cancel
          </Button>
          <Button type="submit" isLoading={isLoading}>
            {isEditing ? "Save Changes" : "Save Template"}
          </Button>
        </ModalFooter>
      </form>
    </Modal>
  );
}
