import { useEffect, useState } from "react";
import { useForm, Controller } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import { ChevronDown, Info } from "lucide-react";
import { useQuery } from "@tanstack/react-query";

import type {
  CreateApiKey,
  ApiKeyOwner,
  BudgetPeriod,
  Organization,
} from "@/api/generated/types.gen";
import { serviceAccountListOptions } from "@/api/generated/@tanstack/react-query.gen";
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

  // Check prefix range for IPv4 (0-32)
  if (prefix !== null && (prefix < 0 || prefix > 32)) return false;

  // Validate IPv4 format and octet ranges
  if (!IPV4_REGEX.test(ip)) return false;
  const octets = address.split(".").map((o) => parseInt(o, 10));
  return octets.every((o) => o >= 0 && o <= 255);
}

function isValidIPv6(ip: string): boolean {
  const cidrMatch = ip.match(/^(.+)\/(\d+)$/);
  const address = cidrMatch ? cidrMatch[1] : ip;
  const prefix = cidrMatch ? parseInt(cidrMatch[2], 10) : null;

  // Check prefix range for IPv6 (0-128)
  if (prefix !== null && (prefix < 0 || prefix > 128)) return false;

  // Basic structure checks
  if (!/^[0-9a-fA-F:]+$/.test(address)) return false;

  // No triple colons allowed
  if (address.includes(":::")) return false;

  // Only one :: allowed
  const doubleColonCount = (address.match(/::/g) || []).length;
  if (doubleColonCount > 1) return false;

  // Split and validate groups
  const groups = address.split(":");

  // Handle :: compression
  if (address.includes("::")) {
    // With ::, total groups after expansion must be <= 8
    const nonEmptyGroupCount = groups.filter((g) => g !== "").length;
    // :: can represent 1 to (8 - nonEmptyGroupCount) groups
    if (nonEmptyGroupCount > 7) return false;
  } else {
    // Without ::, must have exactly 8 groups
    if (groups.length !== 8) return false;
  }

  // Validate each group is valid hex (1-4 chars)
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

const createApiKeySchema = z
  .object({
    name: z.string().min(1, "Name is required"),
    ownerType: z.enum(["organization", "user", "service_account"]),
    org_id: z.string().optional(),
    user_id: z.string().optional(),
    // For service account selection, we need org slug to fetch service accounts
    sa_org_slug: z.string().optional(),
    service_account_id: z.string().optional(),
    budget_limit_cents: z.string().optional(),
    budget_period: z.enum(["daily", "monthly", ""]).optional(),
    expires_at: z.string().optional(),
    // Advanced settings
    scopes: z.array(z.string()).optional(),
    allowed_models: z.string().optional(),
    ip_allowlist: z.string().optional(),
    rate_limit_rpm: z.string().optional(),
    rate_limit_tpm: z.string().optional(),
  })
  .refine(
    (data) => {
      if (data.ownerType === "organization") {
        return !!data.org_id && data.org_id.length > 0;
      }
      return true;
    },
    { message: "Organization is required", path: ["org_id"] }
  )
  .refine(
    (data) => {
      if (data.ownerType === "user") {
        return !!data.user_id && data.user_id.length > 0;
      }
      return true;
    },
    { message: "User ID is required", path: ["user_id"] }
  )
  .refine(
    (data) => {
      if (data.ownerType === "service_account") {
        return !!data.sa_org_slug && data.sa_org_slug.length > 0;
      }
      return true;
    },
    { message: "Organization is required", path: ["sa_org_slug"] }
  )
  .refine(
    (data) => {
      if (data.ownerType === "service_account") {
        return !!data.service_account_id && data.service_account_id.length > 0;
      }
      return true;
    },
    { message: "Service account is required", path: ["service_account_id"] }
  )
  .refine((data) => validateModelPatterns(data.allowed_models), {
    message:
      "Invalid model pattern. Use alphanumeric characters, hyphens, dots, slashes, and optional trailing wildcard (*)",
    path: ["allowed_models"],
  })
  .refine((data) => validateCidrNotation(data.ip_allowlist), {
    message: "Invalid IP/CIDR notation. Use format like 192.168.1.1, 10.0.0.0/8, or 2001:db8::/32",
    path: ["ip_allowlist"],
  });

type ApiKeyFormValues = z.infer<typeof createApiKeySchema>;

const defaultValues: ApiKeyFormValues = {
  name: "",
  ownerType: "organization",
  org_id: "",
  user_id: "",
  sa_org_slug: "",
  service_account_id: "",
  budget_limit_cents: "",
  budget_period: "",
  expires_at: "",
  scopes: [],
  allowed_models: "",
  ip_allowlist: "",
  rate_limit_rpm: "",
  rate_limit_tpm: "",
};

export interface ApiKeyFormModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSubmit: (data: CreateApiKey) => void;
  isLoading?: boolean;
  organizations?: Organization[];
}

export function ApiKeyFormModal({
  isOpen,
  onClose,
  onSubmit,
  isLoading,
  organizations,
}: ApiKeyFormModalProps) {
  const [advancedOpen, setAdvancedOpen] = useState(false);
  const form = useForm<ApiKeyFormValues>({
    resolver: zodResolver(createApiKeySchema),
    defaultValues,
  });

  const ownerType = form.watch("ownerType");
  const selectedScopes = form.watch("scopes") || [];
  const saOrgSlug = form.watch("sa_org_slug");

  // Fetch service accounts when an org is selected for service_account owner type
  const { data: serviceAccountsData, isLoading: isLoadingServiceAccounts } = useQuery({
    ...serviceAccountListOptions({ path: { org_slug: saOrgSlug || "" } }),
    enabled: ownerType === "service_account" && !!saOrgSlug,
  });

  // Reset service_account_id when org changes
  useEffect(() => {
    if (ownerType === "service_account") {
      form.setValue("service_account_id", "");
    }
  }, [saOrgSlug, ownerType, form]);

  // Reset form when modal opens
  useEffect(() => {
    if (isOpen) {
      form.reset(defaultValues);
      setAdvancedOpen(false);
    }
  }, [isOpen, form]);

  const handleSubmit = form.handleSubmit((data) => {
    let owner: ApiKeyOwner;
    if (data.ownerType === "organization") {
      owner = { type: "organization", org_id: data.org_id! };
    } else if (data.ownerType === "service_account") {
      owner = { type: "service_account", service_account_id: data.service_account_id! };
    } else {
      owner = { type: "user", user_id: data.user_id! };
    }

    // Parse allowed_models from comma-separated string
    const allowedModels = data.allowed_models
      ? data.allowed_models
          .split(",")
          .map((m) => m.trim())
          .filter(Boolean)
      : null;

    // Parse ip_allowlist from newline-separated string
    const ipAllowlist = data.ip_allowlist
      ? data.ip_allowlist
          .split("\n")
          .map((ip) => ip.trim())
          .filter(Boolean)
      : null;

    const body: CreateApiKey = {
      name: data.name,
      owner,
      budget_limit_cents: data.budget_limit_cents ? parseInt(data.budget_limit_cents) * 100 : null,
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
              htmlFor="apikey-name"
              required
              error={form.formState.errors.name?.message}
            >
              <Input id="apikey-name" {...form.register("name")} placeholder="My API Key" />
            </FormField>

            <FormField label="Owner Type" htmlFor="apikey-ownerType">
              <Controller
                name="ownerType"
                control={form.control}
                render={({ field }) => (
                  <select
                    id="apikey-ownerType"
                    {...field}
                    className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
                  >
                    <option value="organization">Organization</option>
                    <option value="user">User</option>
                    <option value="service_account">Service Account</option>
                  </select>
                )}
              />
            </FormField>

            {ownerType === "organization" && organizations && (
              <FormField
                label="Organization"
                htmlFor="apikey-org"
                required
                error={form.formState.errors.org_id?.message}
              >
                <Controller
                  name="org_id"
                  control={form.control}
                  render={({ field }) => (
                    <select
                      id="apikey-org"
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

            {ownerType === "user" && (
              <FormField
                label="User ID"
                htmlFor="apikey-user"
                required
                error={form.formState.errors.user_id?.message}
              >
                <Input id="apikey-user" {...form.register("user_id")} placeholder="Enter user ID" />
              </FormField>
            )}

            {ownerType === "service_account" && organizations && (
              <>
                <FormField
                  label="Organization"
                  htmlFor="apikey-sa-org"
                  required
                  error={form.formState.errors.sa_org_slug?.message}
                >
                  <Controller
                    name="sa_org_slug"
                    control={form.control}
                    render={({ field }) => (
                      <select
                        id="apikey-sa-org"
                        {...field}
                        className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
                      >
                        <option value="">Select organization...</option>
                        {organizations.map((org) => (
                          <option key={org.id} value={org.slug}>
                            {org.name}
                          </option>
                        ))}
                      </select>
                    )}
                  />
                </FormField>

                <FormField
                  label="Service Account"
                  htmlFor="apikey-service-account"
                  required
                  error={form.formState.errors.service_account_id?.message}
                >
                  <Controller
                    name="service_account_id"
                    control={form.control}
                    render={({ field }) => (
                      <select
                        id="apikey-service-account"
                        {...field}
                        disabled={!saOrgSlug || isLoadingServiceAccounts}
                        className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm disabled:opacity-50"
                      >
                        <option value="">
                          {!saOrgSlug
                            ? "Select an organization first..."
                            : isLoadingServiceAccounts
                              ? "Loading..."
                              : "Select service account..."}
                        </option>
                        {serviceAccountsData?.data?.map((sa) => (
                          <option key={sa.id} value={sa.id}>
                            {sa.name} ({sa.slug})
                          </option>
                        ))}
                      </select>
                    )}
                  />
                </FormField>
              </>
            )}

            <div className="grid grid-cols-2 gap-4">
              <FormField
                label="Budget Limit ($)"
                htmlFor="apikey-budget"
                error={form.formState.errors.budget_limit_cents?.message}
              >
                <Input
                  id="apikey-budget"
                  type="number"
                  min="0"
                  step="0.01"
                  {...form.register("budget_limit_cents")}
                  placeholder="100.00"
                />
              </FormField>
              <FormField
                label="Budget Period"
                htmlFor="apikey-period"
                error={form.formState.errors.budget_period?.message}
              >
                <Controller
                  name="budget_period"
                  control={form.control}
                  render={({ field }) => (
                    <select
                      id="apikey-period"
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
              htmlFor="apikey-expires"
              helpText="Leave empty for no expiration"
              error={form.formState.errors.expires_at?.message}
            >
              <Input id="apikey-expires" type="datetime-local" {...form.register("expires_at")} />
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
                    htmlFor="apikey-scopes"
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
                    htmlFor="apikey-models"
                    helpText="Comma-separated. Supports wildcards: gpt-4, claude-*, anthropic/*"
                    error={form.formState.errors.allowed_models?.message}
                  >
                    <Input
                      id="apikey-models"
                      {...form.register("allowed_models")}
                      placeholder="gpt-4, claude-*, anthropic/claude-3-*"
                    />
                  </FormField>

                  {/* IP Allowlist */}
                  <FormField
                    label="IP Allowlist"
                    htmlFor="apikey-ips"
                    helpText="One IP or CIDR per line. Leave empty to allow all IPs."
                    error={form.formState.errors.ip_allowlist?.message}
                  >
                    <Textarea
                      id="apikey-ips"
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
                      htmlFor="apikey-rpm"
                      helpText="Override global limit"
                      error={form.formState.errors.rate_limit_rpm?.message}
                    >
                      <Input
                        id="apikey-rpm"
                        type="number"
                        min="1"
                        {...form.register("rate_limit_rpm")}
                        placeholder="Default"
                      />
                    </FormField>
                    <FormField
                      label="Tokens/min"
                      htmlFor="apikey-tpm"
                      helpText="Override global limit"
                      error={form.formState.errors.rate_limit_tpm?.message}
                    >
                      <Input
                        id="apikey-tpm"
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
