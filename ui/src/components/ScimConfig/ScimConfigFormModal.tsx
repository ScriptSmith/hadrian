import { zodResolver } from "@hookform/resolvers/zod";
import { useEffect } from "react";
import { useForm, Controller } from "react-hook-form";
import { z } from "zod";
import { Info } from "lucide-react";

import type {
  OrgScimConfig,
  CreateOrgScimConfig,
  UpdateOrgScimConfig,
  Team,
} from "@/api/generated/types.gen";
import { Button } from "@/components/Button/Button";
import { FormField } from "@/components/FormField/FormField";
import { Modal, ModalHeader, ModalContent, ModalFooter } from "@/components/Modal/Modal";
import { Select } from "@/components/Select/Select";
import { Switch } from "@/components/Switch/Switch";

// Role options
const ROLE_OPTIONS = [
  { value: "admin", label: "Admin" },
  { value: "member", label: "Member" },
  { value: "viewer", label: "Viewer" },
];

// Form schema
const scimConfigSchema = z.object({
  enabled: z.boolean(),
  create_users: z.boolean(),
  sync_display_name: z.boolean(),
  default_team_id: z.string().nullable().optional(),
  default_org_role: z.string().min(1, "Default org role is required"),
  default_team_role: z.string().min(1, "Default team role is required"),
  deactivate_deletes_user: z.boolean(),
  revoke_api_keys_on_deactivate: z.boolean(),
});

type FormValues = z.infer<typeof scimConfigSchema>;

interface ScimConfigFormModalProps {
  open: boolean;
  onClose: () => void;
  onCreateSubmit: (data: CreateOrgScimConfig) => void;
  onUpdateSubmit: (data: UpdateOrgScimConfig) => void;
  isLoading: boolean;
  editingConfig: OrgScimConfig | null;
  teams: Team[];
}

const DEFAULT_VALUES: FormValues = {
  enabled: true,
  create_users: true,
  sync_display_name: true,
  default_team_id: null,
  default_org_role: "member",
  default_team_role: "member",
  deactivate_deletes_user: false,
  revoke_api_keys_on_deactivate: true,
};

export function ScimConfigFormModal({
  open,
  onClose,
  onCreateSubmit,
  onUpdateSubmit,
  isLoading,
  editingConfig,
  teams,
}: ScimConfigFormModalProps) {
  const isEditing = !!editingConfig;

  const form = useForm<FormValues>({
    resolver: zodResolver(scimConfigSchema),
    defaultValues: DEFAULT_VALUES,
  });

  // Reset form when editing config changes
  useEffect(() => {
    if (editingConfig) {
      form.reset({
        enabled: editingConfig.enabled,
        create_users: editingConfig.create_users,
        sync_display_name: editingConfig.sync_display_name,
        default_team_id: editingConfig.default_team_id ?? null,
        default_org_role: editingConfig.default_org_role,
        default_team_role: editingConfig.default_team_role,
        deactivate_deletes_user: editingConfig.deactivate_deletes_user,
        revoke_api_keys_on_deactivate: editingConfig.revoke_api_keys_on_deactivate,
      });
    } else {
      form.reset(DEFAULT_VALUES);
    }
  }, [editingConfig, form]);

  const onSubmit = (data: FormValues) => {
    if (isEditing) {
      const updateData: UpdateOrgScimConfig = {
        enabled: data.enabled,
        create_users: data.create_users,
        sync_display_name: data.sync_display_name,
        default_team_id: data.default_team_id,
        default_org_role: data.default_org_role,
        default_team_role: data.default_team_role,
        deactivate_deletes_user: data.deactivate_deletes_user,
        revoke_api_keys_on_deactivate: data.revoke_api_keys_on_deactivate,
      };
      onUpdateSubmit(updateData);
    } else {
      const createData: CreateOrgScimConfig = {
        create_users: data.create_users,
        sync_display_name: data.sync_display_name,
        default_team_id: data.default_team_id,
        default_org_role: data.default_org_role,
        default_team_role: data.default_team_role,
        deactivate_deletes_user: data.deactivate_deletes_user,
        revoke_api_keys_on_deactivate: data.revoke_api_keys_on_deactivate,
      };
      onCreateSubmit(createData);
    }
  };

  const teamOptions = [
    { value: "", label: "None (no default team)" },
    ...teams.map((team) => ({
      value: team.id,
      label: team.name,
    })),
  ];

  return (
    <Modal open={open} onClose={onClose}>
      <form onSubmit={form.handleSubmit(onSubmit)}>
        <ModalHeader>{isEditing ? "Edit SCIM Configuration" : "Configure SCIM"}</ModalHeader>
        <ModalContent className="max-h-[60vh] overflow-y-auto">
          <div className="space-y-6">
            {/* Info box */}
            <div className="flex items-start gap-3 rounded-lg border bg-muted/30 p-4">
              <Info className="h-5 w-5 text-muted-foreground mt-0.5" />
              <div className="text-sm">
                <p className="font-medium">SCIM 2.0 Provisioning</p>
                <p className="text-muted-foreground">
                  Configure how users and groups are provisioned from your identity provider (Okta,
                  Azure AD, etc.) via SCIM 2.0.
                </p>
              </div>
            </div>

            {/* Enabled toggle (only show when editing) */}
            {isEditing && (
              <div className="space-y-4">
                <h3 className="font-medium text-sm">Status</h3>
                <Controller
                  name="enabled"
                  control={form.control}
                  render={({ field }) => (
                    <FormField label="Enable SCIM" htmlFor="enabled">
                      <div className="flex items-center gap-3">
                        <Switch
                          id="enabled"
                          checked={field.value}
                          onChange={(e) => field.onChange(e.target.checked)}
                        />
                        <span className="text-sm text-muted-foreground">
                          {field.value
                            ? "SCIM provisioning is active"
                            : "SCIM provisioning is paused"}
                        </span>
                      </div>
                    </FormField>
                  )}
                />
              </div>
            )}

            {/* Provisioning Settings */}
            <div className="space-y-4">
              <h3 className="font-medium text-sm">Provisioning Settings</h3>

              <Controller
                name="create_users"
                control={form.control}
                render={({ field }) => (
                  <FormField
                    label="Create Users"
                    htmlFor="create_users"
                    helpText="Automatically create new users in Hadrian when they are provisioned via SCIM"
                  >
                    <Switch
                      id="create_users"
                      checked={field.value}
                      onChange={(e) => field.onChange(e.target.checked)}
                    />
                  </FormField>
                )}
              />

              <Controller
                name="sync_display_name"
                control={form.control}
                render={({ field }) => (
                  <FormField
                    label="Sync Display Name"
                    htmlFor="sync_display_name"
                    helpText="Update user display names when they change in the IdP"
                  >
                    <Switch
                      id="sync_display_name"
                      checked={field.value}
                      onChange={(e) => field.onChange(e.target.checked)}
                    />
                  </FormField>
                )}
              />

              <Controller
                name="default_team_id"
                control={form.control}
                render={({ field }) => (
                  <FormField
                    label="Default Team"
                    htmlFor="default_team_id"
                    helpText="New users will be automatically added to this team"
                  >
                    <Select
                      value={field.value ?? ""}
                      onChange={(value) => field.onChange(value || null)}
                      options={teamOptions}
                    />
                  </FormField>
                )}
              />

              <Controller
                name="default_org_role"
                control={form.control}
                render={({ field }) => (
                  <FormField
                    label="Default Organization Role"
                    htmlFor="default_org_role"
                    required
                    error={form.formState.errors.default_org_role?.message}
                    helpText="Role assigned to new users in the organization"
                  >
                    <Select
                      value={field.value ?? "member"}
                      onChange={(value) => field.onChange(value ?? "member")}
                      options={ROLE_OPTIONS}
                    />
                  </FormField>
                )}
              />

              <Controller
                name="default_team_role"
                control={form.control}
                render={({ field }) => (
                  <FormField
                    label="Default Team Role"
                    htmlFor="default_team_role"
                    required
                    error={form.formState.errors.default_team_role?.message}
                    helpText="Role assigned to new users in the default team"
                  >
                    <Select
                      value={field.value ?? "member"}
                      onChange={(value) => field.onChange(value ?? "member")}
                      options={ROLE_OPTIONS}
                    />
                  </FormField>
                )}
              />
            </div>

            {/* Deprovisioning Settings */}
            <div className="space-y-4">
              <h3 className="font-medium text-sm">Deprovisioning Settings</h3>
              <p className="text-sm text-muted-foreground">
                Configure what happens when a user is deactivated or deleted in your IdP.
              </p>

              <Controller
                name="deactivate_deletes_user"
                control={form.control}
                render={({ field }) => (
                  <FormField
                    label="Delete Users on Deactivation"
                    htmlFor="deactivate_deletes_user"
                    helpText="When disabled, users are marked inactive instead of deleted"
                  >
                    <Switch
                      id="deactivate_deletes_user"
                      checked={field.value}
                      onChange={(e) => field.onChange(e.target.checked)}
                    />
                  </FormField>
                )}
              />

              <Controller
                name="revoke_api_keys_on_deactivate"
                control={form.control}
                render={({ field }) => (
                  <FormField
                    label="Revoke API Keys on Deactivation"
                    htmlFor="revoke_api_keys_on_deactivate"
                    helpText="Automatically revoke all API keys when a user is deactivated"
                  >
                    <Switch
                      id="revoke_api_keys_on_deactivate"
                      checked={field.value}
                      onChange={(e) => field.onChange(e.target.checked)}
                    />
                  </FormField>
                )}
              />
            </div>
          </div>
        </ModalContent>
        <ModalFooter>
          <Button type="button" variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button type="submit" isLoading={isLoading}>
            {isEditing ? "Save Changes" : "Enable SCIM"}
          </Button>
        </ModalFooter>
      </form>
    </Modal>
  );
}
