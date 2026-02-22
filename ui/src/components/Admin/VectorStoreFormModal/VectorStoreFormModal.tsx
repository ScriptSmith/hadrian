import { useEffect } from "react";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";

import type {
  VectorStore,
  CreateVectorStore,
  UpdateVectorStore,
  VectorStoreOwner,
  Organization,
} from "@/api/generated/types.gen";
import { Button } from "@/components/Button/Button";
import { FormField } from "@/components/FormField/FormField";
import { Input } from "@/components/Input/Input";
import { Textarea } from "@/components/Textarea/Textarea";
import { Modal, ModalHeader, ModalContent, ModalFooter } from "@/components/Modal/Modal";

export const EMBEDDING_MODELS = [
  { value: "text-embedding-3-small", label: "OpenAI text-embedding-3-small", dimensions: 1536 },
  { value: "text-embedding-3-large", label: "OpenAI text-embedding-3-large", dimensions: 3072 },
  { value: "text-embedding-ada-002", label: "OpenAI text-embedding-ada-002", dimensions: 1536 },
  { value: "voyage-3", label: "Voyage AI voyage-3", dimensions: 1024 },
  { value: "voyage-3-lite", label: "Voyage AI voyage-3-lite", dimensions: 512 },
] as const;

const createVectorStoreSchema = z.object({
  name: z.string().min(1, "Name is required").max(255, "Name must be less than 255 characters"),
  description: z.string().optional(),
  embedding_model: z.string().min(1, "Embedding model is required"),
  org_id: z.string().min(1, "Organization is required"),
});

const updateVectorStoreSchema = z.object({
  name: z.string().min(1, "Name is required").max(255, "Name must be less than 255 characters"),
  description: z.string().optional(),
});

type CreateFormValues = z.infer<typeof createVectorStoreSchema>;
type UpdateFormValues = z.infer<typeof updateVectorStoreSchema>;

const defaultValues: CreateFormValues = {
  name: "",
  description: "",
  embedding_model: "text-embedding-3-small",
  org_id: "",
};

export interface VectorStoreFormModalProps {
  isOpen: boolean;
  onClose: () => void;
  onCreateSubmit: (data: CreateVectorStore) => void;
  onEditSubmit: (data: UpdateVectorStore) => void;
  isLoading?: boolean;
  editingStore?: VectorStore | null;
  organizations?: Organization[];
}

export function VectorStoreFormModal({
  isOpen,
  onClose,
  onCreateSubmit,
  onEditSubmit,
  isLoading,
  editingStore,
  organizations,
}: VectorStoreFormModalProps) {
  const isEditing = !!editingStore;

  const form = useForm<CreateFormValues | UpdateFormValues>({
    resolver: zodResolver(isEditing ? updateVectorStoreSchema : createVectorStoreSchema),
    defaultValues,
  });

  // Reset form when modal opens with different data
  useEffect(() => {
    if (isOpen) {
      if (editingStore) {
        form.reset({
          name: editingStore.name,
          description: editingStore.description ?? "",
        });
      } else {
        form.reset(defaultValues);
      }
    }
  }, [isOpen, editingStore, form]);

  const handleSubmit = form.handleSubmit((data) => {
    if (isEditing) {
      const body: UpdateVectorStore = {
        name: data.name,
        description: data.description || null,
      };
      onEditSubmit(body);
    } else {
      const createData = data as CreateFormValues;
      const embeddingModel = EMBEDDING_MODELS.find((m) => m.value === createData.embedding_model);
      const owner: VectorStoreOwner = { type: "organization", organization_id: createData.org_id };
      const body: CreateVectorStore = {
        name: createData.name,
        description: createData.description || null,
        embedding_model: createData.embedding_model,
        embedding_dimensions: embeddingModel?.dimensions ?? 1536,
        owner,
      };
      onCreateSubmit(body);
    }
  });

  return (
    <Modal open={isOpen} onClose={onClose}>
      <form onSubmit={handleSubmit}>
        <ModalHeader>{isEditing ? "Edit Knowledge Base" : "Create Knowledge Base"}</ModalHeader>
        <ModalContent>
          <div className="space-y-4">
            <FormField
              label="Name"
              htmlFor="store-name"
              required
              helpText="A unique name for this knowledge base"
              error={form.formState.errors.name?.message}
            >
              <Input id="store-name" {...form.register("name")} placeholder="my-knowledge-base" />
            </FormField>

            <FormField
              label="Description"
              htmlFor="store-description"
              helpText="Optional description of what this knowledge base contains"
              error={form.formState.errors.description?.message}
            >
              <Textarea
                id="store-description"
                {...form.register("description")}
                placeholder="Documents for customer support RAG..."
                rows={3}
              />
            </FormField>

            {!isEditing && (
              <>
                <FormField
                  label="Embedding Model"
                  htmlFor="store-embedding-model"
                  required
                  helpText="The model used to generate embeddings. Cannot be changed after creation."
                  error={
                    (form.formState.errors as { embedding_model?: { message?: string } })
                      .embedding_model?.message
                  }
                >
                  <select
                    id="store-embedding-model"
                    {...form.register("embedding_model")}
                    className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
                  >
                    {EMBEDDING_MODELS.map((model) => (
                      <option key={model.value} value={model.value}>
                        {model.label} ({model.dimensions} dimensions)
                      </option>
                    ))}
                  </select>
                </FormField>

                {organizations && (
                  <FormField
                    label="Organization"
                    htmlFor="store-org"
                    required
                    error={
                      (form.formState.errors as { org_id?: { message?: string } }).org_id?.message
                    }
                  >
                    <select
                      id="store-org"
                      {...form.register("org_id")}
                      className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
                    >
                      <option value="">Select organization...</option>
                      {organizations.map((org) => (
                        <option key={org.id} value={org.id}>
                          {org.name}
                        </option>
                      ))}
                    </select>
                  </FormField>
                )}
              </>
            )}

            {isEditing && (
              <div className="rounded-md border border-muted bg-muted/50 p-3">
                <p className="text-sm text-muted-foreground">
                  <strong>Embedding Model:</strong> {editingStore.embedding_model}
                </p>
                <p className="text-sm text-muted-foreground">
                  <strong>Dimensions:</strong> {editingStore.embedding_dimensions}
                </p>
                <p className="mt-1 text-xs text-muted-foreground">
                  Embedding configuration cannot be changed after creation.
                </p>
              </div>
            )}
          </div>
        </ModalContent>
        <ModalFooter>
          <Button type="button" variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button type="submit" isLoading={isLoading}>
            {isEditing ? "Save" : "Create"}
          </Button>
        </ModalFooter>
      </form>
    </Modal>
  );
}
