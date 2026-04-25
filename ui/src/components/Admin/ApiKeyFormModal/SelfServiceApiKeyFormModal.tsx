import { useEffect } from "react";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";

import type { CreateSelfServiceApiKey } from "@/api/generated/types.gen";
import { Button } from "@/components/Button/Button";
import { Modal, ModalHeader, ModalContent, ModalFooter } from "@/components/Modal/Modal";

import {
  ApiKeyOptionsFields,
  type ApiKeyOptionsFormValues,
  buildApiKeyOptionsPayload,
  validateCidrNotation,
  validateModelPatterns,
} from "./apiKeyOptionsFields";
import { sovereigntyDefaults, sovereigntySchema } from "./sovereigntyFields";

const schema = z
  .object({
    name: z.string().min(1, "Name is required"),
    budget_limit_cents: z.string().optional(),
    budget_period: z.enum(["daily", "monthly", ""]).optional(),
    expires_at: z.string().optional(),
    scopes: z.array(z.string()).optional(),
    allowed_models: z.string().optional(),
    ip_allowlist: z.string().optional(),
    rate_limit_rpm: z.string().optional(),
    rate_limit_tpm: z.string().optional(),
    ...sovereigntySchema,
  })
  .refine((data) => validateModelPatterns(data.allowed_models), {
    message:
      "Invalid model pattern. Use alphanumeric characters, hyphens, dots, slashes, and optional trailing wildcard (*)",
    path: ["allowed_models"],
  })
  .refine((data) => validateCidrNotation(data.ip_allowlist), {
    message: "Invalid IP/CIDR notation. Use format like 192.168.1.1, 10.0.0.0/8, or 2001:db8::/32",
    path: ["ip_allowlist"],
  });

const defaultValues: ApiKeyOptionsFormValues = {
  name: "",
  budget_limit_cents: "",
  budget_period: "",
  expires_at: "",
  scopes: [],
  allowed_models: "",
  ip_allowlist: "",
  rate_limit_rpm: "",
  rate_limit_tpm: "",
  ...sovereigntyDefaults,
};

export interface SelfServiceApiKeyFormModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSubmit: (data: CreateSelfServiceApiKey) => void;
  isLoading?: boolean;
}

export function SelfServiceApiKeyFormModal({
  isOpen,
  onClose,
  onSubmit,
  isLoading,
}: SelfServiceApiKeyFormModalProps) {
  const form = useForm<ApiKeyOptionsFormValues>({
    resolver: zodResolver(schema),
    defaultValues,
  });

  const selectedScopes = form.watch("scopes") || [];

  useEffect(() => {
    if (isOpen) {
      form.reset(defaultValues);
    }
  }, [isOpen, form]);

  const handleSubmit = form.handleSubmit((data) => {
    const payload = buildApiKeyOptionsPayload(data);
    const body: CreateSelfServiceApiKey & Record<string, unknown> = {
      name: payload.name,
      budget_limit_cents: payload.budget_limit_cents,
      budget_period: payload.budget_period,
      expires_at: payload.expires_at,
      scopes: payload.scopes,
      allowed_models: payload.allowed_models,
      ip_allowlist: payload.ip_allowlist,
      rate_limit_rpm: payload.rate_limit_rpm,
      rate_limit_tpm: payload.rate_limit_tpm,
      ...(payload.sovereignty_requirements && {
        sovereignty_requirements: payload.sovereignty_requirements,
      }),
    };
    onSubmit(body);
  });

  return (
    <Modal open={isOpen} onClose={onClose} className="max-w-3xl">
      <form onSubmit={handleSubmit}>
        <ModalHeader>Create API Key</ModalHeader>
        <ModalContent>
          <ApiKeyOptionsFields
            register={form.register}
            control={form.control}
            errors={form.formState.errors}
            selectedScopes={selectedScopes}
            idPrefix="self-apikey"
          />
        </ModalContent>
        <ModalFooter>
          <Button type="button" variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button type="submit" isLoading={isLoading}>
            Create
          </Button>
        </ModalFooter>
      </form>
    </Modal>
  );
}
