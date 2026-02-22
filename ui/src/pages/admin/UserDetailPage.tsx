import { zodResolver } from "@hookform/resolvers/zod";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { createColumnHelper, type ColumnDef } from "@tanstack/react-table";
import { useParams, useNavigate } from "react-router-dom";
import { ArrowLeft, Key, Server, DollarSign, Pencil, Monitor, BarChart3 } from "lucide-react";
import { useState } from "react";
import { useForm } from "react-hook-form";
import { z } from "zod";

import {
  userGetOptions,
  userUpdateMutation,
  apiKeyListByUserOptions,
  dynamicProviderListByUserOptions,
  modelPricingListByUserOptions,
  userSessionsListOptions,
} from "@/api/generated/@tanstack/react-query.gen";
import type { ApiKey, DynamicProvider, DbModelPricing } from "@/api/generated/types.gen";
import { Button } from "@/components/Button/Button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/Card/Card";
import { DataTable } from "@/components/DataTable/DataTable";
import { FormField } from "@/components/FormField/FormField";
import { Input } from "@/components/Input/Input";
import { Modal, ModalHeader, ModalContent, ModalFooter } from "@/components/Modal/Modal";
import { Skeleton } from "@/components/Skeleton/Skeleton";
import { Badge } from "@/components/Badge/Badge";
import { useToast } from "@/components/Toast/Toast";
import { formatDateTime, formatCurrency } from "@/utils/formatters";
import { SessionsPanel } from "@/components/Admin";
import UsageDashboard from "@/components/UsageDashboard/UsageDashboard";

type TabId = "api-keys" | "providers" | "pricing" | "sessions" | "usage";

const tabs: { id: TabId; label: string; icon: React.ReactNode }[] = [
  { id: "api-keys", label: "API Keys", icon: <Key className="h-4 w-4" /> },
  { id: "providers", label: "Providers", icon: <Server className="h-4 w-4" /> },
  { id: "pricing", label: "Pricing", icon: <DollarSign className="h-4 w-4" /> },
  { id: "usage", label: "Usage", icon: <BarChart3 className="h-4 w-4" /> },
  { id: "sessions", label: "Sessions", icon: <Monitor className="h-4 w-4" /> },
];

const apiKeyColumnHelper = createColumnHelper<ApiKey>();
const providerColumnHelper = createColumnHelper<DynamicProvider>();
const pricingColumnHelper = createColumnHelper<DbModelPricing>();

const editUserSchema = z.object({
  name: z.string(),
  email: z.string().email("Invalid email address").or(z.literal("")),
});

type EditUserForm = z.infer<typeof editUserSchema>;

export default function UserDetailPage() {
  const { userId } = useParams<{ userId: string }>();
  const navigate = useNavigate();
  const { toast } = useToast();
  const queryClient = useQueryClient();

  const [activeTab, setActiveTab] = useState<TabId>("api-keys");
  const [isEditModalOpen, setIsEditModalOpen] = useState(false);

  const editForm = useForm<EditUserForm>({
    resolver: zodResolver(editUserSchema),
    defaultValues: { name: "", email: "" },
  });

  // Fetch user details
  const {
    data: user,
    isLoading: userLoading,
    error: userError,
  } = useQuery(userGetOptions({ path: { user_id: userId! } }));

  // Fetch API keys
  const { data: apiKeys, isLoading: apiKeysLoading } = useQuery({
    ...apiKeyListByUserOptions({ path: { user_id: userId! } }),
    enabled: activeTab === "api-keys",
  });

  // Fetch providers
  const { data: providers, isLoading: providersLoading } = useQuery({
    ...dynamicProviderListByUserOptions({ path: { user_id: userId! } }),
    enabled: activeTab === "providers",
  });

  // Fetch pricing
  const { data: pricing, isLoading: pricingLoading } = useQuery({
    ...modelPricingListByUserOptions({ path: { user_id: userId! } }),
    enabled: activeTab === "pricing",
  });

  // Fetch sessions
  const { data: sessions, isLoading: sessionsLoading } = useQuery({
    ...userSessionsListOptions({ path: { user_id: userId! } }),
    enabled: activeTab === "sessions",
  });

  // Update mutation
  const updateMutation = useMutation({
    ...userUpdateMutation(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [{ _id: "userGet" }] });
      setIsEditModalOpen(false);
      toast({ title: "User updated", type: "success" });
    },
    onError: (error) => {
      toast({ title: "Failed to update user", description: String(error), type: "error" });
    },
  });

  const onEditSubmit = (data: EditUserForm) => {
    updateMutation.mutate({
      path: { user_id: userId! },
      body: {
        name: data.name || null,
        email: data.email || null,
      },
    });
  };

  // Column definitions
  const apiKeyColumns = [
    apiKeyColumnHelper.accessor("name", {
      header: "Name",
      cell: (info) => <span className="font-medium">{info.getValue()}</span>,
    }),
    apiKeyColumnHelper.accessor("key_prefix", {
      header: "Key Prefix",
      cell: (info) => (
        <code className="rounded bg-muted px-1.5 py-0.5 text-sm">{info.getValue()}...</code>
      ),
    }),
    apiKeyColumnHelper.accessor("revoked_at", {
      header: "Status",
      cell: (info) =>
        info.getValue() ? (
          <Badge variant="destructive">Revoked</Badge>
        ) : (
          <Badge variant="success">Active</Badge>
        ),
    }),
    apiKeyColumnHelper.accessor("budget_limit_cents", {
      header: "Budget",
      cell: (info) => {
        const limit = info.getValue();
        if (!limit) return "-";
        const period = info.row.original.budget_period;
        return `${formatCurrency(limit / 100)}/${period || "month"}`;
      },
    }),
    apiKeyColumnHelper.accessor("created_at", {
      header: "Created",
      cell: (info) => formatDateTime(info.getValue()),
    }),
  ];

  const providerColumns = [
    providerColumnHelper.accessor("name", {
      header: "Name",
      cell: (info) => <span className="font-medium">{info.getValue()}</span>,
    }),
    providerColumnHelper.accessor("provider_type", {
      header: "Type",
      cell: (info) => <Badge variant="secondary">{info.getValue()}</Badge>,
    }),
    providerColumnHelper.accessor("base_url", {
      header: "Base URL",
      cell: (info) => (
        <code className="rounded bg-muted px-1.5 py-0.5 text-sm">{info.getValue()}</code>
      ),
    }),
    providerColumnHelper.accessor("is_enabled", {
      header: "Status",
      cell: (info) =>
        info.getValue() ? (
          <Badge variant="success">Enabled</Badge>
        ) : (
          <Badge variant="secondary">Disabled</Badge>
        ),
    }),
  ];

  const pricingColumns = [
    pricingColumnHelper.accessor("model", {
      header: "Model",
      cell: (info) => <span className="font-medium">{info.getValue()}</span>,
    }),
    pricingColumnHelper.accessor("provider", {
      header: "Provider",
      cell: (info) => <Badge variant="secondary">{info.getValue()}</Badge>,
    }),
    pricingColumnHelper.accessor("input_per_1m_tokens", {
      header: "Input/1M",
      cell: (info) => formatCurrency(info.getValue() / 1_000_000),
    }),
    pricingColumnHelper.accessor("output_per_1m_tokens", {
      header: "Output/1M",
      cell: (info) => formatCurrency(info.getValue() / 1_000_000),
    }),
    pricingColumnHelper.accessor("source", {
      header: "Source",
      cell: (info) => <Badge variant="outline">{info.getValue()}</Badge>,
    }),
  ];

  if (userLoading) {
    return (
      <div className="p-6 space-y-6">
        <Skeleton className="h-8 w-64" />
        <Skeleton className="h-32 w-full" />
      </div>
    );
  }

  if (userError || !user) {
    return (
      <div className="p-6">
        <div className="text-center py-12 text-destructive">
          User not found or failed to load.
          <br />
          <Button variant="ghost" onClick={() => navigate("/admin/users")} className="mt-4">
            <ArrowLeft className="mr-2 h-4 w-4" />
            Back to Users
          </Button>
        </div>
      </div>
    );
  }

  return (
    <div className="p-6 space-y-6">
      {/* Header */}
      <div className="flex items-center gap-4">
        <Button
          variant="ghost"
          size="icon"
          onClick={() => navigate("/admin/users")}
          aria-label="Back to users"
        >
          <ArrowLeft className="h-4 w-4" />
        </Button>
        <div className="flex-1">
          <div className="flex items-center gap-3">
            <h1 className="text-2xl font-semibold">
              {user.name || user.email || user.external_id}
            </h1>
          </div>
          <div className="flex items-center gap-3 text-muted-foreground text-sm mt-1">
            {user.email && <span>{user.email}</span>}
            <code className="rounded bg-muted px-2 py-0.5 text-xs">{user.external_id}</code>
            <span>Created {formatDateTime(user.created_at)}</span>
          </div>
        </div>
        <Button
          variant="outline"
          onClick={() => {
            editForm.reset({ name: user.name || "", email: user.email || "" });
            setIsEditModalOpen(true);
          }}
        >
          <Pencil className="mr-2 h-4 w-4" />
          Edit
        </Button>
      </div>

      {/* Tabs */}
      <div className="border-b">
        <nav className="flex gap-4" aria-label="Tabs">
          {tabs.map((tab) => (
            <button
              key={tab.id}
              onClick={() => setActiveTab(tab.id)}
              className={`flex items-center gap-2 border-b-2 px-1 py-3 text-sm font-medium transition-colors ${
                activeTab === tab.id
                  ? "border-primary text-primary"
                  : "border-transparent text-muted-foreground hover:border-muted-foreground/30 hover:text-foreground"
              }`}
            >
              {tab.icon}
              {tab.label}
            </button>
          ))}
        </nav>
      </div>

      {/* Tab Content */}
      <Card>
        <CardHeader>
          <CardTitle>{tabs.find((t) => t.id === activeTab)?.label}</CardTitle>
        </CardHeader>
        <CardContent>
          {activeTab === "api-keys" && (
            <DataTable
              columns={apiKeyColumns as ColumnDef<ApiKey>[]}
              data={apiKeys?.data || []}
              isLoading={apiKeysLoading}
              emptyMessage="No API keys for this user."
              searchColumn="name"
              searchPlaceholder="Search API keys..."
            />
          )}
          {activeTab === "providers" && (
            <DataTable
              columns={providerColumns as ColumnDef<DynamicProvider>[]}
              data={providers?.data || []}
              isLoading={providersLoading}
              emptyMessage="No dynamic providers for this user."
              searchColumn="name"
              searchPlaceholder="Search providers..."
            />
          )}
          {activeTab === "pricing" && (
            <DataTable
              columns={pricingColumns as ColumnDef<DbModelPricing>[]}
              data={pricing?.data || []}
              isLoading={pricingLoading}
              emptyMessage="No custom pricing for this user."
              searchColumn="model"
              searchPlaceholder="Search models..."
            />
          )}
          {activeTab === "usage" && userId && <UsageDashboard scope={{ type: "user", userId }} />}
          {activeTab === "sessions" &&
            (sessionsLoading ? (
              <div className="space-y-3">
                <Skeleton className="h-24 w-full" />
                <Skeleton className="h-24 w-full" />
              </div>
            ) : sessions ? (
              <SessionsPanel userId={userId!} sessions={sessions} />
            ) : null)}
        </CardContent>
      </Card>

      {/* Edit Modal */}
      <Modal open={isEditModalOpen} onClose={() => setIsEditModalOpen(false)}>
        <form onSubmit={editForm.handleSubmit(onEditSubmit)}>
          <ModalHeader>Edit User</ModalHeader>
          <ModalContent>
            <div className="space-y-4">
              <FormField label="Name" htmlFor="name">
                <Input id="name" {...editForm.register("name")} placeholder="User Name" />
              </FormField>
              <FormField
                label="Email"
                htmlFor="email"
                error={editForm.formState.errors.email?.message}
              >
                <Input
                  id="email"
                  type="email"
                  {...editForm.register("email")}
                  placeholder="user@example.com"
                />
              </FormField>
            </div>
          </ModalContent>
          <ModalFooter>
            <Button type="button" variant="ghost" onClick={() => setIsEditModalOpen(false)}>
              Cancel
            </Button>
            <Button type="submit" isLoading={updateMutation.isPending}>
              Save
            </Button>
          </ModalFooter>
        </form>
      </Modal>
    </div>
  );
}
