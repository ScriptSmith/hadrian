import { useMutation } from "@tanstack/react-query";
import { useState } from "react";
import { useForm, Controller } from "react-hook-form";
import {
  Play,
  ChevronDown,
  ChevronUp,
  CheckCircle2,
  XCircle,
  AlertCircle,
  Loader2,
  User,
  Target,
  Server,
  Building,
} from "lucide-react";

import { orgRbacPolicySimulateMutation } from "@/api/generated/@tanstack/react-query.gen";
import type {
  OrgRbacPolicy,
  SimulatePolicyResponse,
  PolicyEvaluationResult,
  SimulateRequestContext,
  PolicySource,
} from "@/api/generated/types.gen";
import { Button } from "@/components/Button/Button";
import { FormField } from "@/components/FormField/FormField";
import { Input } from "@/components/Input/Input";
import { Select } from "@/components/Select/Select";
import { Badge } from "@/components/Badge/Badge";
import { cn } from "@/utils/cn";

// Common resources - matches RbacPolicyFormModal
const COMMON_RESOURCES = [
  { value: "*", label: "* (all)" },
  { value: "organization", label: "organization" },
  { value: "team", label: "team" },
  { value: "project", label: "project" },
  { value: "user", label: "user" },
  { value: "api_key", label: "api_key" },
  { value: "sso_config", label: "sso_config" },
  { value: "sso_group_mapping", label: "sso_group_mapping" },
  { value: "scim_config", label: "scim_config" },
  { value: "org_rbac_policy", label: "org_rbac_policy" },
  { value: "dynamic_provider", label: "dynamic_provider" },
  { value: "model_pricing", label: "model_pricing" },
  { value: "domain_verification", label: "domain_verification" },
  { value: "conversation", label: "conversation" },
  { value: "prompt", label: "prompt" },
];

const COMMON_ACTIONS = [
  { value: "*", label: "* (all)" },
  { value: "create", label: "create" },
  { value: "read", label: "read" },
  { value: "list", label: "list" },
  { value: "update", label: "update" },
  { value: "delete", label: "delete" },
  { value: "manage", label: "manage" },
];

interface RbacPolicySimulatorProps {
  orgSlug: string;
  policies: OrgRbacPolicy[];
}

interface FormValues {
  // Subject
  roles: string;
  email: string;
  user_id: string;
  external_id: string;
  org_ids: string;
  team_ids: string;
  project_ids: string;
  // Context - required
  resource_type: string;
  action: string;
  // Context - optional IDs
  resource_id: string;
  org_id: string;
  team_id: string;
  project_id: string;
  // Context - Chat Completion
  model: string;
  max_tokens: string;
  messages_count: string;
  has_tools: string;
  has_file_search: string;
  stream: string;
  reasoning_effort: string;
  response_format: string;
  temperature: string;
  has_images: string;
  // Context - Image Generation
  image_count: string;
  image_size: string;
  image_quality: string;
  // Context - Audio
  character_count: string;
  voice: string;
  language: string;
  // Filter
  policy_id: string;
}

const DEFAULT_VALUES: FormValues = {
  roles: "member",
  email: "",
  user_id: "",
  external_id: "",
  org_ids: "",
  team_ids: "",
  project_ids: "",
  resource_type: "project",
  action: "read",
  resource_id: "",
  org_id: "",
  team_id: "",
  project_id: "",
  model: "",
  max_tokens: "",
  messages_count: "",
  has_tools: "",
  has_file_search: "",
  stream: "",
  reasoning_effort: "",
  response_format: "",
  temperature: "",
  has_images: "",
  image_count: "",
  image_size: "",
  image_quality: "",
  character_count: "",
  voice: "",
  language: "",
  policy_id: "",
};

function ResultBadge({ allowed }: { allowed: boolean }) {
  return (
    <Badge
      className={cn(
        "text-sm px-3 py-1",
        allowed
          ? "bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400"
          : "bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-400"
      )}
    >
      {allowed ? (
        <>
          <CheckCircle2 className="h-4 w-4 mr-1" />
          ALLOWED
        </>
      ) : (
        <>
          <XCircle className="h-4 w-4 mr-1" />
          DENIED
        </>
      )}
    </Badge>
  );
}

function SourceBadge({ source }: { source: PolicySource }) {
  return source === "system" ? (
    <Badge className="text-xs bg-purple-100 text-purple-700 dark:bg-purple-900/30 dark:text-purple-400">
      <Server className="h-3 w-3 mr-1" />
      System
    </Badge>
  ) : (
    <Badge className="text-xs bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-400">
      <Building className="h-3 w-3 mr-1" />
      Org
    </Badge>
  );
}

function PolicyResult({ result, index }: { result: PolicyEvaluationResult; index: number }) {
  return (
    <div
      key={result.id ?? result.name}
      className={cn(
        "flex items-center justify-between p-2 rounded text-sm border",
        result.condition_matched
          ? result.effect === "allow"
            ? "border-green-200 bg-green-50 dark:border-green-800 dark:bg-green-900/20"
            : "border-red-200 bg-red-50 dark:border-red-800 dark:bg-red-900/20"
          : "border-gray-200 bg-gray-50 dark:border-gray-700 dark:bg-gray-800/50"
      )}
    >
      <div className="flex items-center gap-2 flex-wrap">
        <span className="text-xs text-muted-foreground">{index + 1}.</span>
        <span className="font-medium">{result.name}</span>
        <SourceBadge source={result.source} />
        <Badge variant="secondary" className="text-xs">
          P{result.priority}
        </Badge>
        {result.pattern_matched ? (
          result.condition_matched ? (
            <Badge
              variant={result.effect === "allow" ? "default" : "destructive"}
              className="text-xs"
            >
              {result.effect === "allow" ? "MATCH (allow)" : "MATCH (deny)"}
            </Badge>
          ) : result.error ? (
            <Badge variant="destructive" className="text-xs">
              ERROR
            </Badge>
          ) : result.skipped_reason ? (
            <Badge variant="outline" className="text-xs">
              {result.skipped_reason}
            </Badge>
          ) : (
            <span className="text-xs text-muted-foreground">pattern matched, condition false</span>
          )
        ) : (
          <span className="text-xs text-muted-foreground">pattern not matched</span>
        )}
      </div>
      {result.description && (
        <span
          className="text-xs text-muted-foreground ml-2 truncate max-w-48"
          title={result.description}
        >
          {result.description}
        </span>
      )}
    </div>
  );
}

function EvaluationTrace({
  systemPolicies,
  orgPolicies,
}: {
  systemPolicies: PolicyEvaluationResult[];
  orgPolicies: PolicyEvaluationResult[];
}) {
  const hasSystemPolicies = systemPolicies.length > 0;
  const hasOrgPolicies = orgPolicies.length > 0;

  if (!hasSystemPolicies && !hasOrgPolicies) {
    return <p className="text-sm text-muted-foreground italic">No policies were evaluated</p>;
  }

  return (
    <div className="space-y-4">
      {/* System Policies Section */}
      {hasSystemPolicies && (
        <div className="space-y-2">
          <div className="flex items-center gap-2">
            <Server className="h-4 w-4 text-purple-600 dark:text-purple-400" />
            <p className="text-xs font-medium text-purple-700 dark:text-purple-300 uppercase">
              System Policies (from config)
            </p>
          </div>
          <div className="space-y-1">
            {systemPolicies.map((result, index) => (
              <PolicyResult key={result.name} result={result} index={index} />
            ))}
          </div>
        </div>
      )}

      {/* Organization Policies Section */}
      {hasOrgPolicies && (
        <div className="space-y-2">
          <div className="flex items-center gap-2">
            <Building className="h-4 w-4 text-blue-700 dark:text-blue-400" />
            <p className="text-xs font-medium text-blue-700 dark:text-blue-300 uppercase">
              Organization Policies (from database)
            </p>
          </div>
          <div className="space-y-1">
            {orgPolicies.map((result, index) => (
              <PolicyResult key={result.id ?? result.name} result={result} index={index} />
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

export function RbacPolicySimulator({ orgSlug, policies }: RbacPolicySimulatorProps) {
  const [expanded, setExpanded] = useState(false);
  const [showAdvancedContext, setShowAdvancedContext] = useState(false);
  const [result, setResult] = useState<SimulatePolicyResponse | null>(null);

  const form = useForm<FormValues>({
    defaultValues: DEFAULT_VALUES,
  });

  const simulateMutation = useMutation({
    ...orgRbacPolicySimulateMutation(),
    onSuccess: (data) => {
      setResult(data);
    },
    onError: () => {
      setResult(null);
    },
  });

  const handleSimulate = form.handleSubmit((data) => {
    const parseArray = (str: string): string[] =>
      str
        .split(",")
        .map((s) => s.trim())
        .filter((s) => s.length > 0);

    // Build request context only if any fields are set
    const buildRequestContext = (): SimulateRequestContext | null => {
      const hasAnyValue =
        data.max_tokens ||
        data.messages_count ||
        data.has_tools ||
        data.has_file_search ||
        data.stream ||
        data.reasoning_effort ||
        data.response_format ||
        data.temperature ||
        data.has_images ||
        data.image_count ||
        data.image_size ||
        data.image_quality ||
        data.character_count ||
        data.voice ||
        data.language;

      if (!hasAnyValue) return null;

      const parseBool = (str: string): boolean | null => {
        if (str === "true") return true;
        if (str === "false") return false;
        return null;
      };

      return {
        max_tokens: data.max_tokens ? parseInt(data.max_tokens) : null,
        messages_count: data.messages_count ? parseInt(data.messages_count) : null,
        has_tools: parseBool(data.has_tools),
        has_file_search: parseBool(data.has_file_search),
        stream: parseBool(data.stream),
        reasoning_effort: data.reasoning_effort || null,
        response_format: data.response_format || null,
        temperature: data.temperature ? parseFloat(data.temperature) : null,
        has_images: parseBool(data.has_images),
        image_count: data.image_count ? parseInt(data.image_count) : null,
        image_size: data.image_size || null,
        image_quality: data.image_quality || null,
        character_count: data.character_count ? parseInt(data.character_count) : null,
        voice: data.voice || null,
        language: data.language || null,
      };
    };

    simulateMutation.mutate({
      path: { org_slug: orgSlug },
      body: {
        subject: {
          roles: parseArray(data.roles),
          email: data.email || null,
          user_id: data.user_id || null,
          external_id: data.external_id || null,
          org_ids: parseArray(data.org_ids),
          team_ids: parseArray(data.team_ids),
          project_ids: parseArray(data.project_ids),
        },
        context: {
          resource_type: data.resource_type,
          action: data.action,
          resource_id: data.resource_id || null,
          org_id: data.org_id || null,
          team_id: data.team_id || null,
          project_id: data.project_id || null,
          model: data.model || null,
          request: buildRequestContext(),
        },
        policy_id: data.policy_id || null,
      },
    });
  });

  const policyOptions = [
    { value: "", label: "All policies" },
    ...policies.map((p) => ({ value: p.id, label: p.name })),
  ];

  return (
    <div className="border rounded-lg">
      <button
        type="button"
        className="w-full flex items-center justify-between p-4 hover:bg-muted/50 transition-colors"
        onClick={() => setExpanded(!expanded)}
      >
        <div className="flex items-center gap-2">
          <Play className="h-5 w-5 text-primary" />
          <span className="font-medium">Policy Simulator</span>
          <span className="text-sm text-muted-foreground">
            Test how policies evaluate for different subjects and contexts
          </span>
        </div>
        {expanded ? (
          <ChevronUp className="h-5 w-5 text-muted-foreground" />
        ) : (
          <ChevronDown className="h-5 w-5 text-muted-foreground" />
        )}
      </button>

      {expanded && (
        <div className="border-t p-4">
          <form onSubmit={handleSimulate}>
            {/* Side-by-side layout for Subject and Context */}
            <div className="grid grid-cols-2 gap-6">
              {/* Left: Subject (Who) */}
              <div className="space-y-4 p-4 rounded-lg bg-blue-50/50 dark:bg-blue-950/20 border border-blue-200 dark:border-blue-800">
                <div className="flex items-center gap-2">
                  <User className="h-4 w-4 text-blue-700 dark:text-blue-400" />
                  <h4 className="text-sm font-semibold text-blue-700 dark:text-blue-300">
                    Subject (Who)
                  </h4>
                </div>

                <FormField label="Roles" htmlFor="sim-roles" helpText="Comma-separated">
                  <Input
                    id="sim-roles"
                    {...form.register("roles")}
                    placeholder="admin, member, viewer"
                  />
                </FormField>

                <FormField label="Email" htmlFor="sim-email">
                  <Input
                    id="sim-email"
                    {...form.register("email")}
                    placeholder="user@example.com"
                  />
                </FormField>

                <div className="grid grid-cols-2 gap-3">
                  <FormField label="User ID" htmlFor="sim-user_id">
                    <Input id="sim-user_id" {...form.register("user_id")} placeholder="user-123" />
                  </FormField>
                  <FormField label="External ID" htmlFor="sim-external_id">
                    <Input
                      id="sim-external_id"
                      {...form.register("external_id")}
                      placeholder="ext-456"
                    />
                  </FormField>
                </div>

                <FormField
                  label="Organization IDs"
                  htmlFor="sim-org_ids"
                  helpText="Comma-separated"
                >
                  <Input
                    id="sim-org_ids"
                    {...form.register("org_ids")}
                    placeholder="org-1, org-2"
                  />
                </FormField>

                <FormField label="Team IDs" htmlFor="sim-team_ids" helpText="Comma-separated">
                  <Input
                    id="sim-team_ids"
                    {...form.register("team_ids")}
                    placeholder="team-1, team-2"
                  />
                </FormField>

                <FormField label="Project IDs" htmlFor="sim-project_ids" helpText="Comma-separated">
                  <Input
                    id="sim-project_ids"
                    {...form.register("project_ids")}
                    placeholder="proj-1, proj-2"
                  />
                </FormField>
              </div>

              {/* Right: Context (What) */}
              <div className="space-y-4 p-4 rounded-lg bg-amber-50/50 dark:bg-amber-950/20 border border-amber-200 dark:border-amber-800">
                <div className="flex items-center gap-2">
                  <Target className="h-4 w-4 text-amber-800 dark:text-amber-400" />
                  <h4 className="text-sm font-semibold text-amber-700 dark:text-amber-300">
                    Context (What)
                  </h4>
                </div>

                <div className="grid grid-cols-2 gap-3">
                  <FormField label="Resource Type" htmlFor="sim-resource_type" required>
                    <Input
                      id="sim-resource_type"
                      {...form.register("resource_type")}
                      placeholder="project"
                      list="sim-resource-suggestions"
                    />
                    <datalist id="sim-resource-suggestions">
                      {COMMON_RESOURCES.map((r) => (
                        <option key={r.value} value={r.value} />
                      ))}
                    </datalist>
                  </FormField>

                  <FormField label="Action" htmlFor="sim-action" required>
                    <Input
                      id="sim-action"
                      {...form.register("action")}
                      placeholder="read"
                      list="sim-action-suggestions"
                    />
                    <datalist id="sim-action-suggestions">
                      {COMMON_ACTIONS.map((a) => (
                        <option key={a.value} value={a.value} />
                      ))}
                    </datalist>
                  </FormField>
                </div>

                <FormField
                  label="Resource ID"
                  htmlFor="sim-resource_id"
                  helpText="Specific resource"
                >
                  <Input
                    id="sim-resource_id"
                    {...form.register("resource_id")}
                    placeholder="resource-123"
                  />
                </FormField>

                <div className="grid grid-cols-3 gap-3">
                  <FormField label="Org ID" htmlFor="sim-org_id">
                    <Input id="sim-org_id" {...form.register("org_id")} placeholder="org-123" />
                  </FormField>
                  <FormField label="Team ID" htmlFor="sim-team_id">
                    <Input id="sim-team_id" {...form.register("team_id")} placeholder="team-123" />
                  </FormField>
                  <FormField label="Project ID" htmlFor="sim-project_id">
                    <Input
                      id="sim-project_id"
                      {...form.register("project_id")}
                      placeholder="proj-123"
                    />
                  </FormField>
                </div>

                {/* Advanced Context Fields (API-specific) */}
                <div className="pt-2 border-t border-amber-200 dark:border-amber-700">
                  <button
                    type="button"
                    className="flex items-center gap-1 text-xs text-amber-800 dark:text-amber-400 hover:underline"
                    onClick={() => setShowAdvancedContext(!showAdvancedContext)}
                  >
                    {showAdvancedContext ? (
                      <ChevronUp className="h-3 w-3" />
                    ) : (
                      <ChevronDown className="h-3 w-3" />
                    )}
                    {showAdvancedContext ? "Hide" : "Show"} API request fields
                  </button>

                  {showAdvancedContext && (
                    <div className="mt-3 space-y-4">
                      {/* Model (shared across endpoints) */}
                      <FormField label="Model" htmlFor="sim-model" helpText="Model being requested">
                        <Input
                          id="sim-model"
                          {...form.register("model")}
                          placeholder="gpt-4o, claude-3-opus"
                        />
                      </FormField>

                      {/* Chat Completion Fields */}
                      <div className="space-y-3">
                        <p className="text-xs font-medium text-amber-800 dark:text-amber-400">
                          Chat Completion
                        </p>
                        <div className="grid grid-cols-3 gap-3">
                          <FormField label="Max Tokens" htmlFor="sim-max_tokens">
                            <Input
                              id="sim-max_tokens"
                              {...form.register("max_tokens")}
                              placeholder="4096"
                              type="number"
                            />
                          </FormField>
                          <FormField label="Messages Count" htmlFor="sim-messages_count">
                            <Input
                              id="sim-messages_count"
                              {...form.register("messages_count")}
                              placeholder="10"
                              type="number"
                            />
                          </FormField>
                          <FormField label="Temperature" htmlFor="sim-temperature">
                            <Input
                              id="sim-temperature"
                              {...form.register("temperature")}
                              placeholder="0.7"
                              type="number"
                              step="0.1"
                            />
                          </FormField>
                        </div>
                        <div className="grid grid-cols-4 gap-3">
                          <FormField label="Stream" htmlFor="sim-stream">
                            <Select
                              value={form.watch("stream")}
                              onChange={(v) => form.setValue("stream", v ?? "")}
                              options={[
                                { value: "", label: "Not set" },
                                { value: "true", label: "true" },
                                { value: "false", label: "false" },
                              ]}
                            />
                          </FormField>
                          <FormField label="Has Tools" htmlFor="sim-has_tools">
                            <Select
                              value={form.watch("has_tools")}
                              onChange={(v) => form.setValue("has_tools", v ?? "")}
                              options={[
                                { value: "", label: "Not set" },
                                { value: "true", label: "true" },
                                { value: "false", label: "false" },
                              ]}
                            />
                          </FormField>
                          <FormField label="File Search" htmlFor="sim-has_file_search">
                            <Select
                              value={form.watch("has_file_search")}
                              onChange={(v) => form.setValue("has_file_search", v ?? "")}
                              options={[
                                { value: "", label: "Not set" },
                                { value: "true", label: "true" },
                                { value: "false", label: "false" },
                              ]}
                            />
                          </FormField>
                          <FormField label="Has Images" htmlFor="sim-has_images">
                            <Select
                              value={form.watch("has_images")}
                              onChange={(v) => form.setValue("has_images", v ?? "")}
                              options={[
                                { value: "", label: "Not set" },
                                { value: "true", label: "true" },
                                { value: "false", label: "false" },
                              ]}
                            />
                          </FormField>
                        </div>
                        <div className="grid grid-cols-2 gap-3">
                          <FormField label="Reasoning Effort" htmlFor="sim-reasoning_effort">
                            <Select
                              value={form.watch("reasoning_effort")}
                              onChange={(v) => form.setValue("reasoning_effort", v ?? "")}
                              options={[
                                { value: "", label: "Not set" },
                                { value: "none", label: "none" },
                                { value: "minimal", label: "minimal" },
                                { value: "low", label: "low" },
                                { value: "medium", label: "medium" },
                                { value: "high", label: "high" },
                              ]}
                            />
                          </FormField>
                          <FormField label="Response Format" htmlFor="sim-response_format">
                            <Select
                              value={form.watch("response_format")}
                              onChange={(v) => form.setValue("response_format", v ?? "")}
                              options={[
                                { value: "", label: "Not set" },
                                { value: "text", label: "text" },
                                { value: "json_object", label: "json_object" },
                                { value: "json_schema", label: "json_schema" },
                              ]}
                            />
                          </FormField>
                        </div>
                      </div>

                      {/* Image Generation Fields */}
                      <div className="space-y-3">
                        <p className="text-xs font-medium text-amber-800 dark:text-amber-400">
                          Image Generation
                        </p>
                        <div className="grid grid-cols-3 gap-3">
                          <FormField label="Image Count" htmlFor="sim-image_count">
                            <Input
                              id="sim-image_count"
                              {...form.register("image_count")}
                              placeholder="1"
                              type="number"
                            />
                          </FormField>
                          <FormField label="Image Size" htmlFor="sim-image_size">
                            <Select
                              value={form.watch("image_size")}
                              onChange={(v) => form.setValue("image_size", v ?? "")}
                              options={[
                                { value: "", label: "Not set" },
                                { value: "256x256", label: "256x256" },
                                { value: "512x512", label: "512x512" },
                                { value: "1024x1024", label: "1024x1024" },
                                { value: "1536x1024", label: "1536x1024" },
                                { value: "1024x1536", label: "1024x1536" },
                              ]}
                            />
                          </FormField>
                          <FormField label="Image Quality" htmlFor="sim-image_quality">
                            <Select
                              value={form.watch("image_quality")}
                              onChange={(v) => form.setValue("image_quality", v ?? "")}
                              options={[
                                { value: "", label: "Not set" },
                                { value: "standard", label: "standard" },
                                { value: "hd", label: "hd" },
                                { value: "high", label: "high" },
                              ]}
                            />
                          </FormField>
                        </div>
                      </div>

                      {/* Audio Fields */}
                      <div className="space-y-3">
                        <p className="text-xs font-medium text-amber-800 dark:text-amber-400">
                          Audio (TTS / Transcription)
                        </p>
                        <div className="grid grid-cols-3 gap-3">
                          <FormField
                            label="Character Count"
                            htmlFor="sim-character_count"
                            helpText="TTS input length"
                          >
                            <Input
                              id="sim-character_count"
                              {...form.register("character_count")}
                              placeholder="500"
                              type="number"
                            />
                          </FormField>
                          <FormField label="Voice" htmlFor="sim-voice" helpText="TTS voice">
                            <Input
                              id="sim-voice"
                              {...form.register("voice")}
                              placeholder="alloy, echo, fable"
                            />
                          </FormField>
                          <FormField
                            label="Language"
                            htmlFor="sim-language"
                            helpText="ISO-639-1 code"
                          >
                            <Input
                              id="sim-language"
                              {...form.register("language")}
                              placeholder="en, es, fr"
                            />
                          </FormField>
                        </div>
                      </div>

                      <p className="text-xs text-muted-foreground italic">
                        These fields are for CEL conditions like{" "}
                        <code className="bg-muted px-1 rounded">context.model</code>,{" "}
                        <code className="bg-muted px-1 rounded">context.request.max_tokens</code>,{" "}
                        <code className="bg-muted px-1 rounded">context.request.image_size</code>
                      </p>
                    </div>
                  )}
                </div>
              </div>
            </div>

            {/* Policy Filter + Run Button */}
            <div className="mt-4 pt-4 border-t flex items-end gap-4">
              <div className="w-64">
                <FormField label="Test Specific Policy" htmlFor="sim-policy_id">
                  <Controller
                    name="policy_id"
                    control={form.control}
                    render={({ field }) => (
                      <Select
                        value={field.value}
                        onChange={field.onChange}
                        options={policyOptions}
                      />
                    )}
                  />
                </FormField>
              </div>
              <Button type="submit" disabled={simulateMutation.isPending}>
                {simulateMutation.isPending ? (
                  <>
                    <Loader2 className="h-4 w-4 mr-2 animate-spin" />
                    Simulating...
                  </>
                ) : (
                  <>
                    <Play className="h-4 w-4 mr-2" />
                    Run Simulation
                  </>
                )}
              </Button>
            </div>
          </form>

          {/* Results */}
          {result && (
            <div className="mt-4 pt-4 border-t space-y-4">
              {/* RBAC Disabled Banner */}
              {!result.rbac_enabled && (
                <div className="flex items-start gap-2 p-3 rounded-lg bg-amber-100 dark:bg-amber-900/30 border border-amber-300 dark:border-amber-700">
                  <AlertCircle className="h-4 w-4 text-amber-800 dark:text-amber-400 mt-0.5" />
                  <div>
                    <p className="text-sm font-medium text-amber-800 dark:text-amber-200">
                      RBAC is Disabled
                    </p>
                    <p className="text-xs text-amber-700 dark:text-amber-300">
                      All requests are allowed when RBAC is disabled in the configuration.
                    </p>
                  </div>
                </div>
              )}

              <div className="flex items-center justify-between">
                <h4 className="text-sm font-semibold text-foreground">Result</h4>
                <ResultBadge allowed={result.allowed} />
              </div>

              {result.reason && (
                <div className="flex items-start gap-2 p-3 rounded-lg bg-muted/50">
                  <AlertCircle className="h-4 w-4 text-muted-foreground mt-0.5" />
                  <p className="text-sm">{result.reason}</p>
                </div>
              )}

              {result.matched_policy && (
                <p className="text-sm">
                  <span className="text-muted-foreground">Determined by: </span>
                  <span className="font-medium">{result.matched_policy}</span>
                  {result.matched_policy_source && (
                    <span className="ml-2">
                      <SourceBadge source={result.matched_policy_source} />
                    </span>
                  )}
                </p>
              )}

              <EvaluationTrace
                systemPolicies={result.system_policies_evaluated}
                orgPolicies={result.org_policies_evaluated}
              />
            </div>
          )}

          {simulateMutation.isError && (
            <div className="mt-4 pt-4 border-t">
              <div className="flex items-start gap-2 p-3 rounded-lg bg-destructive/10 border border-destructive/20">
                <XCircle className="h-4 w-4 text-destructive mt-0.5" />
                <p className="text-sm text-destructive">
                  Simulation failed. Please check your inputs and try again.
                </p>
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
