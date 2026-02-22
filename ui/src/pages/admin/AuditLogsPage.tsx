import { useQuery } from "@tanstack/react-query";
import { createColumnHelper } from "@tanstack/react-table";
import { User, Key, Server, Filter, X } from "lucide-react";
import { useState, useEffect } from "react";

import { auditLogListOptions } from "@/api/generated/@tanstack/react-query.gen";
import type { AuditLog, AuditActorType } from "@/api/generated/types.gen";
import { Badge } from "@/components/Badge/Badge";
import { Button } from "@/components/Button/Button";
import { CodeBadge } from "@/components/CodeBadge/CodeBadge";
import { Input } from "@/components/Input/Input";
import { PageHeader, ResourceTable } from "@/components/Admin";
import { useCursorPagination } from "@/hooks";
import { formatDateTime } from "@/utils/formatters";

// Helper to safely access details properties
function getDetailValue(details: unknown, key: string): string | null {
  if (typeof details === "object" && details !== null && key in details) {
    const value = (details as Record<string, unknown>)[key];
    return typeof value === "string" ? value : null;
  }
  return null;
}

const columnHelper = createColumnHelper<AuditLog>();

function ActorTypeBadge({ type }: { type: AuditActorType }) {
  switch (type) {
    case "user":
      return (
        <Badge variant="secondary" className="gap-1">
          <User className="h-3 w-3" />
          User
        </Badge>
      );
    case "api_key":
      return (
        <Badge variant="outline" className="gap-1">
          <Key className="h-3 w-3" />
          API Key
        </Badge>
      );
    case "system":
      return (
        <Badge variant="default" className="gap-1">
          <Server className="h-3 w-3" />
          System
        </Badge>
      );
    default:
      return <Badge variant="outline">{type}</Badge>;
  }
}

function ActionBadge({ action }: { action: string }) {
  const [resource, verb] = action.split(".");
  const variant = verb === "delete" || verb === "revoke" ? "destructive" : "secondary";

  return (
    <Badge variant={variant}>
      {resource}.{verb}
    </Badge>
  );
}

export default function AuditLogsPage() {
  const [filters, setFilters] = useState({
    action: "",
    resource_type: "",
    actor_type: "" as AuditActorType | "",
  });
  const [showFilters, setShowFilters] = useState(false);

  const pagination = useCursorPagination({ defaultLimit: 50 });

  const {
    data: auditLogs,
    isLoading,
    error,
  } = useQuery({
    ...auditLogListOptions({
      query: {
        action: filters.action || undefined,
        resource_type: filters.resource_type || undefined,
        actor_type: filters.actor_type || undefined,
        ...pagination.queryParams,
      },
    }),
  });

  const hasActiveFilters = filters.action || filters.resource_type || filters.actor_type;

  const clearFilters = () => {
    setFilters({ action: "", resource_type: "", actor_type: "" });
    pagination.actions.goToFirstPage();
  };

  // Reset pagination when filters change
  useEffect(() => {
    pagination.actions.goToFirstPage();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [filters.action, filters.resource_type, filters.actor_type]);

  const columns = [
    columnHelper.accessor("timestamp", {
      header: "Time",
      cell: (info) => (
        <span className="text-sm text-muted-foreground whitespace-nowrap">
          {formatDateTime(info.getValue())}
        </span>
      ),
    }),
    columnHelper.accessor("action", {
      header: "Action",
      cell: (info) => <ActionBadge action={info.getValue()} />,
    }),
    columnHelper.accessor("actor_type", {
      header: "Actor",
      cell: (info) => (
        <div className="flex flex-col gap-1">
          <ActorTypeBadge type={info.getValue()} />
          {info.row.original.actor_id && (
            <CodeBadge className="text-xs">{info.row.original.actor_id.slice(0, 8)}...</CodeBadge>
          )}
        </div>
      ),
    }),
    columnHelper.accessor("resource_type", {
      header: "Resource",
      cell: (info) => (
        <div className="flex flex-col gap-1">
          <span className="font-medium">{info.getValue()}</span>
          <CodeBadge className="text-xs">{info.row.original.resource_id.slice(0, 8)}...</CodeBadge>
        </div>
      ),
    }),
    columnHelper.accessor("details", {
      header: "Details",
      cell: (info) => {
        const details = info.getValue();
        if (!details || typeof details !== "object" || Object.keys(details).length === 0) {
          return <span className="text-muted-foreground">-</span>;
        }

        // Show key details like name, slug, etc.
        const name = getDetailValue(details, "name") || getDetailValue(details, "slug");
        if (name) {
          return <span className="text-sm">{name}</span>;
        }

        return (
          <span className="text-sm text-muted-foreground">
            {Object.keys(details).length} field(s)
          </span>
        );
      },
    }),
    columnHelper.accessor("org_id", {
      header: "Context",
      cell: (info) => {
        const orgId = info.getValue();
        const projectId = info.row.original.project_id;

        if (!orgId && !projectId) {
          return <span className="text-muted-foreground">-</span>;
        }

        return (
          <div className="flex flex-col gap-1 text-xs">
            {orgId && <span className="text-muted-foreground">Org: {orgId.slice(0, 8)}...</span>}
            {projectId && (
              <span className="text-muted-foreground">Proj: {projectId.slice(0, 8)}...</span>
            )}
          </div>
        );
      },
    }),
  ];

  return (
    <div className="p-6">
      <PageHeader title="Audit Logs" description="View audit trail of administrative actions" />

      {/* Filter controls */}
      <div className="mb-4 flex items-center gap-2">
        <Button
          variant={showFilters ? "secondary" : "outline"}
          size="sm"
          onClick={() => setShowFilters(!showFilters)}
          className="gap-2"
        >
          <Filter className="h-4 w-4" />
          Filters
          {hasActiveFilters && (
            <Badge variant="default" className="ml-1 h-5 min-w-5 rounded-full px-1.5">
              {[filters.action, filters.resource_type, filters.actor_type].filter(Boolean).length}
            </Badge>
          )}
        </Button>
        {hasActiveFilters && (
          <Button variant="ghost" size="sm" onClick={clearFilters} className="gap-1">
            <X className="h-4 w-4" />
            Clear
          </Button>
        )}
      </div>

      {showFilters && (
        <div className="mb-4 flex flex-wrap gap-4 rounded-lg border bg-card p-4">
          <div className="flex flex-col gap-1">
            <label htmlFor="filter-action" className="text-xs font-medium text-muted-foreground">
              Action
            </label>
            <Input
              id="filter-action"
              placeholder="e.g., api_key.create"
              value={filters.action}
              onChange={(e) => setFilters({ ...filters, action: e.target.value })}
              className="h-9 w-48"
            />
          </div>
          <div className="flex flex-col gap-1">
            <label
              htmlFor="filter-resource-type"
              className="text-xs font-medium text-muted-foreground"
            >
              Resource Type
            </label>
            <Input
              id="filter-resource-type"
              placeholder="e.g., api_key"
              value={filters.resource_type}
              onChange={(e) => setFilters({ ...filters, resource_type: e.target.value })}
              className="h-9 w-48"
            />
          </div>
          <div className="flex flex-col gap-1">
            <label
              htmlFor="filter-actor-type"
              className="text-xs font-medium text-muted-foreground"
            >
              Actor Type
            </label>
            <select
              id="filter-actor-type"
              value={filters.actor_type}
              onChange={(e) =>
                setFilters({ ...filters, actor_type: e.target.value as AuditActorType | "" })
              }
              className="h-9 w-36 rounded-md border border-input bg-background px-3 text-sm"
            >
              <option value="">All</option>
              <option value="user">User</option>
              <option value="api_key">API Key</option>
              <option value="system">System</option>
            </select>
          </div>
        </div>
      )}

      <ResourceTable
        title="Recent Activity"
        columns={columns}
        data={auditLogs?.data || []}
        isLoading={isLoading}
        error={error}
        emptyMessage="No audit logs found."
        errorMessage="Failed to load audit logs. Please try again."
        paginationProps={{
          pagination: auditLogs?.pagination,
          isFirstPage: pagination.info.isFirstPage,
          pageNumber: pagination.info.pageNumber,
          onPrevious: () => pagination.actions.goToPreviousPage(auditLogs!.pagination),
          onNext: () => pagination.actions.goToNextPage(auditLogs!.pagination),
          onFirst: () => pagination.actions.goToFirstPage(),
        }}
      />
    </div>
  );
}
