import { zodResolver } from "@hookform/resolvers/zod";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { createColumnHelper, type ColumnDef } from "@tanstack/react-table";
import { useParams, useNavigate, useSearchParams } from "react-router-dom";
import {
  ArrowLeft,
  Plus,
  Pencil,
  Trash2,
  Info,
  Users,
  Shield,
  FlaskConical,
  CheckCircle2,
  XCircle,
  Download,
  Upload,
  AlertTriangle,
} from "lucide-react";
import { useState, useEffect } from "react";
import { useForm, Controller } from "react-hook-form";
import { z } from "zod";

import {
  ssoGroupMappingListOptions,
  ssoGroupMappingCreateMutation,
  ssoGroupMappingUpdateMutation,
  ssoGroupMappingDeleteMutation,
  ssoGroupMappingTestMutation,
  ssoGroupMappingImportMutation,
  teamListOptions,
  organizationGetOptions,
} from "@/api/generated/@tanstack/react-query.gen";
import { ssoGroupMappingExport } from "@/api/generated/sdk.gen";
import type {
  ExportFormat,
  SsoGroupMapping,
  Team,
  TestMappingResult,
  ImportConflictStrategy,
  ImportResponse,
} from "@/api/generated/types.gen";
import { Button } from "@/components/Button/Button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/Card/Card";
import { CodeBadge } from "@/components/CodeBadge/CodeBadge";
import { DataTable } from "@/components/DataTable/DataTable";
import {
  Dropdown,
  DropdownTrigger,
  DropdownContent,
  DropdownItem,
} from "@/components/Dropdown/Dropdown";
import { FormField } from "@/components/FormField/FormField";
import { Input } from "@/components/Input/Input";
import { Modal, ModalHeader, ModalContent, ModalFooter } from "@/components/Modal/Modal";
import { Select } from "@/components/Select/Select";
import { Skeleton } from "@/components/Skeleton/Skeleton";
import { useToast } from "@/components/Toast/Toast";
import { useConfirm } from "@/components/ConfirmDialog/ConfirmDialog";
import { Badge } from "@/components/Badge/Badge";
import { formatDateTime } from "@/utils/formatters";

const columnHelper = createColumnHelper<SsoGroupMapping>();

// Form schema for creating/editing a mapping
const mappingFormSchema = z.object({
  idp_group: z.string().min(1, "IdP group name is required"),
  team_id: z.string().nullable(),
  role: z.string().nullable(),
  priority: z.number().int().min(0, "Priority must be 0 or higher"),
  sso_connection_name: z.string().min(1, "SSO connection is required"),
});

type MappingFormValues = z.infer<typeof mappingFormSchema>;

// Role options - common roles used in the system
const ROLE_OPTIONS = [
  { value: "admin", label: "Admin" },
  { value: "member", label: "Member" },
  { value: "viewer", label: "Viewer" },
];

// Form schema for testing mappings
const testFormSchema = z.object({
  idp_groups_text: z.string().min(1, "Enter at least one IdP group"),
  sso_connection_name: z.string().min(1, "SSO connection is required"),
  default_role: z.string().min(1, "Default role is required"),
});

type TestFormValues = z.infer<typeof testFormSchema>;

// Test results state
interface TestResults {
  resolved: TestMappingResult[];
  unmapped_groups: string[];
}

export default function SsoGroupMappingsPage() {
  const { orgSlug } = useParams<{ orgSlug: string }>();
  const [searchParams] = useSearchParams();
  const connectionFilter = searchParams.get("connection");
  const navigate = useNavigate();
  const { toast } = useToast();
  const confirm = useConfirm();
  const queryClient = useQueryClient();

  const [isCreateModalOpen, setIsCreateModalOpen] = useState(false);
  const [editingMapping, setEditingMapping] = useState<SsoGroupMapping | null>(null);
  const [isTestModalOpen, setIsTestModalOpen] = useState(false);
  const [testResults, setTestResults] = useState<TestResults | null>(null);
  const [isImportModalOpen, setIsImportModalOpen] = useState(false);
  const [importResults, setImportResults] = useState<ImportResponse | null>(null);
  const [importConflictStrategy, setImportConflictStrategy] =
    useState<ImportConflictStrategy>("skip");
  const [importFileContent, setImportFileContent] = useState<string | null>(null);
  const [importFileName, setImportFileName] = useState<string | null>(null);
  const [importParseError, setImportParseError] = useState<string | null>(null);

  const form = useForm<MappingFormValues>({
    resolver: zodResolver(mappingFormSchema),
    defaultValues: {
      idp_group: "",
      team_id: null,
      role: null,
      priority: 0,
      sso_connection_name: connectionFilter || "default",
    },
  });

  const testForm = useForm<TestFormValues>({
    resolver: zodResolver(testFormSchema),
    defaultValues: {
      idp_groups_text: "",
      sso_connection_name: connectionFilter || "default",
      default_role: "member",
    },
  });

  // Reset form when modal opens/closes
  useEffect(() => {
    if (isCreateModalOpen) {
      form.reset({
        idp_group: "",
        team_id: null,
        role: null,
        priority: 0,
        sso_connection_name: connectionFilter || "default",
      });
    }
  }, [isCreateModalOpen, form, connectionFilter]);

  useEffect(() => {
    if (editingMapping) {
      form.reset({
        idp_group: editingMapping.idp_group,
        team_id: editingMapping.team_id || null,
        role: editingMapping.role || null,
        priority: editingMapping.priority,
        sso_connection_name: editingMapping.sso_connection_name,
      });
    }
  }, [editingMapping, form]);

  // Reset test form when modal opens
  useEffect(() => {
    if (isTestModalOpen) {
      testForm.reset({
        idp_groups_text: "",
        sso_connection_name: connectionFilter || "default",
        default_role: "member",
      });
      setTestResults(null);
    }
  }, [isTestModalOpen, testForm, connectionFilter]);

  // Reset import modal state when modal opens
  useEffect(() => {
    if (isImportModalOpen) {
      setImportResults(null);
      setImportConflictStrategy("skip");
      setImportFileContent(null);
      setImportFileName(null);
      setImportParseError(null);
    }
  }, [isImportModalOpen]);

  // Fetch organization details
  const { data: org, isLoading: orgLoading } = useQuery(
    organizationGetOptions({ path: { slug: orgSlug! } })
  );

  // Fetch mappings
  const {
    data: mappings,
    isLoading: mappingsLoading,
    error: mappingsError,
  } = useQuery(ssoGroupMappingListOptions({ path: { org_slug: orgSlug! } }));

  // Fetch teams for the dropdown
  const { data: teams } = useQuery(teamListOptions({ path: { org_slug: orgSlug! } }));

  // Create mutation
  const createMutation = useMutation({
    ...ssoGroupMappingCreateMutation(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "ssoGroupMappingList" }] });
      setIsCreateModalOpen(false);
      toast({ title: "Group mapping created", type: "success" });
    },
    onError: (error) => {
      toast({ title: "Failed to create mapping", description: String(error), type: "error" });
    },
  });

  // Update mutation
  const updateMutation = useMutation({
    ...ssoGroupMappingUpdateMutation(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "ssoGroupMappingList" }] });
      setEditingMapping(null);
      toast({ title: "Group mapping updated", type: "success" });
    },
    onError: (error) => {
      toast({ title: "Failed to update mapping", description: String(error), type: "error" });
    },
  });

  // Delete mutation
  const deleteMutation = useMutation({
    ...ssoGroupMappingDeleteMutation(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "ssoGroupMappingList" }] });
      toast({ title: "Group mapping deleted", type: "success" });
    },
    onError: (error) => {
      toast({ title: "Failed to delete mapping", description: String(error), type: "error" });
    },
  });

  // Test mutation
  const testMutation = useMutation({
    ...ssoGroupMappingTestMutation(),
    onSuccess: (data) => {
      setTestResults(data);
    },
    onError: (error) => {
      toast({ title: "Failed to test mappings", description: String(error), type: "error" });
    },
  });

  // Import mutation
  const importMutation = useMutation({
    ...ssoGroupMappingImportMutation(),
    onSuccess: (data) => {
      setImportResults(data);
      queryClient.invalidateQueries({ queryKey: [{ _id: "ssoGroupMappingList" }] });
      if (data.errors.length === 0) {
        toast({
          title: "Import completed",
          description: `Created: ${data.created}, Updated: ${data.updated}, Skipped: ${data.skipped}`,
          type: "success",
        });
      } else {
        toast({
          title: "Import completed with errors",
          description: `Created: ${data.created}, Errors: ${data.errors.length}`,
          type: "warning",
        });
      }
    },
    onError: (error) => {
      toast({ title: "Failed to import mappings", description: String(error), type: "error" });
    },
  });

  // Export state
  const [isExporting, setIsExporting] = useState(false);

  const handleExport = async (format: ExportFormat) => {
    if (!orgSlug) return;

    setIsExporting(true);
    try {
      const response = await ssoGroupMappingExport({
        path: { org_slug: orgSlug },
        query: {
          format,
          sso_connection_name: connectionFilter || undefined,
        },
      });

      if (response.error) {
        throw new Error(String(response.error));
      }

      const timestamp = new Date().toISOString().slice(0, 10);
      const filename = connectionFilter
        ? `sso-group-mappings-${orgSlug}-${connectionFilter}-${timestamp}`
        : `sso-group-mappings-${orgSlug}-${timestamp}`;

      if (format === "csv") {
        // CSV response - download as text file
        const blob = new Blob([response.data as string], { type: "text/csv" });
        const url = URL.createObjectURL(blob);
        const link = document.createElement("a");
        link.href = url;
        link.download = `${filename}.csv`;
        document.body.appendChild(link);
        link.click();
        document.body.removeChild(link);
        URL.revokeObjectURL(url);
      } else {
        // JSON response - stringify and download
        const content = JSON.stringify(response.data, null, 2);
        const blob = new Blob([content], { type: "application/json" });
        const url = URL.createObjectURL(blob);
        const link = document.createElement("a");
        link.href = url;
        link.download = `${filename}.json`;
        document.body.appendChild(link);
        link.click();
        document.body.removeChild(link);
        URL.revokeObjectURL(url);
      }

      toast({ title: `Exported as ${format.toUpperCase()}`, type: "success" });
    } catch (error) {
      toast({ title: "Failed to export mappings", description: String(error), type: "error" });
    } finally {
      setIsExporting(false);
    }
  };

  const handleCreate = (data: MappingFormValues) => {
    createMutation.mutate({
      path: { org_slug: orgSlug! },
      body: {
        idp_group: data.idp_group,
        team_id: data.team_id,
        role: data.role,
        priority: data.priority,
        sso_connection_name: data.sso_connection_name,
      },
    });
  };

  const handleUpdate = (data: MappingFormValues) => {
    if (!editingMapping) return;
    updateMutation.mutate({
      path: { org_slug: orgSlug!, mapping_id: editingMapping.id },
      body: {
        idp_group: data.idp_group,
        team_id: data.team_id,
        role: data.role,
        priority: data.priority,
      },
    });
  };

  const handleDelete = async (mapping: SsoGroupMapping) => {
    const confirmed = await confirm({
      title: "Delete Group Mapping",
      message: `Are you sure you want to delete the mapping for "${mapping.idp_group}"? Users with this IdP group will no longer be automatically added to the mapped team.`,
      confirmLabel: "Delete",
      variant: "destructive",
    });
    if (confirmed) {
      deleteMutation.mutate({
        path: { org_slug: orgSlug!, mapping_id: mapping.id },
      });
    }
  };

  const handleTest = (data: TestFormValues) => {
    // Parse IdP groups from text (comma or newline separated)
    const idpGroups = data.idp_groups_text
      .split(/[,\n]/)
      .map((g) => g.trim())
      .filter((g) => g.length > 0);

    if (idpGroups.length === 0) {
      toast({ title: "Enter at least one IdP group", type: "error" });
      return;
    }

    testMutation.mutate({
      path: { org_slug: orgSlug! },
      body: {
        idp_groups: idpGroups,
        sso_connection_name: data.sso_connection_name,
        default_role: data.default_role,
      },
    });
  };

  const handleFileSelect = (event: React.ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    if (!file) return;

    setImportFileName(file.name);
    setImportParseError(null);

    const reader = new FileReader();
    reader.onload = (e) => {
      const content = e.target?.result as string;
      try {
        // Validate JSON structure
        const parsed = JSON.parse(content);
        // Check if it's an export file (has mappings array) or direct array
        const mappings = parsed.mappings || parsed;
        if (!Array.isArray(mappings)) {
          throw new Error("Expected an array of mappings or an export file with 'mappings' array");
        }
        if (mappings.length === 0) {
          throw new Error("No mappings found in file");
        }
        // Validate each mapping has required fields
        for (let i = 0; i < mappings.length; i++) {
          if (!mappings[i].idp_group) {
            throw new Error(`Mapping at index ${i} is missing required 'idp_group' field`);
          }
        }
        setImportFileContent(content);
      } catch (err) {
        setImportParseError(err instanceof Error ? err.message : "Invalid JSON file");
        setImportFileContent(null);
      }
    };
    reader.onerror = () => {
      setImportParseError("Failed to read file");
      setImportFileContent(null);
    };
    reader.readAsText(file);
  };

  const handleImport = () => {
    if (!orgSlug || !importFileContent) return;

    try {
      const parsed = JSON.parse(importFileContent);
      // Support both export file format and direct array
      const mappings = parsed.mappings || parsed;

      importMutation.mutate({
        path: { org_slug: orgSlug },
        body: {
          mappings: mappings.map((m: Record<string, unknown>) => ({
            idp_group: m.idp_group,
            team_id: m.team_id || null,
            role: m.role || null,
            priority: typeof m.priority === "number" ? m.priority : 0,
            sso_connection_name: m.sso_connection_name || connectionFilter || "default",
          })),
          on_conflict: importConflictStrategy,
        },
      });
    } catch (err) {
      toast({ title: "Failed to parse file", description: String(err), type: "error" });
    }
  };

  // Filter mappings by connection if query param is set
  const filteredMappings =
    connectionFilter && mappings?.data
      ? mappings.data.filter((m) => m.sso_connection_name === connectionFilter)
      : mappings?.data || [];

  // Get team name by ID
  const getTeamName = (teamId: string | null | undefined): string => {
    if (!teamId) return "-";
    const team = teams?.data?.find((t: Team) => t.id === teamId);
    return team?.name || teamId;
  };

  // Column definitions
  const columns = [
    columnHelper.accessor("idp_group", {
      header: "IdP Group",
      cell: (info) => <CodeBadge>{info.getValue()}</CodeBadge>,
    }),
    columnHelper.accessor("team_id", {
      header: "Team",
      cell: (info) => {
        const teamId = info.getValue();
        if (!teamId) {
          return <span className="text-muted-foreground">-</span>;
        }
        return (
          <span className="flex items-center gap-1">
            <Users className="h-3 w-3" />
            {getTeamName(teamId)}
          </span>
        );
      },
    }),
    columnHelper.accessor("role", {
      header: "Role",
      cell: (info) => {
        const role = info.getValue();
        if (!role) {
          return <span className="text-muted-foreground">-</span>;
        }
        return <Badge variant="secondary">{role}</Badge>;
      },
    }),
    columnHelper.accessor("priority", {
      header: "Priority",
      cell: (info) => {
        const priority = info.getValue();
        if (priority === 0) {
          return <span className="text-muted-foreground">0</span>;
        }
        return <Badge variant="outline">{priority}</Badge>;
      },
    }),
    columnHelper.accessor("sso_connection_name", {
      header: "SSO Connection",
      cell: (info) => <Badge variant="outline">{info.getValue()}</Badge>,
    }),
    columnHelper.accessor("created_at", {
      header: "Created",
      cell: (info) => formatDateTime(info.getValue()),
    }),
    columnHelper.display({
      id: "actions",
      header: () => <span className="sr-only">Actions</span>,
      cell: ({ row }) => (
        <div className="flex items-center gap-1">
          <Button
            variant="ghost"
            size="sm"
            onClick={() => setEditingMapping(row.original)}
            title="Edit mapping"
          >
            <Pencil className="h-4 w-4" />
          </Button>
          <Button
            variant="ghost"
            size="sm"
            className="text-destructive"
            onClick={() => handleDelete(row.original)}
            title="Delete mapping"
          >
            <Trash2 className="h-4 w-4" />
          </Button>
        </div>
      ),
    }),
  ];

  if (orgLoading) {
    return (
      <div className="p-6 space-y-6">
        <Skeleton className="h-8 w-64" />
        <Skeleton className="h-32 w-full" />
      </div>
    );
  }

  return (
    <div className="p-6 space-y-6">
      {/* Header */}
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="sm" onClick={() => navigate("/admin/sso")}>
          <ArrowLeft className="mr-2 h-4 w-4" />
          Back to SSO
        </Button>
      </div>

      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold flex items-center gap-2">
            <Shield className="h-6 w-6" />
            SSO Group Mappings
          </h1>
          <p className="text-muted-foreground mt-1">
            {org?.name && <span>{org.name} • </span>}
            Map IdP groups to teams and roles
            {connectionFilter && (
              <span>
                {" "}
                • Filtered by <CodeBadge>{connectionFilter}</CodeBadge>
              </span>
            )}
          </p>
        </div>
        <div className="flex gap-2">
          <Dropdown>
            <DropdownTrigger disabled={isExporting}>
              <Download className="mr-2 h-4 w-4" />
              {isExporting ? "Exporting..." : "Export"}
            </DropdownTrigger>
            <DropdownContent align="end">
              <DropdownItem onClick={() => handleExport("json")}>Export as JSON</DropdownItem>
              <DropdownItem onClick={() => handleExport("csv")}>Export as CSV</DropdownItem>
            </DropdownContent>
          </Dropdown>
          <Button variant="outline" onClick={() => setIsImportModalOpen(true)}>
            <Upload className="mr-2 h-4 w-4" />
            Import
          </Button>
          <Button variant="outline" onClick={() => setIsTestModalOpen(true)}>
            <FlaskConical className="mr-2 h-4 w-4" />
            Test Mapping
          </Button>
          <Button onClick={() => setIsCreateModalOpen(true)}>
            <Plus className="mr-2 h-4 w-4" />
            Add Mapping
          </Button>
        </div>
      </div>

      {/* Info banner */}
      <div className="flex items-start gap-3 rounded-lg border bg-muted/30 p-4">
        <Info className="h-5 w-5 text-muted-foreground mt-0.5" />
        <div className="text-sm">
          <p className="font-medium">How group mappings work</p>
          <p className="text-muted-foreground">
            When users authenticate via SSO, their IdP groups are matched against these mappings.
            Matching users are automatically added to the specified teams with the assigned role. If
            no role is specified, the default role from the SSO connection config is used.
          </p>
        </div>
      </div>

      {/* Error state */}
      {mappingsError && (
        <Card className="border-destructive">
          <CardContent className="flex items-center gap-3 p-6">
            <div>
              <p className="font-medium">Failed to load group mappings</p>
              <p className="text-sm text-muted-foreground">{String(mappingsError)}</p>
            </div>
          </CardContent>
        </Card>
      )}

      {/* Mappings table */}
      <Card>
        <CardHeader>
          <CardTitle>Group Mappings</CardTitle>
        </CardHeader>
        <CardContent>
          <DataTable
            columns={columns as ColumnDef<SsoGroupMapping>[]}
            data={filteredMappings}
            isLoading={mappingsLoading}
            emptyMessage={
              connectionFilter
                ? `No group mappings for SSO connection "${connectionFilter}".`
                : "No group mappings configured. Add a mapping to automatically assign users to teams based on their IdP groups."
            }
            searchColumn="idp_group"
            searchPlaceholder="Search by IdP group..."
          />
        </CardContent>
      </Card>

      {/* Create Modal */}
      <MappingFormModal
        open={isCreateModalOpen}
        onClose={() => setIsCreateModalOpen(false)}
        onSubmit={handleCreate}
        form={form}
        teams={teams?.data || []}
        isLoading={createMutation.isPending}
        title="Add Group Mapping"
        submitLabel="Create"
        showConnectionField={!connectionFilter}
      />

      {/* Edit Modal */}
      <MappingFormModal
        open={!!editingMapping}
        onClose={() => setEditingMapping(null)}
        onSubmit={handleUpdate}
        form={form}
        teams={teams?.data || []}
        isLoading={updateMutation.isPending}
        title="Edit Group Mapping"
        submitLabel="Save"
        showConnectionField={false}
      />

      {/* Test Modal */}
      <TestMappingModal
        open={isTestModalOpen}
        onClose={() => setIsTestModalOpen(false)}
        onSubmit={handleTest}
        form={testForm}
        isLoading={testMutation.isPending}
        results={testResults}
        showConnectionField={!connectionFilter}
      />

      {/* Import Modal */}
      <ImportMappingModal
        open={isImportModalOpen}
        onClose={() => setIsImportModalOpen(false)}
        onImport={handleImport}
        onFileSelect={handleFileSelect}
        fileName={importFileName}
        parseError={importParseError}
        fileContent={importFileContent}
        conflictStrategy={importConflictStrategy}
        onConflictStrategyChange={setImportConflictStrategy}
        isLoading={importMutation.isPending}
        results={importResults}
      />
    </div>
  );
}

// Form modal component
interface MappingFormModalProps {
  open: boolean;
  onClose: () => void;
  onSubmit: (data: MappingFormValues) => void;
  form: ReturnType<typeof useForm<MappingFormValues>>;
  teams: Team[];
  isLoading: boolean;
  title: string;
  submitLabel: string;
  showConnectionField: boolean;
}

function MappingFormModal({
  open,
  onClose,
  onSubmit,
  form,
  teams,
  isLoading,
  title,
  submitLabel,
  showConnectionField,
}: MappingFormModalProps) {
  const teamOptions = [
    { value: "", label: "No team (org-level role only)" },
    ...teams.map((team) => ({
      value: team.id,
      label: team.name,
    })),
  ];

  const roleOptions = [{ value: "", label: "Use default role" }, ...ROLE_OPTIONS];

  return (
    <Modal open={open} onClose={onClose}>
      <form onSubmit={form.handleSubmit(onSubmit)}>
        <ModalHeader>{title}</ModalHeader>
        <ModalContent>
          <div className="space-y-4">
            <FormField
              label="IdP Group"
              htmlFor="idp_group"
              required
              error={form.formState.errors.idp_group?.message}
              helpText="Enter the exact group name as it appears in your identity provider's groups claim"
            >
              <Input
                id="idp_group"
                {...form.register("idp_group")}
                placeholder="e.g., Engineering, Admins, engineering@company.com"
              />
            </FormField>

            <FormField label="Team" htmlFor="team_id" helpText="Team to add matching users to">
              <Controller
                name="team_id"
                control={form.control}
                render={({ field }) => (
                  <Select
                    value={field.value || ""}
                    onChange={(value) => field.onChange(value || null)}
                    placeholder="Select a team..."
                    options={teamOptions}
                  />
                )}
              />
            </FormField>

            <FormField
              label="Role"
              htmlFor="role"
              helpText="Role to assign within the team (or org if no team selected)"
            >
              <Controller
                name="role"
                control={form.control}
                render={({ field }) => (
                  <Select
                    value={field.value || ""}
                    onChange={(value) => field.onChange(value || null)}
                    placeholder="Select a role..."
                    options={roleOptions}
                  />
                )}
              />
            </FormField>

            <FormField
              label="Priority"
              htmlFor="priority"
              error={form.formState.errors.priority?.message}
              helpText="Higher priority wins when multiple mappings target the same team (default: 0)"
            >
              <Controller
                name="priority"
                control={form.control}
                render={({ field }) => (
                  <Input
                    id="priority"
                    type="number"
                    min={0}
                    step={1}
                    value={field.value}
                    onChange={(e) => field.onChange(parseInt(e.target.value, 10) || 0)}
                    placeholder="0"
                  />
                )}
              />
            </FormField>

            {showConnectionField && (
              <FormField
                label="SSO Connection"
                htmlFor="sso_connection_name"
                required
                error={form.formState.errors.sso_connection_name?.message}
                helpText="Which SSO connection this mapping applies to"
              >
                <Input
                  id="sso_connection_name"
                  {...form.register("sso_connection_name")}
                  placeholder="default"
                />
              </FormField>
            )}
          </div>
        </ModalContent>
        <ModalFooter>
          <Button type="button" variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button type="submit" isLoading={isLoading}>
            {submitLabel}
          </Button>
        </ModalFooter>
      </form>
    </Modal>
  );
}

// Test modal component
interface TestMappingModalProps {
  open: boolean;
  onClose: () => void;
  onSubmit: (data: TestFormValues) => void;
  form: ReturnType<typeof useForm<TestFormValues>>;
  isLoading: boolean;
  results: TestResults | null;
  showConnectionField: boolean;
}

function TestMappingModal({
  open,
  onClose,
  onSubmit,
  form,
  isLoading,
  results,
  showConnectionField,
}: TestMappingModalProps) {
  const roleOptions = ROLE_OPTIONS;

  return (
    <Modal open={open} onClose={onClose}>
      <form onSubmit={form.handleSubmit(onSubmit)}>
        <ModalHeader>Test Group Mapping</ModalHeader>
        <ModalContent>
          <div className="space-y-4">
            <FormField
              label="IdP Groups"
              htmlFor="idp_groups_text"
              required
              error={form.formState.errors.idp_groups_text?.message}
              helpText="Enter IdP group names (comma or newline separated) to see what teams they would resolve to"
            >
              <textarea
                id="idp_groups_text"
                {...form.register("idp_groups_text")}
                placeholder={"Engineering\nPlatform\nAdmins"}
                rows={4}
                className="flex min-h-[80px] w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
              />
            </FormField>

            {showConnectionField && (
              <FormField
                label="SSO Connection"
                htmlFor="test_sso_connection_name"
                required
                error={form.formState.errors.sso_connection_name?.message}
                helpText="Which SSO connection to test against"
              >
                <Input
                  id="test_sso_connection_name"
                  {...form.register("sso_connection_name")}
                  placeholder="default"
                />
              </FormField>
            )}

            <FormField
              label="Default Role"
              htmlFor="default_role"
              helpText="Role to use when a mapping doesn't specify one"
            >
              <Controller
                name="default_role"
                control={form.control}
                render={({ field }) => (
                  <Select
                    value={field.value}
                    onChange={field.onChange}
                    placeholder="Select a role..."
                    options={roleOptions}
                  />
                )}
              />
            </FormField>

            {/* Results section */}
            {results && (
              <div className="space-y-4 pt-4 border-t">
                <h4 className="font-medium">Results</h4>

                {/* Resolved mappings */}
                {results.resolved.length > 0 ? (
                  <div className="space-y-2">
                    <p className="text-sm text-muted-foreground flex items-center gap-1">
                      <CheckCircle2 className="h-4 w-4 text-green-500" />
                      Resolved ({results.resolved.length})
                    </p>
                    <div className="rounded-md border">
                      <table className="w-full text-sm">
                        <thead>
                          <tr className="border-b bg-muted/50">
                            <th className="px-3 py-2 text-left font-medium">IdP Group</th>
                            <th className="px-3 py-2 text-left font-medium">Team</th>
                            <th className="px-3 py-2 text-left font-medium">Role</th>
                          </tr>
                        </thead>
                        <tbody>
                          {results.resolved.map((r, i) => (
                            <tr key={i} className="border-b last:border-0">
                              <td className="px-3 py-2">
                                <CodeBadge>{r.idp_group}</CodeBadge>
                              </td>
                              <td className="px-3 py-2 flex items-center gap-1">
                                <Users className="h-3 w-3" />
                                {r.team_name}
                              </td>
                              <td className="px-3 py-2">
                                <Badge variant="secondary">{r.role}</Badge>
                              </td>
                            </tr>
                          ))}
                        </tbody>
                      </table>
                    </div>
                  </div>
                ) : (
                  <p className="text-sm text-muted-foreground">No groups matched any mappings.</p>
                )}

                {/* Unmapped groups */}
                {results.unmapped_groups.length > 0 && (
                  <div className="space-y-2">
                    <p className="text-sm text-muted-foreground flex items-center gap-1">
                      <XCircle className="h-4 w-4 text-yellow-500" />
                      Unmapped ({results.unmapped_groups.length})
                    </p>
                    <div className="flex flex-wrap gap-2">
                      {results.unmapped_groups.map((g, i) => (
                        <CodeBadge key={i}>{g}</CodeBadge>
                      ))}
                    </div>
                  </div>
                )}
              </div>
            )}
          </div>
        </ModalContent>
        <ModalFooter>
          <Button type="button" variant="ghost" onClick={onClose}>
            Close
          </Button>
          <Button type="submit" isLoading={isLoading}>
            <FlaskConical className="mr-2 h-4 w-4" />
            Test
          </Button>
        </ModalFooter>
      </form>
    </Modal>
  );
}

// Import modal component
interface ImportMappingModalProps {
  open: boolean;
  onClose: () => void;
  onImport: () => void;
  onFileSelect: (event: React.ChangeEvent<HTMLInputElement>) => void;
  fileName: string | null;
  parseError: string | null;
  fileContent: string | null;
  conflictStrategy: ImportConflictStrategy;
  onConflictStrategyChange: (strategy: ImportConflictStrategy) => void;
  isLoading: boolean;
  results: ImportResponse | null;
}

const CONFLICT_STRATEGY_OPTIONS = [
  { value: "skip", label: "Skip existing", description: "Skip mappings that already exist" },
  {
    value: "overwrite",
    label: "Overwrite",
    description: "Update existing mappings with imported values",
  },
  { value: "error", label: "Error on conflict", description: "Fail if any mapping already exists" },
];

function ImportMappingModal({
  open,
  onClose,
  onImport,
  onFileSelect,
  fileName,
  parseError,
  fileContent,
  conflictStrategy,
  onConflictStrategyChange,
  isLoading,
  results,
}: ImportMappingModalProps) {
  // Count mappings in file
  let mappingCount = 0;
  if (fileContent) {
    try {
      const parsed = JSON.parse(fileContent);
      const mappings = parsed.mappings || parsed;
      mappingCount = Array.isArray(mappings) ? mappings.length : 0;
    } catch {
      // Ignore parse errors here, they're handled elsewhere
    }
  }

  return (
    <Modal open={open} onClose={onClose}>
      <ModalHeader>Import Group Mappings</ModalHeader>
      <ModalContent>
        <div className="space-y-4">
          {/* File picker */}
          <FormField
            label="JSON File"
            htmlFor="import_file"
            required
            error={parseError || undefined}
            helpText="Upload a JSON file exported from this page or a JSON array of mappings"
          >
            <div className="flex items-center gap-3">
              <label
                htmlFor="import_file"
                className="flex cursor-pointer items-center gap-2 rounded-md border border-input bg-background px-4 py-2 text-sm font-medium hover:bg-accent hover:text-accent-foreground"
              >
                <Upload className="h-4 w-4" />
                Choose File
              </label>
              <input
                id="import_file"
                type="file"
                accept=".json,application/json"
                onChange={onFileSelect}
                className="hidden"
              />
              {fileName && (
                <span className="text-sm text-muted-foreground">
                  {fileName}
                  {fileContent && (
                    <span className="ml-2 text-green-700">
                      ({mappingCount} mapping{mappingCount !== 1 ? "s" : ""})
                    </span>
                  )}
                </span>
              )}
            </div>
          </FormField>

          {/* Conflict strategy */}
          <FormField
            label="Conflict Strategy"
            htmlFor="conflict_strategy"
            helpText="How to handle mappings that already exist"
          >
            <Select
              value={conflictStrategy}
              onChange={(value) => onConflictStrategyChange(value as ImportConflictStrategy)}
              options={CONFLICT_STRATEGY_OPTIONS}
            />
          </FormField>

          {/* Results section */}
          {results && (
            <div className="space-y-3 pt-4 border-t">
              <h4 className="font-medium">Import Results</h4>

              <div className="grid grid-cols-3 gap-4 text-center">
                <div className="rounded-lg border bg-green-50 dark:bg-green-950/30 p-3">
                  <div className="text-2xl font-bold text-green-700">{results.created}</div>
                  <div className="text-xs text-muted-foreground">Created</div>
                </div>
                <div className="rounded-lg border bg-blue-50 dark:bg-blue-950/30 p-3">
                  <div className="text-2xl font-bold text-blue-700">{results.updated}</div>
                  <div className="text-xs text-muted-foreground">Updated</div>
                </div>
                <div className="rounded-lg border bg-gray-50 dark:bg-gray-800/30 p-3">
                  <div className="text-2xl font-bold text-gray-600">{results.skipped}</div>
                  <div className="text-xs text-muted-foreground">Skipped</div>
                </div>
              </div>

              {/* Errors */}
              {results.errors.length > 0 && (
                <div className="space-y-2">
                  <p className="text-sm text-destructive flex items-center gap-1">
                    <AlertTriangle className="h-4 w-4" />
                    Errors ({results.errors.length})
                  </p>
                  <div className="max-h-32 overflow-y-auto rounded-md border border-destructive/50 bg-destructive/5 p-2">
                    {results.errors.map((err, i) => (
                      <div key={i} className="text-sm py-1">
                        <span className="font-medium">{err.idp_group}</span>
                        <span className="text-muted-foreground">: {err.error}</span>
                      </div>
                    ))}
                  </div>
                </div>
              )}
            </div>
          )}
        </div>
      </ModalContent>
      <ModalFooter>
        <Button type="button" variant="ghost" onClick={onClose}>
          {results ? "Close" : "Cancel"}
        </Button>
        {!results && (
          <Button onClick={onImport} isLoading={isLoading} disabled={!fileContent || isLoading}>
            <Upload className="mr-2 h-4 w-4" />
            Import
          </Button>
        )}
      </ModalFooter>
    </Modal>
  );
}
