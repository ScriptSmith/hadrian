import { zodResolver } from "@hookform/resolvers/zod";
import { useEffect } from "react";
import { useForm, Controller } from "react-hook-form";
import { z } from "zod";
import { Info } from "lucide-react";

import type {
  OrgRbacPolicy,
  CreateOrgRbacPolicy,
  UpdateOrgRbacPolicy,
  RbacPolicyEffect,
} from "@/api/generated/types.gen";
import { Button } from "@/components/Button/Button";
import { FormField } from "@/components/FormField/FormField";
import { Input } from "@/components/Input/Input";
import { Modal, ModalHeader, ModalContent, ModalFooter } from "@/components/Modal/Modal";
import { Select } from "@/components/Select/Select";
import { Switch } from "@/components/Switch/Switch";
import { Textarea } from "@/components/Textarea/Textarea";
import { CelExpressionInput } from "./CelExpressionInput";

const EFFECT_OPTIONS = [
  { value: "deny", label: "Deny", description: "Block access when condition matches" },
  { value: "allow", label: "Allow", description: "Grant access when condition matches" },
];

// Common resources in the system - users can also enter custom patterns
const COMMON_RESOURCES = [
  { value: "*", label: "* (all resources)" },
  { value: "model", label: "model" },
  { value: "organization", label: "organization" },
  { value: "team", label: "team" },
  { value: "project", label: "project" },
  { value: "user", label: "user" },
  { value: "api_key", label: "api_key" },
  { value: "sso_config", label: "sso_config" },
  { value: "sso_group_mapping", label: "sso_group_mapping" },
  { value: "scim_config", label: "scim_config" },
  { value: "rbac_policy", label: "rbac_policy" },
  { value: "dynamic_provider", label: "dynamic_provider" },
  { value: "model_pricing", label: "model_pricing" },
  { value: "domain_verification", label: "domain_verification" },
  { value: "conversation", label: "conversation" },
  { value: "prompt", label: "prompt" },
];

// Common actions - users can also enter custom patterns
const COMMON_ACTIONS = [
  { value: "*", label: "* (all actions)" },
  { value: "use", label: "use" },
  { value: "create", label: "create" },
  { value: "read", label: "read" },
  { value: "list", label: "list" },
  { value: "update", label: "update" },
  { value: "delete", label: "delete" },
  { value: "manage", label: "manage" },
];

const rbacPolicySchema = z.object({
  name: z.string().min(1, "Name is required").max(100, "Name must be 100 characters or less"),
  description: z.string().max(500, "Description must be 500 characters or less").optional(),
  resource: z.string().min(1, "Resource pattern is required"),
  action: z.string().min(1, "Action pattern is required"),
  condition: z.string().min(1, "CEL condition is required"),
  effect: z.enum(["allow", "deny"] as const),
  priority: z
    .number()
    .int()
    .min(-1000, "Priority must be at least -1000")
    .max(1000, "Priority must be at most 1000"),
  enabled: z.boolean(),
  reason: z.string().max(500, "Reason must be 500 characters or less").optional(),
});

type FormValues = z.infer<typeof rbacPolicySchema>;

interface RbacPolicyFormModalProps {
  open: boolean;
  onClose: () => void;
  onCreateSubmit: (data: CreateOrgRbacPolicy) => void;
  onUpdateSubmit: (data: UpdateOrgRbacPolicy) => void;
  isLoading: boolean;
  editingPolicy: OrgRbacPolicy | null;
}

const DEFAULT_VALUES: FormValues = {
  name: "",
  description: "",
  resource: "*",
  action: "*",
  condition: "",
  effect: "deny",
  priority: 0,
  enabled: true,
  reason: "",
};

export function RbacPolicyFormModal({
  open,
  onClose,
  onCreateSubmit,
  onUpdateSubmit,
  isLoading,
  editingPolicy,
}: RbacPolicyFormModalProps) {
  const isEditing = !!editingPolicy;

  const form = useForm<FormValues>({
    resolver: zodResolver(rbacPolicySchema),
    defaultValues: DEFAULT_VALUES,
  });

  useEffect(() => {
    if (open) {
      if (editingPolicy) {
        form.reset({
          name: editingPolicy.name,
          description: editingPolicy.description ?? "",
          resource: editingPolicy.resource,
          action: editingPolicy.action,
          condition: editingPolicy.condition,
          effect: editingPolicy.effect,
          priority: editingPolicy.priority,
          enabled: editingPolicy.enabled,
          reason: "",
        });
      } else {
        form.reset(DEFAULT_VALUES);
      }
    }
  }, [open, editingPolicy, form]);

  const handleSubmit = form.handleSubmit((data) => {
    if (isEditing) {
      const updateData: UpdateOrgRbacPolicy = {
        name: data.name,
        description: data.description || null,
        resource: data.resource,
        action: data.action,
        condition: data.condition,
        effect: data.effect as RbacPolicyEffect,
        priority: data.priority,
        enabled: data.enabled,
        reason: data.reason || null,
      };
      onUpdateSubmit(updateData);
    } else {
      const createData: CreateOrgRbacPolicy = {
        name: data.name,
        description: data.description || null,
        resource: data.resource,
        action: data.action,
        condition: data.condition,
        effect: data.effect as RbacPolicyEffect,
        priority: data.priority,
        enabled: data.enabled,
        reason: data.reason || null,
      };
      onCreateSubmit(createData);
    }
  });

  return (
    <Modal open={open} onClose={onClose}>
      <form onSubmit={handleSubmit}>
        <ModalHeader>{isEditing ? "Edit RBAC Policy" : "Create RBAC Policy"}</ModalHeader>
        <ModalContent className="space-y-6 max-h-[70vh] overflow-y-auto">
          {/* Basic Info */}
          <div className="space-y-4">
            <h3 className="text-sm font-semibold text-foreground">Basic Information</h3>

            <FormField
              label="Name"
              htmlFor="name"
              required
              error={form.formState.errors.name?.message}
              helpText="A unique name for this policy within the organization"
            >
              <Input
                id="name"
                {...form.register("name")}
                placeholder="require-admin-for-settings"
              />
            </FormField>

            <FormField
              label="Description"
              htmlFor="description"
              error={form.formState.errors.description?.message}
              helpText="Optional description explaining what this policy does"
            >
              <Textarea
                id="description"
                {...form.register("description")}
                placeholder="Restricts settings access to administrators only"
                rows={2}
              />
            </FormField>
          </div>

          {/* Policy Scope */}
          <div className="space-y-4">
            <h3 className="text-sm font-semibold text-foreground">Policy Scope</h3>

            <div className="flex items-start gap-3 rounded-lg border bg-muted/30 p-3">
              <Info className="h-5 w-5 text-muted-foreground mt-0.5 flex-shrink-0" />
              <p className="text-sm text-muted-foreground">
                Select from suggestions or type custom patterns. Use{" "}
                <code className="bg-muted px-1 rounded">*</code> to match all. Wildcards can be used
                for prefix matching: <code className="bg-muted px-1 rounded">team*</code> matches
                team, teams, team_member. Pattern matching is case-sensitive.
              </p>
            </div>

            <div className="grid grid-cols-2 gap-4">
              <FormField
                label="Resource Pattern"
                htmlFor="resource"
                required
                error={form.formState.errors.resource?.message}
                helpText="Select a resource or type a custom pattern with wildcards"
              >
                <Input
                  id="resource"
                  {...form.register("resource")}
                  placeholder="*"
                  list="resource-suggestions"
                />
                <datalist id="resource-suggestions">
                  {COMMON_RESOURCES.map((r) => (
                    <option key={r.value} value={r.value}>
                      {r.label}
                    </option>
                  ))}
                </datalist>
              </FormField>

              <FormField
                label="Action Pattern"
                htmlFor="action"
                required
                error={form.formState.errors.action?.message}
                helpText="Select an action or type a custom pattern"
              >
                <Input
                  id="action"
                  {...form.register("action")}
                  placeholder="*"
                  list="action-suggestions"
                />
                <datalist id="action-suggestions">
                  {COMMON_ACTIONS.map((a) => (
                    <option key={a.value} value={a.value}>
                      {a.label}
                    </option>
                  ))}
                </datalist>
              </FormField>
            </div>
          </div>

          {/* CEL Condition */}
          <div className="space-y-4">
            <h3 className="text-sm font-semibold text-foreground">CEL Condition</h3>

            <FormField
              label="Condition"
              htmlFor="condition"
              required
              error={form.formState.errors.condition?.message}
              helpText="CEL expression that must evaluate to true for the policy to apply"
            >
              <Controller
                name="condition"
                control={form.control}
                render={({ field }) => (
                  <CelExpressionInput
                    value={field.value ?? ""}
                    onChange={field.onChange}
                    error={form.formState.errors.condition?.message}
                    placeholder="'admin' in subject.roles"
                  />
                )}
              />
            </FormField>
          </div>

          {/* Settings */}
          <div className="space-y-4">
            <h3 className="text-sm font-semibold text-foreground">Settings</h3>

            <div className="grid grid-cols-2 gap-4">
              <FormField
                label="Effect"
                htmlFor="effect"
                required
                error={form.formState.errors.effect?.message}
                helpText="What happens when the condition matches"
              >
                <Controller
                  name="effect"
                  control={form.control}
                  render={({ field }) => (
                    <Select
                      value={field.value}
                      onChange={field.onChange}
                      options={EFFECT_OPTIONS}
                    />
                  )}
                />
              </FormField>

              <FormField
                label="Priority"
                htmlFor="priority"
                required
                error={form.formState.errors.priority?.message}
                helpText="Higher priority policies are evaluated first. At the same priority, deny policies are evaluated before allow policies."
              >
                <Input
                  id="priority"
                  type="number"
                  min={0}
                  {...form.register("priority", { valueAsNumber: true })}
                  placeholder="0"
                />
              </FormField>
            </div>

            <div className="flex items-center justify-between p-3 rounded-lg border">
              <div>
                <p className="text-sm font-medium">Enabled</p>
                <p className="text-xs text-muted-foreground">Disabled policies are not evaluated</p>
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
          </div>

          {/* Reason (for audit trail) */}
          {isEditing && (
            <div className="space-y-4">
              <h3 className="text-sm font-semibold text-foreground">Change Reason</h3>

              <FormField
                label="Reason"
                htmlFor="reason"
                error={form.formState.errors.reason?.message}
                helpText="Optional reason for this change (stored in version history)"
              >
                <Textarea
                  id="reason"
                  {...form.register("reason")}
                  placeholder="Updating to include new team requirements"
                  rows={2}
                />
              </FormField>
            </div>
          )}
        </ModalContent>
        <ModalFooter>
          <Button type="button" variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button type="submit" isLoading={isLoading}>
            {isEditing ? "Save Changes" : "Create Policy"}
          </Button>
        </ModalFooter>
      </form>
    </Modal>
  );
}
