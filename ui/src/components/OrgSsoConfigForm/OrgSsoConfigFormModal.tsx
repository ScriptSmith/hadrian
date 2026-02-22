import { zodResolver } from "@hookform/resolvers/zod";
import { useMutation, useQuery } from "@tanstack/react-query";
import { useEffect, useState, useRef } from "react";
import { useForm, Controller } from "react-hook-form";
import { z } from "zod";
import { Info, Loader2, Copy, Check, Download, Upload } from "lucide-react";

import type {
  OrgSsoConfig,
  CreateOrgSsoConfig,
  UpdateOrgSsoConfig,
  Team,
  SsoProviderType,
  SsoEnforcementMode,
} from "@/api/generated/types.gen";
import { orgSsoConfigParseSamlMetadata, orgSsoConfigGetSpMetadata } from "@/api/generated/sdk.gen";
import { Button } from "@/components/Button/Button";
import { FormField } from "@/components/FormField/FormField";
import { Input } from "@/components/Input/Input";
import { Modal, ModalHeader, ModalContent, ModalFooter } from "@/components/Modal/Modal";
import { Select } from "@/components/Select/Select";
import { Switch } from "@/components/Switch/Switch";
import { Textarea } from "@/components/Textarea/Textarea";
import { useToast } from "@/components/Toast/Toast";

// Provider type options
const PROVIDER_TYPE_OPTIONS = [
  { value: "oidc", label: "OpenID Connect (OIDC)" },
  { value: "saml", label: "SAML 2.0" },
];

// Enforcement mode options
const ENFORCEMENT_MODE_OPTIONS = [
  {
    value: "optional",
    label: "Optional",
    description: "Users can authenticate via other methods",
  },
  { value: "required", label: "Required", description: "Users must authenticate via this SSO" },
  {
    value: "test",
    label: "Test Mode",
    description: "SSO validation runs but doesn't block access",
  },
];

// Role options
const ROLE_OPTIONS = [
  { value: "admin", label: "Admin" },
  { value: "member", label: "Member" },
  { value: "viewer", label: "Viewer" },
];

// NameID format options
const NAME_ID_FORMAT_OPTIONS = [
  { value: "", label: "Default (IdP chooses)" },
  { value: "urn:oasis:names:tc:SAML:1.1:nameid-format:emailAddress", label: "Email Address" },
  { value: "urn:oasis:names:tc:SAML:1.1:nameid-format:unspecified", label: "Unspecified" },
  { value: "urn:oasis:names:tc:SAML:2.0:nameid-format:persistent", label: "Persistent" },
  { value: "urn:oasis:names:tc:SAML:2.0:nameid-format:transient", label: "Transient" },
];

// Domain validation regex (matches AddDomainModal pattern)
const DOMAIN_REGEX = /^(?:[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?\.)+[a-zA-Z]{2,}$/;

// Form schema with conditional validation for OIDC vs SAML
const ssoConfigSchema = z
  .object({
    provider_type: z.enum(["oidc", "saml"] as const),
    // OIDC fields
    issuer: z.string().optional().or(z.literal("")),
    discovery_url: z.string().url("Must be a valid URL").optional().or(z.literal("")),
    client_id: z.string().optional().or(z.literal("")),
    client_secret: z.string().optional().or(z.literal("")),
    redirect_uri: z.string().url("Must be a valid URL").optional().or(z.literal("")),
    scopes: z.string().optional().or(z.literal("")),
    identity_claim: z.string().optional().or(z.literal("")),
    org_claim: z.string().optional(),
    groups_claim: z.string().optional(),
    // SAML fields
    saml_metadata_url: z.string().url("Must be a valid URL").optional().or(z.literal("")),
    saml_idp_entity_id: z.string().optional().or(z.literal("")),
    saml_idp_sso_url: z.string().url("Must be a valid URL").optional().or(z.literal("")),
    saml_idp_slo_url: z.string().url("Must be a valid URL").optional().or(z.literal("")),
    saml_idp_certificate: z.string().optional().or(z.literal("")),
    saml_sp_entity_id: z.string().optional().or(z.literal("")),
    saml_sp_private_key: z.string().optional().or(z.literal("")),
    saml_name_id_format: z.string().optional().or(z.literal("")),
    saml_sign_requests: z.boolean(),
    saml_force_authn: z.boolean(),
    saml_authn_context_class_ref: z.string().optional().or(z.literal("")),
    saml_identity_attribute: z.string().optional().or(z.literal("")),
    saml_email_attribute: z.string().optional().or(z.literal("")),
    saml_name_attribute: z.string().optional().or(z.literal("")),
    saml_groups_attribute: z.string().optional().or(z.literal("")),
    // Common fields
    provisioning_enabled: z.boolean(),
    create_users: z.boolean(),
    default_team_id: z.string().nullable().optional(),
    default_org_role: z.string().min(1, "Default org role is required"),
    default_team_role: z.string().min(1, "Default team role is required"),
    allowed_email_domains: z
      .string()
      .optional()
      .refine(
        (value) => {
          if (!value || value.trim() === "") return true;
          const domains = value
            .split(",")
            .map((d) => d.trim())
            .filter((d) => d !== "");
          return domains.every((domain) => DOMAIN_REGEX.test(domain));
        },
        { message: "Please enter valid domains (e.g., acme.com, acme.org)" }
      ),
    sync_attributes_on_login: z.boolean(),
    sync_memberships_on_login: z.boolean(),
    enforcement_mode: z.enum(["optional", "required", "test"] as const),
    enabled: z.boolean(),
  })
  .superRefine((data, ctx) => {
    if (data.provider_type === "oidc") {
      // OIDC validation
      if (!data.issuer) {
        ctx.addIssue({
          code: z.ZodIssueCode.custom,
          path: ["issuer"],
          message: "Issuer URL is required",
        });
      } else {
        try {
          new URL(data.issuer);
        } catch {
          ctx.addIssue({
            code: z.ZodIssueCode.custom,
            path: ["issuer"],
            message: "Must be a valid URL",
          });
        }
      }
      if (!data.client_id) {
        ctx.addIssue({
          code: z.ZodIssueCode.custom,
          path: ["client_id"],
          message: "Client ID is required",
        });
      }
      if (!data.identity_claim) {
        ctx.addIssue({
          code: z.ZodIssueCode.custom,
          path: ["identity_claim"],
          message: "Identity claim is required",
        });
      }
      // Validate scopes include openid
      if (data.scopes && data.scopes.trim() !== "") {
        const scopes = data.scopes.split(/\s+/).map((s) => s.trim().toLowerCase());
        if (!scopes.includes("openid")) {
          ctx.addIssue({
            code: z.ZodIssueCode.custom,
            path: ["scopes"],
            message: "The 'openid' scope is required for OIDC",
          });
        }
      }
    } else if (data.provider_type === "saml") {
      // SAML validation
      // SP entity ID always required
      if (!data.saml_sp_entity_id) {
        ctx.addIssue({
          code: z.ZodIssueCode.custom,
          path: ["saml_sp_entity_id"],
          message: "SP Entity ID is required",
        });
      }
      // Identity attribute always required
      if (!data.saml_identity_attribute) {
        ctx.addIssue({
          code: z.ZodIssueCode.custom,
          path: ["saml_identity_attribute"],
          message: "Identity attribute is required",
        });
      }
      // Manual config required if no metadata URL
      if (!data.saml_metadata_url) {
        if (!data.saml_idp_entity_id) {
          ctx.addIssue({
            code: z.ZodIssueCode.custom,
            path: ["saml_idp_entity_id"],
            message: "IdP Entity ID is required when not using metadata URL",
          });
        }
        if (!data.saml_idp_sso_url) {
          ctx.addIssue({
            code: z.ZodIssueCode.custom,
            path: ["saml_idp_sso_url"],
            message: "IdP SSO URL is required when not using metadata URL",
          });
        }
        if (!data.saml_idp_certificate) {
          ctx.addIssue({
            code: z.ZodIssueCode.custom,
            path: ["saml_idp_certificate"],
            message: "IdP Certificate is required when not using metadata URL",
          });
        }
      }
    }
  });

type FormValues = z.infer<typeof ssoConfigSchema>;

interface OrgSsoConfigFormModalProps {
  open: boolean;
  onClose: () => void;
  onCreateSubmit: (data: CreateOrgSsoConfig) => void;
  onUpdateSubmit: (data: UpdateOrgSsoConfig) => void;
  isLoading: boolean;
  editingConfig: OrgSsoConfig | null;
  teams: Team[];
  orgSlug: string;
}

const DEFAULT_VALUES: FormValues = {
  provider_type: "oidc",
  // OIDC defaults
  issuer: "",
  discovery_url: "",
  client_id: "",
  client_secret: "",
  redirect_uri: "",
  scopes: "openid email profile",
  identity_claim: "sub",
  org_claim: "",
  groups_claim: "",
  // SAML defaults
  saml_metadata_url: "",
  saml_idp_entity_id: "",
  saml_idp_sso_url: "",
  saml_idp_slo_url: "",
  saml_idp_certificate: "",
  saml_sp_entity_id: "",
  saml_sp_private_key: "",
  saml_name_id_format: "",
  saml_sign_requests: false,
  saml_force_authn: false,
  saml_authn_context_class_ref: "",
  saml_identity_attribute: "urn:oid:0.9.2342.19200300.100.1.1", // uid
  saml_email_attribute: "urn:oid:0.9.2342.19200300.100.1.3", // mail
  saml_name_attribute: "",
  saml_groups_attribute: "",
  // Common defaults
  provisioning_enabled: true,
  create_users: true,
  default_team_id: null,
  default_org_role: "member",
  default_team_role: "member",
  allowed_email_domains: "",
  sync_attributes_on_login: false,
  sync_memberships_on_login: true,
  enforcement_mode: "optional",
  enabled: true,
};

export function OrgSsoConfigFormModal({
  open,
  onClose,
  onCreateSubmit,
  onUpdateSubmit,
  isLoading,
  editingConfig,
  teams,
  orgSlug,
}: OrgSsoConfigFormModalProps) {
  const isEditing = !!editingConfig;
  const [samlConfigMode, setSamlConfigMode] = useState<"metadata" | "manual">("metadata");
  const [spMetadataCopied, setSpMetadataCopied] = useState(false);
  const certInputRef = useRef<HTMLInputElement>(null);
  const keyInputRef = useRef<HTMLInputElement>(null);
  const toast = useToast();

  const form = useForm<FormValues>({
    resolver: zodResolver(ssoConfigSchema),
    defaultValues: DEFAULT_VALUES,
  });

  const providerType = form.watch("provider_type");

  // SAML metadata parsing mutation
  const parseSamlMetadataMutation = useMutation({
    mutationFn: async (metadataUrl: string) => {
      const { data, error } = await orgSsoConfigParseSamlMetadata({
        path: { org_slug: orgSlug },
        body: { metadata_url: metadataUrl },
      });
      if (error) {
        // ErrorResponse has { error: ErrorInfo } where ErrorInfo has { message: string }
        const errorMessage = error.error?.message || "Failed to parse metadata";
        throw new Error(errorMessage);
      }
      return data;
    },
    onSuccess: (data) => {
      form.setValue("saml_idp_entity_id", data.entity_id);
      form.setValue("saml_idp_sso_url", data.sso_url);
      form.setValue("saml_idp_slo_url", data.slo_url ?? "");
      form.setValue("saml_idp_certificate", data.certificates[0] ?? "");
      // Set name ID format if available
      if (data.name_id_formats.length > 0) {
        const preferredFormat = data.name_id_formats.find((f) =>
          NAME_ID_FORMAT_OPTIONS.some((opt) => opt.value === f)
        );
        if (preferredFormat) {
          form.setValue("saml_name_id_format", preferredFormat);
        }
      }
      toast.success("IdP metadata loaded successfully");
    },
    onError: (error: Error) => {
      toast.error(error.message || "Failed to parse metadata");
    },
  });

  // SP metadata query - only fetch when editing a SAML config
  const spMetadataQuery = useQuery({
    queryKey: ["sp-metadata", orgSlug],
    queryFn: async () => {
      const { data, error } = await orgSsoConfigGetSpMetadata({
        path: { org_slug: orgSlug },
      });
      if (error) {
        throw new Error("Failed to fetch SP metadata");
      }
      // The endpoint returns XML as text, cast from unknown
      return data as string;
    },
    enabled: isEditing && editingConfig?.provider_type === "saml",
  });

  const handleCopySpMetadata = async () => {
    const metadata = spMetadataQuery.data;
    if (metadata) {
      await navigator.clipboard.writeText(metadata);
      setSpMetadataCopied(true);
      setTimeout(() => setSpMetadataCopied(false), 2000);
    }
  };

  const handleDownloadSpMetadata = () => {
    const metadata = spMetadataQuery.data;
    if (metadata) {
      const blob = new Blob([metadata], { type: "application/xml" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `sp-metadata-${orgSlug}.xml`;
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      URL.revokeObjectURL(url);
    }
  };

  const handleCertUpload = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (file) {
      const reader = new FileReader();
      reader.onload = (event) => {
        const content = event.target?.result as string;
        form.setValue("saml_idp_certificate", content);
      };
      reader.readAsText(file);
    }
    // Reset input so same file can be selected again
    e.target.value = "";
  };

  const handleKeyUpload = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (file) {
      const reader = new FileReader();
      reader.onload = (event) => {
        const content = event.target?.result as string;
        form.setValue("saml_sp_private_key", content);
      };
      reader.readAsText(file);
    }
    e.target.value = "";
  };

  const handleFetchMetadata = () => {
    const metadataUrl = form.getValues("saml_metadata_url");
    if (!metadataUrl) {
      toast.error("Please enter a metadata URL");
      return;
    }
    parseSamlMetadataMutation.mutate(metadataUrl);
  };

  // Reset form when modal opens or editing config changes
  useEffect(() => {
    if (open) {
      if (editingConfig) {
        // Determine SAML config mode based on existing data
        if (editingConfig.saml_metadata_url) {
          setSamlConfigMode("metadata");
        } else if (editingConfig.saml_idp_entity_id) {
          setSamlConfigMode("manual");
        }

        form.reset({
          provider_type: editingConfig.provider_type,
          // OIDC fields
          issuer: editingConfig.issuer ?? "",
          discovery_url: editingConfig.discovery_url ?? "",
          client_id: editingConfig.client_id ?? "",
          client_secret: "", // Never pre-fill secrets
          redirect_uri: editingConfig.redirect_uri ?? "",
          scopes: editingConfig.scopes?.join(" ") ?? "openid email profile",
          identity_claim: editingConfig.identity_claim ?? "sub",
          org_claim: editingConfig.org_claim ?? "",
          groups_claim: editingConfig.groups_claim ?? "",
          // SAML fields
          saml_metadata_url: editingConfig.saml_metadata_url ?? "",
          saml_idp_entity_id: editingConfig.saml_idp_entity_id ?? "",
          saml_idp_sso_url: editingConfig.saml_idp_sso_url ?? "",
          saml_idp_slo_url: editingConfig.saml_idp_slo_url ?? "",
          saml_idp_certificate: editingConfig.saml_idp_certificate ?? "",
          saml_sp_entity_id: editingConfig.saml_sp_entity_id ?? "",
          saml_sp_private_key: "", // Never pre-fill secrets
          saml_name_id_format: editingConfig.saml_name_id_format ?? "",
          saml_sign_requests: editingConfig.saml_sign_requests ?? false,
          saml_force_authn: editingConfig.saml_force_authn ?? false,
          saml_authn_context_class_ref: editingConfig.saml_authn_context_class_ref ?? "",
          saml_identity_attribute:
            editingConfig.saml_identity_attribute ?? "urn:oid:0.9.2342.19200300.100.1.1",
          saml_email_attribute:
            editingConfig.saml_email_attribute ?? "urn:oid:0.9.2342.19200300.100.1.3",
          saml_name_attribute: editingConfig.saml_name_attribute ?? "",
          saml_groups_attribute: editingConfig.saml_groups_attribute ?? "",
          // Common fields
          provisioning_enabled: editingConfig.provisioning_enabled,
          create_users: editingConfig.create_users,
          default_team_id: editingConfig.default_team_id ?? null,
          default_org_role: editingConfig.default_org_role,
          default_team_role: editingConfig.default_team_role,
          allowed_email_domains: editingConfig.allowed_email_domains.join(", "),
          sync_attributes_on_login: editingConfig.sync_attributes_on_login,
          sync_memberships_on_login: editingConfig.sync_memberships_on_login,
          enforcement_mode: editingConfig.enforcement_mode,
          enabled: editingConfig.enabled,
        });
      } else {
        form.reset(DEFAULT_VALUES);
        setSamlConfigMode("metadata");
      }
    }
  }, [open, editingConfig, form]);

  const handleSubmit = form.handleSubmit((data) => {
    // Validate client_secret for OIDC create mode
    if (!isEditing && data.provider_type === "oidc" && !data.client_secret) {
      form.setError("client_secret", { message: "Client secret is required" });
      return;
    }

    // Parse scopes from space-separated string
    const scopes = data.scopes
      ? data.scopes
          .split(/\s+/)
          .map((s: string) => s.trim())
          .filter((s: string) => s.length > 0)
      : ["openid", "email", "profile"];

    // Parse email domains from comma-separated string
    const emailDomains = data.allowed_email_domains
      ? data.allowed_email_domains
          .split(",")
          .map((d: string) => d.trim())
          .filter((d: string) => d.length > 0)
      : [];

    if (isEditing) {
      const updateData: UpdateOrgSsoConfig = {
        provider_type: data.provider_type as SsoProviderType,
        // Common fields
        provisioning_enabled: data.provisioning_enabled,
        create_users: data.create_users,
        default_team_id: data.default_team_id || null,
        default_org_role: data.default_org_role,
        default_team_role: data.default_team_role,
        allowed_email_domains: emailDomains,
        sync_attributes_on_login: data.sync_attributes_on_login,
        sync_memberships_on_login: data.sync_memberships_on_login,
        enforcement_mode: data.enforcement_mode as SsoEnforcementMode,
      };

      if (data.provider_type === "oidc") {
        // OIDC-specific fields
        updateData.issuer = data.issuer || null;
        updateData.discovery_url = data.discovery_url || null;
        updateData.client_id = data.client_id || null;
        updateData.redirect_uri = data.redirect_uri || null;
        updateData.scopes = scopes;
        updateData.identity_claim = data.identity_claim || null;
        updateData.org_claim = data.org_claim || null;
        updateData.groups_claim = data.groups_claim || null;
        // Only include client_secret if provided (rotating secret)
        if (data.client_secret) {
          updateData.client_secret = data.client_secret;
        }
      } else {
        // SAML-specific fields
        updateData.saml_metadata_url = data.saml_metadata_url || null;
        updateData.saml_idp_entity_id = data.saml_idp_entity_id || null;
        updateData.saml_idp_sso_url = data.saml_idp_sso_url || null;
        updateData.saml_idp_slo_url = data.saml_idp_slo_url || null;
        updateData.saml_idp_certificate = data.saml_idp_certificate || null;
        updateData.saml_sp_entity_id = data.saml_sp_entity_id || null;
        updateData.saml_name_id_format = data.saml_name_id_format || null;
        updateData.saml_sign_requests = data.saml_sign_requests;
        updateData.saml_force_authn = data.saml_force_authn;
        updateData.saml_authn_context_class_ref = data.saml_authn_context_class_ref || null;
        updateData.saml_identity_attribute = data.saml_identity_attribute || null;
        updateData.saml_email_attribute = data.saml_email_attribute || null;
        updateData.saml_name_attribute = data.saml_name_attribute || null;
        updateData.saml_groups_attribute = data.saml_groups_attribute || null;
        // Only include private key if provided
        if (data.saml_sp_private_key) {
          updateData.saml_sp_private_key = data.saml_sp_private_key;
        }
      }
      onUpdateSubmit(updateData);
    } else {
      const createData: CreateOrgSsoConfig = {
        provider_type: data.provider_type as SsoProviderType,
        // Common fields
        provisioning_enabled: data.provisioning_enabled,
        create_users: data.create_users,
        default_team_id: data.default_team_id || undefined,
        default_org_role: data.default_org_role,
        default_team_role: data.default_team_role,
        allowed_email_domains: emailDomains,
        sync_attributes_on_login: data.sync_attributes_on_login,
        sync_memberships_on_login: data.sync_memberships_on_login,
        enforcement_mode: data.enforcement_mode as SsoEnforcementMode,
        enabled: data.enabled,
      };

      if (data.provider_type === "oidc") {
        // OIDC-specific fields
        createData.issuer = data.issuer || undefined;
        createData.discovery_url = data.discovery_url || undefined;
        createData.client_id = data.client_id || undefined;
        createData.client_secret = data.client_secret!;
        createData.redirect_uri = data.redirect_uri || undefined;
        createData.scopes = scopes;
        createData.identity_claim = data.identity_claim || undefined;
        createData.org_claim = data.org_claim || undefined;
        createData.groups_claim = data.groups_claim || undefined;
      } else {
        // SAML-specific fields
        createData.saml_metadata_url = data.saml_metadata_url || undefined;
        createData.saml_idp_entity_id = data.saml_idp_entity_id || undefined;
        createData.saml_idp_sso_url = data.saml_idp_sso_url || undefined;
        createData.saml_idp_slo_url = data.saml_idp_slo_url || undefined;
        createData.saml_idp_certificate = data.saml_idp_certificate || undefined;
        createData.saml_sp_entity_id = data.saml_sp_entity_id || undefined;
        createData.saml_sp_private_key = data.saml_sp_private_key || undefined;
        createData.saml_name_id_format = data.saml_name_id_format || undefined;
        createData.saml_sign_requests = data.saml_sign_requests;
        createData.saml_force_authn = data.saml_force_authn;
        createData.saml_authn_context_class_ref = data.saml_authn_context_class_ref || undefined;
        createData.saml_identity_attribute = data.saml_identity_attribute || undefined;
        createData.saml_email_attribute = data.saml_email_attribute || undefined;
        createData.saml_name_attribute = data.saml_name_attribute || undefined;
        createData.saml_groups_attribute = data.saml_groups_attribute || undefined;
      }
      onCreateSubmit(createData);
    }
  });

  const teamOptions = [
    { value: "", label: "None (org-level only)" },
    ...teams.map((team) => ({
      value: team.id,
      label: team.name,
    })),
  ];

  return (
    <Modal open={open} onClose={onClose}>
      <form onSubmit={handleSubmit}>
        <ModalHeader>{isEditing ? "Edit SSO Configuration" : "Configure SSO"}</ModalHeader>
        <ModalContent className="space-y-6 max-h-[70vh] overflow-y-auto">
          {/* Provider Type Selection */}
          <div className="space-y-4">
            <h3 className="text-sm font-semibold text-foreground">Provider Type</h3>

            <FormField
              label="Provider Type"
              htmlFor="provider_type"
              error={form.formState.errors.provider_type?.message}
            >
              <Controller
                name="provider_type"
                control={form.control}
                render={({ field }) => (
                  <Select
                    value={field.value}
                    onChange={field.onChange}
                    options={PROVIDER_TYPE_OPTIONS}
                  />
                )}
              />
            </FormField>
          </div>

          {/* OIDC Provider Settings */}
          {providerType === "oidc" && (
            <div className="space-y-4">
              <h3 className="text-sm font-semibold text-foreground">OIDC Provider Settings</h3>

              <FormField
                label="Issuer URL"
                htmlFor="issuer"
                required
                error={form.formState.errors.issuer?.message}
                helpText='OIDC issuer URL (e.g., "https://accounts.google.com")'
              >
                <Input
                  id="issuer"
                  {...form.register("issuer")}
                  placeholder="https://your-idp.com"
                />
              </FormField>

              <FormField
                label="Discovery URL"
                htmlFor="discovery_url"
                error={form.formState.errors.discovery_url?.message}
                helpText="Optional. Defaults to issuer/.well-known/openid-configuration"
              >
                <Input
                  id="discovery_url"
                  {...form.register("discovery_url")}
                  placeholder="https://your-idp.com/.well-known/openid-configuration"
                />
              </FormField>

              <div className="grid grid-cols-2 gap-4">
                <FormField
                  label="Client ID"
                  htmlFor="client_id"
                  required
                  error={form.formState.errors.client_id?.message}
                >
                  <Input
                    id="client_id"
                    {...form.register("client_id")}
                    placeholder="your-client-id"
                  />
                </FormField>

                <FormField
                  label="Client Secret"
                  htmlFor="client_secret"
                  required={!isEditing}
                  error={form.formState.errors.client_secret?.message}
                  helpText={isEditing ? "Leave blank to keep existing secret" : undefined}
                >
                  <Input
                    id="client_secret"
                    type="password"
                    {...form.register("client_secret")}
                    placeholder={isEditing ? "Leave blank to keep existing" : "your-client-secret"}
                  />
                </FormField>
              </div>

              <FormField
                label="Redirect URI"
                htmlFor="redirect_uri"
                error={form.formState.errors.redirect_uri?.message}
                helpText="Optional. Uses global default if not set"
              >
                <Input
                  id="redirect_uri"
                  {...form.register("redirect_uri")}
                  placeholder="https://your-gateway.com/auth/callback"
                />
              </FormField>

              <FormField
                label="Scopes"
                htmlFor="scopes"
                error={form.formState.errors.scopes?.message}
                helpText='Space-separated OAuth2 scopes. The "openid" scope is required.'
              >
                <Input
                  id="scopes"
                  {...form.register("scopes")}
                  placeholder="openid email profile groups"
                />
              </FormField>
            </div>
          )}

          {/* OIDC Token Claims */}
          {providerType === "oidc" && (
            <div className="space-y-4">
              <h3 className="text-sm font-semibold text-foreground">Token Claims</h3>

              <div className="grid grid-cols-3 gap-4">
                <FormField
                  label="Identity Claim"
                  htmlFor="identity_claim"
                  required
                  error={form.formState.errors.identity_claim?.message}
                  helpText='JWT claim for user identity (default: "sub")'
                >
                  <Input
                    id="identity_claim"
                    {...form.register("identity_claim")}
                    placeholder="sub"
                  />
                </FormField>

                <FormField
                  label="Org Claim"
                  htmlFor="org_claim"
                  error={form.formState.errors.org_claim?.message}
                  helpText="JWT claim for organization IDs"
                >
                  <Input id="org_claim" {...form.register("org_claim")} placeholder="org" />
                </FormField>

                <FormField
                  label="Groups Claim"
                  htmlFor="groups_claim"
                  error={form.formState.errors.groups_claim?.message}
                  helpText="JWT claim for group memberships"
                >
                  <Input
                    id="groups_claim"
                    {...form.register("groups_claim")}
                    placeholder="groups"
                  />
                </FormField>
              </div>
            </div>
          )}

          {/* SAML IdP Configuration */}
          {providerType === "saml" && (
            <div className="space-y-4">
              <h3 className="text-sm font-semibold text-foreground">Identity Provider (IdP)</h3>

              <div className="flex items-start gap-3 rounded-lg border bg-muted/30 p-3">
                <Info className="h-5 w-5 text-muted-foreground mt-0.5 flex-shrink-0" />
                <p className="text-sm text-muted-foreground">
                  You can configure your IdP by providing a metadata URL (recommended) or by
                  entering the configuration manually.
                </p>
              </div>

              <div className="flex gap-4 border-b pb-2">
                <button
                  type="button"
                  className={`text-sm font-medium pb-2 border-b-2 transition-colors ${
                    samlConfigMode === "metadata"
                      ? "border-primary text-primary"
                      : "border-transparent text-muted-foreground hover:text-foreground"
                  }`}
                  onClick={() => setSamlConfigMode("metadata")}
                >
                  Metadata URL
                </button>
                <button
                  type="button"
                  className={`text-sm font-medium pb-2 border-b-2 transition-colors ${
                    samlConfigMode === "manual"
                      ? "border-primary text-primary"
                      : "border-transparent text-muted-foreground hover:text-foreground"
                  }`}
                  onClick={() => setSamlConfigMode("manual")}
                >
                  Manual Configuration
                </button>
              </div>

              {samlConfigMode === "metadata" && (
                <div className="space-y-4">
                  <FormField
                    label="Metadata URL"
                    htmlFor="saml_metadata_url"
                    error={form.formState.errors.saml_metadata_url?.message}
                    helpText="URL to your IdP's SAML metadata (must be HTTPS)"
                  >
                    <div className="flex gap-2">
                      <Input
                        id="saml_metadata_url"
                        {...form.register("saml_metadata_url")}
                        placeholder="https://idp.example.com/metadata.xml"
                        className="flex-1"
                      />
                      <Button
                        type="button"
                        variant="secondary"
                        onClick={handleFetchMetadata}
                        disabled={parseSamlMetadataMutation.isPending}
                      >
                        {parseSamlMetadataMutation.isPending ? (
                          <>
                            <Loader2 className="h-4 w-4 animate-spin mr-2" />
                            Fetching...
                          </>
                        ) : (
                          "Fetch & Auto-Fill"
                        )}
                      </Button>
                    </div>
                  </FormField>

                  <p className="text-xs text-muted-foreground">
                    Click &quot;Fetch & Auto-Fill&quot; to automatically populate the IdP fields
                    below. You can also manually edit the fields after fetching.
                  </p>
                </div>
              )}

              <FormField
                label="IdP Entity ID"
                htmlFor="saml_idp_entity_id"
                required={samlConfigMode === "manual"}
                error={form.formState.errors.saml_idp_entity_id?.message}
                helpText="Unique identifier for your IdP"
              >
                <Input
                  id="saml_idp_entity_id"
                  {...form.register("saml_idp_entity_id")}
                  placeholder="https://idp.example.com/entity"
                />
              </FormField>

              <div className="grid grid-cols-2 gap-4">
                <FormField
                  label="IdP SSO URL"
                  htmlFor="saml_idp_sso_url"
                  required={samlConfigMode === "manual"}
                  error={form.formState.errors.saml_idp_sso_url?.message}
                  helpText="Single Sign-On service URL"
                >
                  <Input
                    id="saml_idp_sso_url"
                    {...form.register("saml_idp_sso_url")}
                    placeholder="https://idp.example.com/sso"
                  />
                </FormField>

                <FormField
                  label="IdP SLO URL"
                  htmlFor="saml_idp_slo_url"
                  error={form.formState.errors.saml_idp_slo_url?.message}
                  helpText="Single Logout service URL (optional)"
                >
                  <Input
                    id="saml_idp_slo_url"
                    {...form.register("saml_idp_slo_url")}
                    placeholder="https://idp.example.com/slo"
                  />
                </FormField>
              </div>

              <FormField
                label={
                  <span className="flex items-center gap-2">
                    IdP Certificate
                    <Button
                      type="button"
                      variant="ghost"
                      size="sm"
                      className="h-6 px-2 text-xs"
                      onClick={() => certInputRef.current?.click()}
                    >
                      <Upload className="h-3 w-3 mr-1" />
                      Upload
                    </Button>
                    <input
                      ref={certInputRef}
                      type="file"
                      accept=".pem,.crt,.cer"
                      className="hidden"
                      onChange={handleCertUpload}
                      aria-label="Upload IdP certificate"
                    />
                  </span>
                }
                htmlFor="saml_idp_certificate"
                required={samlConfigMode === "manual"}
                error={form.formState.errors.saml_idp_certificate?.message}
                helpText="X.509 certificate in PEM format for signature verification"
              >
                <Textarea
                  id="saml_idp_certificate"
                  {...form.register("saml_idp_certificate")}
                  placeholder="-----BEGIN CERTIFICATE-----&#10;...&#10;-----END CERTIFICATE-----"
                  rows={4}
                  className="font-mono text-xs"
                />
              </FormField>
            </div>
          )}

          {/* SAML SP Configuration */}
          {providerType === "saml" && (
            <div className="space-y-4">
              <h3 className="text-sm font-semibold text-foreground">
                Service Provider (SP) Settings
              </h3>

              <FormField
                label="SP Entity ID"
                htmlFor="saml_sp_entity_id"
                required
                error={form.formState.errors.saml_sp_entity_id?.message}
                helpText="Unique identifier for your application (e.g., https://your-app.com/saml)"
              >
                <Input
                  id="saml_sp_entity_id"
                  {...form.register("saml_sp_entity_id")}
                  placeholder="https://your-gateway.com/saml"
                />
              </FormField>

              {/* SP Metadata Panel - only shown in edit mode */}
              {isEditing && (
                <details className="rounded-lg border p-4">
                  <summary className="text-sm font-medium cursor-pointer hover:text-muted-foreground">
                    SP Metadata (for IdP Configuration)
                  </summary>
                  <div className="pt-3 space-y-3">
                    <p className="text-xs text-muted-foreground">
                      Provide this metadata XML to your IdP administrator to complete the SAML
                      integration.
                    </p>
                    {spMetadataQuery.isLoading ? (
                      <div className="flex items-center justify-center py-4">
                        <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" />
                      </div>
                    ) : spMetadataQuery.error ? (
                      <div className="text-sm text-destructive">
                        Failed to load SP metadata. Save your configuration first.
                      </div>
                    ) : spMetadataQuery.data ? (
                      <div className="relative">
                        <pre className="bg-muted p-3 rounded-md text-xs font-mono overflow-x-auto max-h-48 whitespace-pre-wrap">
                          {spMetadataQuery.data}
                        </pre>
                        <div className="absolute top-2 right-2 flex gap-1">
                          <Button
                            type="button"
                            variant="ghost"
                            size="sm"
                            onClick={handleCopySpMetadata}
                            title="Copy to clipboard"
                          >
                            {spMetadataCopied ? (
                              <Check className="h-4 w-4 text-success" />
                            ) : (
                              <Copy className="h-4 w-4" />
                            )}
                          </Button>
                          <Button
                            type="button"
                            variant="ghost"
                            size="sm"
                            onClick={handleDownloadSpMetadata}
                            title="Download as file"
                          >
                            <Download className="h-4 w-4" />
                          </Button>
                        </div>
                      </div>
                    ) : null}
                  </div>
                </details>
              )}

              <FormField
                label={
                  <span className="flex items-center gap-2">
                    SP Private Key
                    <Button
                      type="button"
                      variant="ghost"
                      size="sm"
                      className="h-6 px-2 text-xs"
                      onClick={() => keyInputRef.current?.click()}
                    >
                      <Upload className="h-3 w-3 mr-1" />
                      Upload
                    </Button>
                    <input
                      ref={keyInputRef}
                      type="file"
                      accept=".pem,.key"
                      className="hidden"
                      onChange={handleKeyUpload}
                      aria-label="Upload SP private key"
                    />
                  </span>
                }
                htmlFor="saml_sp_private_key"
                error={form.formState.errors.saml_sp_private_key?.message}
                helpText={
                  isEditing
                    ? "Leave blank to keep existing. Required for signed AuthnRequests."
                    : "Optional. Required for signing authentication requests."
                }
              >
                <Textarea
                  id="saml_sp_private_key"
                  {...form.register("saml_sp_private_key")}
                  placeholder="-----BEGIN PRIVATE KEY-----&#10;...&#10;-----END PRIVATE KEY-----"
                  rows={4}
                  className="font-mono text-xs"
                />
              </FormField>

              <div className="grid grid-cols-2 gap-4">
                <div className="flex items-center justify-between p-3 rounded-lg border">
                  <div>
                    <p className="text-sm font-medium">Sign Requests</p>
                    <p className="text-xs text-muted-foreground">Sign authentication requests</p>
                  </div>
                  <Controller
                    name="saml_sign_requests"
                    control={form.control}
                    render={({ field }) => (
                      <Switch
                        checked={field.value}
                        onChange={(e) => field.onChange(e.target.checked)}
                        aria-label="Sign requests"
                      />
                    )}
                  />
                </div>

                <div className="flex items-center justify-between p-3 rounded-lg border">
                  <div>
                    <p className="text-sm font-medium">Force Re-authentication</p>
                    <p className="text-xs text-muted-foreground">
                      Always require IdP authentication
                    </p>
                  </div>
                  <Controller
                    name="saml_force_authn"
                    control={form.control}
                    render={({ field }) => (
                      <Switch
                        checked={field.value}
                        onChange={(e) => field.onChange(e.target.checked)}
                        aria-label="Force re-authentication"
                      />
                    )}
                  />
                </div>
              </div>
            </div>
          )}

          {/* SAML Attribute Mappings */}
          {providerType === "saml" && (
            <div className="space-y-4">
              <h3 className="text-sm font-semibold text-foreground">Attribute Mappings</h3>

              <div className="flex items-start gap-3 rounded-lg border bg-muted/30 p-3">
                <Info className="h-5 w-5 text-muted-foreground mt-0.5 flex-shrink-0" />
                <p className="text-sm text-muted-foreground">
                  Map SAML assertion attributes to user properties. Use OID format (e.g.,
                  urn:oid:0.9.2342.19200300.100.1.1 for uid) or friendly names depending on your
                  IdP.
                </p>
              </div>

              <div className="grid grid-cols-2 gap-4">
                <FormField
                  label="Identity Attribute"
                  htmlFor="saml_identity_attribute"
                  required
                  error={form.formState.errors.saml_identity_attribute?.message}
                  helpText="SAML attribute for unique user identity"
                >
                  <Input
                    id="saml_identity_attribute"
                    {...form.register("saml_identity_attribute")}
                    placeholder="urn:oid:0.9.2342.19200300.100.1.1"
                  />
                </FormField>

                <FormField
                  label="Email Attribute"
                  htmlFor="saml_email_attribute"
                  error={form.formState.errors.saml_email_attribute?.message}
                  helpText="SAML attribute for user email"
                >
                  <Input
                    id="saml_email_attribute"
                    {...form.register("saml_email_attribute")}
                    placeholder="urn:oid:0.9.2342.19200300.100.1.3"
                  />
                </FormField>
              </div>

              <div className="grid grid-cols-2 gap-4">
                <FormField
                  label="Name Attribute"
                  htmlFor="saml_name_attribute"
                  error={form.formState.errors.saml_name_attribute?.message}
                  helpText="SAML attribute for display name"
                >
                  <Input
                    id="saml_name_attribute"
                    {...form.register("saml_name_attribute")}
                    placeholder="urn:oid:2.16.840.1.113730.3.1.241"
                  />
                </FormField>

                <FormField
                  label="Groups Attribute"
                  htmlFor="saml_groups_attribute"
                  error={form.formState.errors.saml_groups_attribute?.message}
                  helpText="SAML attribute for group memberships"
                >
                  <Input
                    id="saml_groups_attribute"
                    {...form.register("saml_groups_attribute")}
                    placeholder="memberOf"
                  />
                </FormField>
              </div>
            </div>
          )}

          {/* SAML Advanced Settings */}
          {providerType === "saml" && (
            <details className="space-y-4">
              <summary className="text-sm font-semibold text-foreground cursor-pointer hover:text-muted-foreground">
                Advanced Settings
              </summary>

              <div className="space-y-4 pt-4">
                <FormField
                  label="NameID Format"
                  htmlFor="saml_name_id_format"
                  error={form.formState.errors.saml_name_id_format?.message}
                  helpText="Requested NameID format for the SAML assertion"
                >
                  <Controller
                    name="saml_name_id_format"
                    control={form.control}
                    render={({ field }) => (
                      <Select
                        value={field.value}
                        onChange={field.onChange}
                        options={NAME_ID_FORMAT_OPTIONS}
                      />
                    )}
                  />
                </FormField>

                <FormField
                  label="Authentication Context Class"
                  htmlFor="saml_authn_context_class_ref"
                  error={form.formState.errors.saml_authn_context_class_ref?.message}
                  helpText="Requested authentication context (e.g., urn:oasis:names:tc:SAML:2.0:ac:classes:PasswordProtectedTransport)"
                >
                  <Input
                    id="saml_authn_context_class_ref"
                    {...form.register("saml_authn_context_class_ref")}
                    placeholder="urn:oasis:names:tc:SAML:2.0:ac:classes:PasswordProtectedTransport"
                  />
                </FormField>
              </div>
            </details>
          )}

          {/* JIT Provisioning */}
          <div className="space-y-4">
            <h3 className="text-sm font-semibold text-foreground">JIT Provisioning</h3>

            <div className="flex items-start gap-3 rounded-lg border bg-muted/30 p-3">
              <Info className="h-5 w-5 text-muted-foreground mt-0.5 flex-shrink-0" />
              <p className="text-sm text-muted-foreground">
                Just-In-Time (JIT) provisioning automatically creates users and assigns them to
                teams when they first log in via SSO.
              </p>
            </div>

            <div className="grid grid-cols-2 gap-4">
              <div className="flex items-center justify-between p-3 rounded-lg border">
                <div>
                  <p className="text-sm font-medium">Enable Provisioning</p>
                  <p className="text-xs text-muted-foreground">Automatically provision users</p>
                </div>
                <Controller
                  name="provisioning_enabled"
                  control={form.control}
                  render={({ field }) => (
                    <Switch
                      checked={field.value}
                      onChange={(e) => field.onChange(e.target.checked)}
                      aria-label="Enable provisioning"
                    />
                  )}
                />
              </div>

              <div className="flex items-center justify-between p-3 rounded-lg border">
                <div>
                  <p className="text-sm font-medium">Create Users</p>
                  <p className="text-xs text-muted-foreground">Create new users on first login</p>
                </div>
                <Controller
                  name="create_users"
                  control={form.control}
                  render={({ field }) => (
                    <Switch
                      checked={field.value}
                      onChange={(e) => field.onChange(e.target.checked)}
                      aria-label="Create users"
                    />
                  )}
                />
              </div>
            </div>

            <div className="grid grid-cols-3 gap-4">
              <FormField
                label="Default Team"
                htmlFor="default_team_id"
                helpText="Team to add new users to"
              >
                <Controller
                  name="default_team_id"
                  control={form.control}
                  render={({ field }) => (
                    <Select
                      value={field.value ?? ""}
                      onChange={(value) => field.onChange(value || null)}
                      options={teamOptions}
                    />
                  )}
                />
              </FormField>

              <FormField
                label="Default Org Role"
                htmlFor="default_org_role"
                required
                error={form.formState.errors.default_org_role?.message}
              >
                <Controller
                  name="default_org_role"
                  control={form.control}
                  render={({ field }) => (
                    <Select value={field.value} onChange={field.onChange} options={ROLE_OPTIONS} />
                  )}
                />
              </FormField>

              <FormField
                label="Default Team Role"
                htmlFor="default_team_role"
                required
                error={form.formState.errors.default_team_role?.message}
              >
                <Controller
                  name="default_team_role"
                  control={form.control}
                  render={({ field }) => (
                    <Select value={field.value} onChange={field.onChange} options={ROLE_OPTIONS} />
                  )}
                />
              </FormField>
            </div>

            <FormField
              label="Allowed Email Domains"
              htmlFor="allowed_email_domains"
              helpText="Comma-separated list of allowed domains (leave empty to allow all)"
            >
              <Input
                id="allowed_email_domains"
                {...form.register("allowed_email_domains")}
                placeholder="acme.com, acme.org"
              />
            </FormField>

            <div className="grid grid-cols-2 gap-4">
              <div className="flex items-center justify-between p-3 rounded-lg border">
                <div>
                  <p className="text-sm font-medium">Sync Attributes</p>
                  <p className="text-xs text-muted-foreground">Update email/name on each login</p>
                </div>
                <Controller
                  name="sync_attributes_on_login"
                  control={form.control}
                  render={({ field }) => (
                    <Switch
                      checked={field.value}
                      onChange={(e) => field.onChange(e.target.checked)}
                      aria-label="Sync attributes"
                    />
                  )}
                />
              </div>

              <div className="flex items-center justify-between p-3 rounded-lg border">
                <div>
                  <p className="text-sm font-medium">Sync Team Memberships</p>
                  <p className="text-xs text-muted-foreground">Sync from IdP groups on login</p>
                </div>
                <Controller
                  name="sync_memberships_on_login"
                  control={form.control}
                  render={({ field }) => (
                    <Switch
                      checked={field.value}
                      onChange={(e) => field.onChange(e.target.checked)}
                      aria-label="Sync team memberships"
                    />
                  )}
                />
              </div>
            </div>
          </div>

          {/* Enforcement */}
          <div className="space-y-4">
            <h3 className="text-sm font-semibold text-foreground">Enforcement</h3>

            <div className="flex items-start gap-3 rounded-lg border bg-muted/30 p-3">
              <Info className="h-5 w-5 text-muted-foreground mt-0.5 flex-shrink-0" />
              <div className="text-sm text-muted-foreground space-y-2">
                <p>
                  <strong className="text-foreground">Optional:</strong> Users can sign in via SSO
                  or other methods (API key, global SSO).
                </p>
                <p>
                  <strong className="text-foreground">Test Mode:</strong> Logs when users bypass SSO
                  but doesn&apos;t block them. Use this to audit impact before enforcing.
                </p>
                <p>
                  <strong className="text-foreground">Required:</strong> Users with verified email
                  domains must sign in through this SSO. Other auth methods are blocked.
                </p>
              </div>
            </div>

            <FormField
              label="Enforcement Mode"
              htmlFor="enforcement_mode"
              error={form.formState.errors.enforcement_mode?.message}
              helpText="Enforcement only applies to verified email domains"
            >
              <Controller
                name="enforcement_mode"
                control={form.control}
                render={({ field }) => (
                  <Select
                    value={field.value}
                    onChange={field.onChange}
                    options={ENFORCEMENT_MODE_OPTIONS}
                  />
                )}
              />
            </FormField>

            {!isEditing && (
              <div className="flex items-center justify-between p-3 rounded-lg border">
                <div>
                  <p className="text-sm font-medium">Enabled</p>
                  <p className="text-xs text-muted-foreground">
                    Activate this SSO configuration immediately
                  </p>
                </div>
                <Controller
                  name="enabled"
                  control={form.control}
                  render={({ field }) => (
                    <Switch
                      checked={field.value}
                      onChange={(e) => field.onChange(e.target.checked)}
                      aria-label="Enabled"
                    />
                  )}
                />
              </div>
            )}
          </div>
        </ModalContent>
        <ModalFooter>
          <Button type="button" variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button type="submit" isLoading={isLoading}>
            {isEditing ? "Save Changes" : "Create SSO Config"}
          </Button>
        </ModalFooter>
      </form>
    </Modal>
  );
}
