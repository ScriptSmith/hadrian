import { useEffect } from "react";
import { useForm, Controller } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";

import type {
  DbModelPricing,
  CreateModelPricing,
  UpdateModelPricing,
  PricingOwner,
} from "@/api/generated/types.gen";
import { Button } from "@/components/Button/Button";
import { CodeBadge } from "@/components/CodeBadge/CodeBadge";
import { FormField } from "@/components/FormField/FormField";
import { Input } from "@/components/Input/Input";
import { Modal, ModalHeader, ModalContent, ModalFooter } from "@/components/Modal/Modal";

// Convert microcents to dollars for display
export const microcentsToDollars = (microcents: number) => (microcents / 1_000_000).toFixed(4);

// Convert dollars to microcents for storage
export const dollarsToMicrocents = (dollars: string) =>
  Math.round(parseFloat(dollars || "0") * 1_000_000);

const pricingSchema = z.object({
  provider: z.string().min(1, "Provider is required"),
  model: z.string().min(1, "Model is required"),
  input_per_1m_tokens: z.string().min(1, "Input cost is required"),
  output_per_1m_tokens: z.string().min(1, "Output cost is required"),
  cached_input_per_1m_tokens: z.string(),
  reasoning_per_1m_tokens: z.string(),
  per_request: z.string(),
  per_image: z.string(),
  source: z.enum(["manual", "provider_api", "default"]),
});

type PricingFormValues = z.infer<typeof pricingSchema>;

const defaultValues: PricingFormValues = {
  provider: "",
  model: "",
  input_per_1m_tokens: "",
  output_per_1m_tokens: "",
  cached_input_per_1m_tokens: "",
  reasoning_per_1m_tokens: "",
  per_request: "",
  per_image: "",
  source: "manual",
};

export interface PricingFormModalProps {
  isOpen: boolean;
  onClose: () => void;
  onCreateSubmit: (data: CreateModelPricing) => void;
  onEditSubmit: (data: UpdateModelPricing) => void;
  isLoading?: boolean;
  editingPricing?: DbModelPricing | null;
}

export function PricingFormModal({
  isOpen,
  onClose,
  onCreateSubmit,
  onEditSubmit,
  isLoading,
  editingPricing,
}: PricingFormModalProps) {
  const isEditing = !!editingPricing;

  const form = useForm<PricingFormValues>({
    resolver: zodResolver(pricingSchema),
    defaultValues,
  });

  // Reset form when modal opens with different data
  useEffect(() => {
    if (isOpen) {
      if (editingPricing) {
        form.reset({
          provider: editingPricing.provider,
          model: editingPricing.model,
          input_per_1m_tokens: microcentsToDollars(editingPricing.input_per_1m_tokens),
          output_per_1m_tokens: microcentsToDollars(editingPricing.output_per_1m_tokens),
          cached_input_per_1m_tokens: editingPricing.cached_input_per_1m_tokens
            ? microcentsToDollars(editingPricing.cached_input_per_1m_tokens)
            : "",
          reasoning_per_1m_tokens: editingPricing.reasoning_per_1m_tokens
            ? microcentsToDollars(editingPricing.reasoning_per_1m_tokens)
            : "",
          per_request: editingPricing.per_request
            ? microcentsToDollars(editingPricing.per_request)
            : "",
          per_image: editingPricing.per_image ? microcentsToDollars(editingPricing.per_image) : "",
          source: editingPricing.source,
        });
      } else {
        form.reset(defaultValues);
      }
    }
  }, [isOpen, editingPricing, form]);

  const handleSubmit = form.handleSubmit((data) => {
    if (isEditing) {
      const body: UpdateModelPricing = {
        input_per_1m_tokens: dollarsToMicrocents(data.input_per_1m_tokens),
        output_per_1m_tokens: dollarsToMicrocents(data.output_per_1m_tokens),
        cached_input_per_1m_tokens: data.cached_input_per_1m_tokens
          ? dollarsToMicrocents(data.cached_input_per_1m_tokens)
          : null,
        reasoning_per_1m_tokens: data.reasoning_per_1m_tokens
          ? dollarsToMicrocents(data.reasoning_per_1m_tokens)
          : null,
        per_request: data.per_request ? dollarsToMicrocents(data.per_request) : null,
        per_image: data.per_image ? dollarsToMicrocents(data.per_image) : null,
        source: data.source,
      };
      onEditSubmit(body);
    } else {
      const owner: PricingOwner = { type: "global" };
      const body: CreateModelPricing = {
        provider: data.provider,
        model: data.model,
        input_per_1m_tokens: dollarsToMicrocents(data.input_per_1m_tokens),
        output_per_1m_tokens: dollarsToMicrocents(data.output_per_1m_tokens),
        cached_input_per_1m_tokens: data.cached_input_per_1m_tokens
          ? dollarsToMicrocents(data.cached_input_per_1m_tokens)
          : null,
        reasoning_per_1m_tokens: data.reasoning_per_1m_tokens
          ? dollarsToMicrocents(data.reasoning_per_1m_tokens)
          : null,
        per_request: data.per_request ? dollarsToMicrocents(data.per_request) : null,
        per_image: data.per_image ? dollarsToMicrocents(data.per_image) : null,
        source: data.source,
        owner,
      };
      onCreateSubmit(body);
    }
  });

  return (
    <Modal open={isOpen} onClose={onClose}>
      <form onSubmit={handleSubmit}>
        <ModalHeader>{isEditing ? "Edit Pricing" : "Add Model Pricing"}</ModalHeader>
        <ModalContent>
          <div className="space-y-4">
            {isEditing && editingPricing && (
              <div className="rounded-md bg-muted p-3">
                <p className="text-sm">
                  <span className="font-medium">{editingPricing.provider}</span>
                  {" / "}
                  <CodeBadge>{editingPricing.model}</CodeBadge>
                </p>
              </div>
            )}

            {!isEditing && (
              <div className="grid grid-cols-2 gap-4">
                <FormField
                  label="Provider"
                  htmlFor="pricing-provider"
                  required
                  error={form.formState.errors.provider?.message}
                >
                  <Input
                    id="pricing-provider"
                    {...form.register("provider")}
                    placeholder="openai"
                  />
                </FormField>
                <FormField
                  label="Model"
                  htmlFor="pricing-model"
                  required
                  error={form.formState.errors.model?.message}
                >
                  <Input id="pricing-model" {...form.register("model")} placeholder="gpt-4" />
                </FormField>
              </div>
            )}

            <div className="grid grid-cols-2 gap-4">
              <FormField
                label="Input Cost ($/1M tokens)"
                htmlFor="pricing-input"
                required
                error={form.formState.errors.input_per_1m_tokens?.message}
              >
                <Input
                  id="pricing-input"
                  type="number"
                  step="0.0001"
                  min="0"
                  {...form.register("input_per_1m_tokens")}
                  placeholder="0.03"
                />
              </FormField>
              <FormField
                label="Output Cost ($/1M tokens)"
                htmlFor="pricing-output"
                required
                error={form.formState.errors.output_per_1m_tokens?.message}
              >
                <Input
                  id="pricing-output"
                  type="number"
                  step="0.0001"
                  min="0"
                  {...form.register("output_per_1m_tokens")}
                  placeholder="0.06"
                />
              </FormField>
            </div>

            <div className="grid grid-cols-2 gap-4">
              <FormField label="Cached Input ($/1M)" htmlFor="pricing-cached">
                <Input
                  id="pricing-cached"
                  type="number"
                  step="0.0001"
                  min="0"
                  {...form.register("cached_input_per_1m_tokens")}
                  placeholder="0.015"
                />
              </FormField>
              <FormField label="Reasoning ($/1M)" htmlFor="pricing-reasoning">
                <Input
                  id="pricing-reasoning"
                  type="number"
                  step="0.0001"
                  min="0"
                  {...form.register("reasoning_per_1m_tokens")}
                  placeholder="0.15"
                />
              </FormField>
            </div>

            <FormField label="Source" htmlFor="pricing-source">
              <Controller
                name="source"
                control={form.control}
                render={({ field }) => (
                  <select
                    id="pricing-source"
                    value={field.value}
                    onChange={field.onChange}
                    className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
                  >
                    <option value="manual">Manual</option>
                    <option value="provider_api">Provider API</option>
                    <option value="default">Default</option>
                  </select>
                )}
              />
            </FormField>
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
