import { useEffect, useState } from "react";
import { useForm, Controller } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import { ChevronDown, Info } from "lucide-react";

import type { CreateSelfServiceApiKey, BudgetPeriod } from "@/api/generated/types.gen";
import { Button } from "@/components/Button/Button";
import { FormField } from "@/components/FormField/FormField";
import { Input } from "@/components/Input/Input";
import { Select } from "@/components/Select/Select";
import { Textarea } from "@/components/Textarea/Textarea";
import { Modal, ModalHeader, ModalContent, ModalFooter } from "@/components/Modal/Modal";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/Tooltip/Tooltip";
import { cn } from "@/utils/cn";

// Available API key scopes
const API_KEY_SCOPES = [
  { value: "chat", label: "Chat" },
  { value: "completions", label: "Completions" },
  { value: "embeddings", label: "Embeddings" },
  { value: "images", label: "Images" },
  { value: "audio", label: "Audio" },
  { value: "files", label: "Files" },
  { value: "models", label: "Models" },
  { value: "admin", label: "Admin" },
];

// Validation for model patterns (supports wildcards like "gpt-4*")
const MODEL_PATTERN_REGEX = /^[a-zA-Z0-9][a-zA-Z0-9\-._/]*\*?$/;

function validateModelPatterns(value: string | undefined): boolean {
  if (!value || value.trim() === "") return true;
  const patterns = value
    .split(",")
    .map((p) => p.trim())
    .filter(Boolean);
  return patterns.every((p) => MODEL_PATTERN_REGEX.test(p));
}

// Validation for IP/CIDR notation
const IPV4_REGEX = /^(\d{1,3}\.){3}\d{1,3}(\/\d{1,2})?$/;

function isValidIPv4(ip: string): boolean {
  const cidrMatch = ip.match(/^(.+)\/(\d+)$/);
  const address = cidrMatch ? cidrMatch[1] : ip;
  const prefix = cidrMatch ? parseInt(cidrMatch[2], 10) : null;
  if (prefix !== null && (prefix < 0 || prefix > 32)) return false;
  if (!IPV4_REGEX.test(ip)) return false;
  const octets = address.split(".").map((o) => parseInt(o, 10));
  return octets.every((o) => o >= 0 && o <= 255);
}

function isValidIPv6(ip: string): boolean {
  const cidrMatch = ip.match(/^(.+)\/(\d+)$/);
  const address = cidrMatch ? cidrMatch[1] : ip;
  const prefix = cidrMatch ? parseInt(cidrMatch[2], 10) : null;
  if (prefix !== null && (prefix < 0 || prefix > 128)) return false;
  if (!/^[0-9a-fA-F:]+$/.test(address)) return false;
  if (address.includes(":::")) return false;
  const doubleColonCount = (address.match(/::/g) || []).length;
  if (doubleColonCount > 1) return false;
  const groups = address.split(":");
  if (address.includes("::")) {
    const nonEmptyGroupCount = groups.filter((g) => g !== "").length;
    if (nonEmptyGroupCount > 7) return false;
  } else {
    if (groups.length !== 8) return false;
  }
  const nonEmptyGroups = groups.filter((g) => g !== "");
  return nonEmptyGroups.every((g) => g.length >= 1 && g.length <= 4 && /^[0-9a-fA-F]+$/.test(g));
}

function validateCidrNotation(value: string | undefined): boolean {
  if (!value || value.trim() === "") return true;
  const entries = value
    .split("\n")
    .map((e) => e.trim())
    .filter(Boolean);
  return entries.every((entry) => isValidIPv4(entry) || isValidIPv6(entry));
}

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

type FormValues = z.infer<typeof schema>;

const defaultValues: FormValues = {
  name: "",
  budget_limit_cents: "",
  budget_period: "",
  expires_at: "",
  scopes: [],
  allowed_models: "",
  ip_allowlist: "",
  rate_limit_rpm: "",
  rate_limit_tpm: "",
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
  const [advancedOpen, setAdvancedOpen] = useState(false);
  const form = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues,
  });

  const selectedScopes = form.watch("scopes") || [];

  // Reset form when modal opens
  useEffect(() => {
    if (isOpen) {
      form.reset(defaultValues);
      setAdvancedOpen(false);
    }
  }, [isOpen, form]);

  const handleSubmit = form.handleSubmit((data) => {
    const allowedModels = data.allowed_models
      ? data.allowed_models
          .split(",")
          .map((m) => m.trim())
          .filter(Boolean)
      : null;

    const ipAllowlist = data.ip_allowlist
      ? data.ip_allowlist
          .split("\n")
          .map((ip) => ip.trim())
          .filter(Boolean)
      : null;

    const body: CreateSelfServiceApiKey = {
      name: data.name,
      budget_limit_cents: data.budget_limit_cents
        ? Math.round(parseFloat(data.budget_limit_cents) * 100)
        : null,
      budget_period: (data.budget_period as BudgetPeriod) || null,
      expires_at: data.expires_at || null,
      scopes: data.scopes && data.scopes.length > 0 ? data.scopes : null,
      allowed_models: allowedModels && allowedModels.length > 0 ? allowedModels : null,
      ip_allowlist: ipAllowlist && ipAllowlist.length > 0 ? ipAllowlist : null,
      rate_limit_rpm: data.rate_limit_rpm ? parseInt(data.rate_limit_rpm) : null,
      rate_limit_tpm: data.rate_limit_tpm ? parseInt(data.rate_limit_tpm) : null,
    };

    onSubmit(body);
  });

  return (
    <Modal open={isOpen} onClose={onClose}>
      <form onSubmit={handleSubmit}>
        <ModalHeader>Create API Key</ModalHeader>
        <ModalContent>
          <div className="space-y-4">
            <FormField
              label="Name"
              htmlFor="self-apikey-name"
              required
              error={form.formState.errors.name?.message}
            >
              <Input id="self-apikey-name" {...form.register("name")} placeholder="My API Key" />
            </FormField>

            <div className="grid grid-cols-2 gap-4">
              <FormField
                label="Budget Limit ($)"
                htmlFor="self-apikey-budget"
                error={form.formState.errors.budget_limit_cents?.message}
              >
                <Input
                  id="self-apikey-budget"
                  type="number"
                  min="0"
                  step="0.01"
                  {...form.register("budget_limit_cents")}
                  placeholder="100.00"
                />
              </FormField>
              <FormField
                label="Budget Period"
                htmlFor="self-apikey-period"
                error={form.formState.errors.budget_period?.message}
              >
                <Controller
                  name="budget_period"
                  control={form.control}
                  render={({ field }) => (
                    <select
                      id="self-apikey-period"
                      {...field}
                      className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
                    >
                      <option value="">No period</option>
                      <option value="daily">Daily</option>
                      <option value="monthly">Monthly</option>
                    </select>
                  )}
                />
              </FormField>
            </div>

            <FormField
              label="Expires At"
              htmlFor="self-apikey-expires"
              helpText="Leave empty for no expiration"
              error={form.formState.errors.expires_at?.message}
            >
              <Input
                id="self-apikey-expires"
                type="datetime-local"
                {...form.register("expires_at")}
              />
            </FormField>

            {/* Advanced Settings - Collapsible */}
            <div className="border-t pt-4">
              <button
                type="button"
                className="flex w-full items-center justify-between text-sm font-medium text-muted-foreground hover:text-foreground"
                onClick={() => setAdvancedOpen(!advancedOpen)}
              >
                Advanced Settings
                <ChevronDown
                  className={cn("h-4 w-4 transition-transform", advancedOpen && "rotate-180")}
                />
              </button>

              <div
                className={cn(
                  "overflow-hidden transition-all duration-200",
                  advancedOpen ? "max-h-[600px] opacity-100 mt-4" : "max-h-0 opacity-0"
                )}
              >
                <div className="space-y-4">
                  {/* Scopes */}
                  <FormField
                    label={
                      <span className="flex items-center gap-1">
                        Permission Scopes
                        <Tooltip>
                          <TooltipTrigger asChild>
                            <Info className="h-3.5 w-3.5 text-muted-foreground cursor-help" />
                          </TooltipTrigger>
                          <TooltipContent className="max-w-xs">
                            <p>Restrict which API endpoints this key can access.</p>
                            <p className="mt-1 text-xs text-muted-foreground">
                              Leave empty for full access to all endpoints.
                            </p>
                          </TooltipContent>
                        </Tooltip>
                      </span>
                    }
                    htmlFor="self-apikey-scopes"
                    helpText={
                      selectedScopes.length > 0
                        ? `${selectedScopes.length} scope${selectedScopes.length === 1 ? "" : "s"} selected`
                        : "No restrictions (full access)"
                    }
                  >
                    <Controller
                      name="scopes"
                      control={form.control}
                      render={({ field }) => (
                        <Select
                          multiple
                          options={API_KEY_SCOPES}
                          value={field.value || []}
                          onChange={field.onChange}
                          placeholder="Select scopes..."
                          searchable
                        />
                      )}
                    />
                  </FormField>

                  {/* Allowed Models */}
                  <FormField
                    label="Model Restrictions"
                    htmlFor="self-apikey-models"
                    helpText="Comma-separated. Supports wildcards: gpt-4, claude-*, anthropic/*"
                    error={form.formState.errors.allowed_models?.message}
                  >
                    <Input
                      id="self-apikey-models"
                      {...form.register("allowed_models")}
                      placeholder="gpt-4, claude-*, anthropic/claude-3-*"
                    />
                  </FormField>

                  {/* IP Allowlist */}
                  <FormField
                    label="IP Allowlist"
                    htmlFor="self-apikey-ips"
                    helpText="One IP or CIDR per line. Leave empty to allow all IPs."
                    error={form.formState.errors.ip_allowlist?.message}
                  >
                    <Textarea
                      id="self-apikey-ips"
                      {...form.register("ip_allowlist")}
                      placeholder="192.168.1.0/24&#10;10.0.0.1&#10;2001:db8::/32"
                      className="font-mono text-xs min-h-[80px]"
                      rows={3}
                    />
                  </FormField>

                  {/* Rate Limits */}
                  <div className="grid grid-cols-2 gap-4">
                    <FormField
                      label="Requests/min"
                      htmlFor="self-apikey-rpm"
                      helpText="Override global limit"
                      error={form.formState.errors.rate_limit_rpm?.message}
                    >
                      <Input
                        id="self-apikey-rpm"
                        type="number"
                        min="1"
                        {...form.register("rate_limit_rpm")}
                        placeholder="Default"
                      />
                    </FormField>
                    <FormField
                      label="Tokens/min"
                      htmlFor="self-apikey-tpm"
                      helpText="Override global limit"
                      error={form.formState.errors.rate_limit_tpm?.message}
                    >
                      <Input
                        id="self-apikey-tpm"
                        type="number"
                        min="1"
                        {...form.register("rate_limit_tpm")}
                        placeholder="Default"
                      />
                    </FormField>
                  </div>
                </div>
              </div>
            </div>
          </div>
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
