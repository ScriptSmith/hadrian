import { useQuery } from "@tanstack/react-query";
import { createColumnHelper } from "@tanstack/react-table";
import { Download, Filter, X } from "lucide-react";
import { useState, useEffect, useCallback, useMemo } from "react";
import { Link } from "react-router-dom";

import {
  usageLogListOptions,
  meUsageLogListOptions,
  userListOptions,
} from "@/api/generated/@tanstack/react-query.gen";
import { usageLogExport, meUsageLogExport } from "@/api/generated/sdk.gen";
import type { UsageLogResponse } from "@/api/generated/types.gen";
import { Badge } from "@/components/Badge/Badge";
import { Button } from "@/components/Button/Button";
import { useToast } from "@/components/Toast/Toast";
import { CodeBadge } from "@/components/CodeBadge/CodeBadge";
import { Input } from "@/components/Input/Input";
import {
  Modal,
  ModalHeader,
  ModalContent,
  ModalFooter,
  ModalClose,
} from "@/components/Modal/Modal";
import { HighlightedCode } from "@/components/HighlightedCode/HighlightedCode";
import { ResourceTable } from "@/components/Admin";
import type { UsageScope } from "@/components/UsageDashboard/UsageDashboard";
import { useCursorPagination } from "@/hooks";
import { formatCurrency, formatDateTime } from "@/utils/formatters";

const columnHelper = createColumnHelper<UsageLogResponse>();

function FinishReasonBadge({ reason }: { reason: string }) {
  const variant =
    reason === "stop"
      ? "secondary"
      : reason === "error"
        ? "destructive"
        : reason === "content_filter"
          ? "warning"
          : "outline";
  return <Badge variant={variant}>{reason}</Badge>;
}

function DetailRow({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex items-baseline justify-between gap-4 border-b py-2 last:border-0">
      <span className="shrink-0 text-sm text-muted-foreground">{label}</span>
      <span className="text-sm font-medium">{children}</span>
    </div>
  );
}

function LogDetailModal({
  log,
  onClose,
  userMap,
}: {
  log: UsageLogResponse;
  onClose: () => void;
  userMap: Map<string, string>;
}) {
  const [showRawJson, setShowRawJson] = useState(false);
  const rawJson = JSON.stringify(log, null, 2);

  const userName = log.user_id ? userMap.get(log.user_id) : null;

  return (
    <Modal open onClose={onClose} className="max-w-2xl">
      <ModalClose onClose={onClose} />
      <ModalHeader>Request Detail</ModalHeader>
      <ModalContent className="max-h-[70vh] overflow-y-auto">
        <div className="space-y-1">
          <DetailRow label="Timestamp">{formatDateTime(log.recorded_at)}</DetailRow>
          <DetailRow label="Request ID">
            <CodeBadge>{log.request_id}</CodeBadge>
          </DetailRow>
          <DetailRow label="Model">{log.model}</DetailRow>
          <DetailRow label="Provider">
            {log.provider}
            {log.provider_source && (
              <Badge variant="outline" className="ml-2 text-xs">
                {log.provider_source}
              </Badge>
            )}
          </DetailRow>
          <DetailRow label="Input Tokens">{log.input_tokens.toLocaleString()}</DetailRow>
          <DetailRow label="Output Tokens">{log.output_tokens.toLocaleString()}</DetailRow>
          {log.cached_tokens > 0 && (
            <DetailRow label="Cached Tokens">{log.cached_tokens.toLocaleString()}</DetailRow>
          )}
          {log.reasoning_tokens > 0 && (
            <DetailRow label="Reasoning Tokens">{log.reasoning_tokens.toLocaleString()}</DetailRow>
          )}
          <DetailRow label="Cost">{formatCurrency(log.cost)}</DetailRow>
          <DetailRow label="Latency">
            {log.latency_ms != null ? `${log.latency_ms.toLocaleString()}ms` : "-"}
          </DetailRow>
          <DetailRow label="Finish Reason">
            {log.finish_reason ? <FinishReasonBadge reason={log.finish_reason} /> : "-"}
          </DetailRow>
          <DetailRow label="Streamed">
            <Badge variant={log.streamed ? "secondary" : "outline"}>
              {log.streamed ? "Yes" : "No"}
            </Badge>
          </DetailRow>
          <DetailRow label="Status Code">
            {log.status_code != null ? (
              <Badge variant={log.status_code >= 400 ? "destructive" : "secondary"}>
                {log.status_code}
              </Badge>
            ) : (
              "-"
            )}
          </DetailRow>
          <DetailRow label="Pricing Source">{log.pricing_source}</DetailRow>
          {log.user_id && (
            <DetailRow label="User">
              {userName ? (
                <Link to={`/admin/users/${log.user_id}`} className="text-primary hover:underline">
                  {userName}
                </Link>
              ) : (
                <CodeBadge>{log.user_id}</CodeBadge>
              )}
            </DetailRow>
          )}
          {log.api_key_id && (
            <DetailRow label="API Key ID">
              <CodeBadge>{log.api_key_id}</CodeBadge>
            </DetailRow>
          )}
          {log.org_id && (
            <DetailRow label="Org ID">
              <CodeBadge>{log.org_id}</CodeBadge>
            </DetailRow>
          )}
          {log.project_id && (
            <DetailRow label="Project ID">
              <CodeBadge>{log.project_id}</CodeBadge>
            </DetailRow>
          )}
          {log.team_id && (
            <DetailRow label="Team ID">
              <CodeBadge>{log.team_id}</CodeBadge>
            </DetailRow>
          )}
          {log.service_account_id && (
            <DetailRow label="Service Account ID">
              <CodeBadge>{log.service_account_id}</CodeBadge>
            </DetailRow>
          )}
        </div>

        <div className="mt-4">
          <Button
            variant="outline"
            size="sm"
            onClick={() => setShowRawJson(!showRawJson)}
            className="mb-2"
          >
            {showRawJson ? "Hide" : "Show"} Raw JSON
          </Button>
          {showRawJson && (
            <HighlightedCode code={rawJson} language="json" maxHeight="16rem" showCopy />
          )}
        </div>
      </ModalContent>
      <ModalFooter>
        <Button variant="outline" onClick={onClose}>
          Close
        </Button>
      </ModalFooter>
    </Modal>
  );
}

interface UsageLogsTableProps {
  scope: UsageScope;
}

function scopeToQueryFilters(scope: UsageScope) {
  switch (scope.type) {
    case "me":
    case "global":
      return {};
    case "organization":
      return { org_id: scope.orgId };
    case "user":
      return { user_id: scope.userId };
    case "project":
      return { project_id: scope.projectId };
    case "team":
      return { team_id: scope.teamId };
    case "apiKey":
      return { api_key_id: scope.keyId };
    default:
      return {};
  }
}

/** Clickable cell that adds a value to a filter */
function FilterableCell({
  value,
  onFilter,
  children,
}: {
  value: string;
  onFilter: (value: string) => void;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      onClick={(e) => {
        e.stopPropagation();
        onFilter(value);
      }}
      className="cursor-pointer text-left hover:underline"
      title={`Filter by "${value}"`}
    >
      {children}
    </button>
  );
}

export default function UsageLogsTable({ scope }: UsageLogsTableProps) {
  const toast = useToast();
  const [filters, setFilters] = useState({
    model: "",
    provider: "",
    provider_source: "" as "" | "static" | "dynamic",
    from: "",
    to: "",
  });
  const [showFilters, setShowFilters] = useState(false);
  const [selectedLog, setSelectedLog] = useState<UsageLogResponse | null>(null);

  const pagination = useCursorPagination({ defaultLimit: 50 });

  const scopeFilters = scopeToQueryFilters(scope);
  const queryFilters = {
    model: filters.model || undefined,
    provider: filters.provider || undefined,
    provider_source: filters.provider_source || undefined,
    from: filters.from ? new Date(filters.from).toISOString() : undefined,
    to: filters.to ? new Date(filters.to).toISOString() : undefined,
    ...scopeFilters,
    ...pagination.queryParams,
  };

  const isMe = scope.type === "me";

  const { data, isLoading, error } = useQuery({
    ...(isMe
      ? meUsageLogListOptions({ query: queryFilters })
      : usageLogListOptions({ query: queryFilters })),
  });

  // Fetch users for name resolution (only for non-"me" scopes)
  // TODO: This fetches all users client-side and doesn't scale. Replace with
  // server-side user name resolution in the usage log response.
  const { data: usersData } = useQuery({
    ...userListOptions(),
    enabled: !isMe,
  });

  const userMap = useMemo(() => {
    const map = new Map<string, string>();
    if (usersData?.data) {
      for (const user of usersData.data) {
        const display = user.name || user.email || user.external_id;
        map.set(user.id, display);
      }
    }
    return map;
  }, [usersData]);

  const hasActiveFilters =
    filters.model || filters.provider || filters.provider_source || filters.from || filters.to;

  const clearFilters = () => {
    setFilters({ model: "", provider: "", provider_source: "", from: "", to: "" });
    pagination.actions.goToFirstPage();
  };

  const addModelFilter = useCallback((model: string) => {
    setFilters((f) => ({ ...f, model }));
    setShowFilters(true);
  }, []);

  const addProviderFilter = useCallback((provider: string) => {
    setFilters((f) => ({ ...f, provider }));
    setShowFilters(true);
  }, []);

  useEffect(() => {
    pagination.actions.goToFirstPage();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [filters.model, filters.provider, filters.provider_source, filters.from, filters.to]);

  const handleExport = useCallback(
    async (format: "csv" | "jsonl") => {
      try {
        const exportFilters = {
          model: filters.model || undefined,
          provider: filters.provider || undefined,
          provider_source: filters.provider_source || undefined,
          from: filters.from ? new Date(filters.from).toISOString() : undefined,
          to: filters.to ? new Date(filters.to).toISOString() : undefined,
          format,
          ...scopeFilters,
        };

        const exportFn = isMe ? meUsageLogExport : usageLogExport;
        const { data: blob } = await exportFn({
          query: exportFilters,
          responseType: "blob",
        } as Parameters<typeof exportFn>[0]);

        if (blob instanceof Blob) {
          const url = URL.createObjectURL(blob);
          const a = document.createElement("a");
          a.href = url;
          a.download = `usage-logs.${format}`;
          a.click();
          URL.revokeObjectURL(url);
        }
      } catch {
        toast.error(`Failed to export usage logs as ${format.toUpperCase()}`);
      }
    },
    [filters, scopeFilters, isMe, toast]
  );

  const columns = useMemo(
    () => [
      columnHelper.accessor("recorded_at", {
        header: "Timestamp",
        cell: (info) => (
          <span className="whitespace-nowrap text-sm text-muted-foreground">
            {formatDateTime(info.getValue())}
          </span>
        ),
      }),
      columnHelper.accessor("model", {
        header: "Model",
        cell: (info) => (
          <FilterableCell value={info.getValue()} onFilter={addModelFilter}>
            <span className="text-sm font-medium">{info.getValue()}</span>
          </FilterableCell>
        ),
      }),
      columnHelper.accessor("provider", {
        header: "Provider",
        cell: (info) => (
          <div className="flex items-center gap-1.5">
            <FilterableCell value={info.getValue()} onFilter={addProviderFilter}>
              <span className="text-sm">{info.getValue()}</span>
            </FilterableCell>
            {info.row.original.provider_source && (
              <Badge variant="outline" className="text-xs">
                {info.row.original.provider_source}
              </Badge>
            )}
          </div>
        ),
      }),
      columnHelper.accessor("input_tokens", {
        header: "In",
        cell: (info) => (
          <span className="text-sm tabular-nums">{info.getValue().toLocaleString()}</span>
        ),
      }),
      columnHelper.accessor("output_tokens", {
        header: "Out",
        cell: (info) => (
          <span className="text-sm tabular-nums">{info.getValue().toLocaleString()}</span>
        ),
      }),
      columnHelper.accessor("cached_tokens", {
        header: "Cached",
        cell: (info) => {
          const val = info.getValue();
          return val ? (
            <span className="text-sm tabular-nums">{val.toLocaleString()}</span>
          ) : (
            <span className="text-muted-foreground">-</span>
          );
        },
      }),
      columnHelper.accessor("reasoning_tokens", {
        header: "Reasoning",
        cell: (info) => {
          const val = info.getValue();
          return val ? (
            <span className="text-sm tabular-nums">{val.toLocaleString()}</span>
          ) : (
            <span className="text-muted-foreground">-</span>
          );
        },
      }),
      columnHelper.accessor("cost", {
        header: "Cost",
        cell: (info) => (
          <span className="text-sm font-medium tabular-nums">
            {formatCurrency(info.getValue())}
          </span>
        ),
      }),
      columnHelper.accessor("latency_ms", {
        header: "Latency",
        cell: (info) => {
          const ms = info.getValue();
          return ms != null ? (
            <span className="text-sm tabular-nums">{ms.toLocaleString()}ms</span>
          ) : (
            <span className="text-muted-foreground">-</span>
          );
        },
      }),
      columnHelper.accessor("finish_reason", {
        header: "Finish",
        cell: (info) => {
          const reason = info.getValue();
          return reason ? (
            <FinishReasonBadge reason={reason} />
          ) : (
            <span className="text-muted-foreground">-</span>
          );
        },
      }),
      columnHelper.accessor("streamed", {
        header: "Stream",
        cell: (info) => (
          <Badge variant={info.getValue() ? "secondary" : "outline"}>
            {info.getValue() ? "Yes" : "No"}
          </Badge>
        ),
      }),
      columnHelper.accessor("status_code", {
        header: "Status",
        cell: (info) => {
          const code = info.getValue();
          if (code == null) return <span className="text-muted-foreground">-</span>;
          const variant = code >= 400 ? "destructive" : "secondary";
          return <Badge variant={variant}>{code}</Badge>;
        },
      }),
      columnHelper.accessor("user_id", {
        header: "User",
        cell: (info) => {
          const userId = info.getValue();
          if (!userId) return <span className="text-muted-foreground">-</span>;
          const name = userMap.get(userId);
          if (name) {
            return (
              <Link
                to={`/admin/users/${userId}`}
                onClick={(e) => e.stopPropagation()}
                className="text-sm text-primary hover:underline"
                title={userId}
              >
                {name}
              </Link>
            );
          }
          return (
            <span title={userId}>
              <CodeBadge className="text-xs">{userId.slice(0, 8)}...</CodeBadge>
            </span>
          );
        },
      }),
    ],
    [userMap, addModelFilter, addProviderFilter]
  );

  return (
    <div>
      {/* Filter bar */}
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
              {
                [
                  filters.model,
                  filters.provider,
                  filters.provider_source,
                  filters.from,
                  filters.to,
                ].filter(Boolean).length
              }
            </Badge>
          )}
        </Button>
        {hasActiveFilters && (
          <Button variant="ghost" size="sm" onClick={clearFilters} className="gap-1">
            <X className="h-4 w-4" />
            Clear
          </Button>
        )}
        <div className="ml-auto flex gap-2">
          <Button variant="outline" size="sm" onClick={() => handleExport("csv")} className="gap-2">
            <Download className="h-4 w-4" />
            CSV
          </Button>
          <Button
            variant="outline"
            size="sm"
            onClick={() => handleExport("jsonl")}
            className="gap-2"
          >
            <Download className="h-4 w-4" />
            JSONL
          </Button>
        </div>
      </div>

      {showFilters && (
        <div className="mb-4 flex flex-wrap gap-4 rounded-lg border bg-card p-4">
          <div className="flex flex-col gap-1">
            <label htmlFor="filter-model" className="text-xs font-medium text-muted-foreground">
              Model
            </label>
            <Input
              id="filter-model"
              placeholder="e.g., gpt-4o"
              value={filters.model}
              onChange={(e) => setFilters({ ...filters, model: e.target.value })}
              className="h-9 w-48"
            />
          </div>
          <div className="flex flex-col gap-1">
            <label htmlFor="filter-provider" className="text-xs font-medium text-muted-foreground">
              Provider
            </label>
            <Input
              id="filter-provider"
              placeholder="e.g., openai"
              value={filters.provider}
              onChange={(e) => setFilters({ ...filters, provider: e.target.value })}
              className="h-9 w-48"
            />
          </div>
          <div className="flex flex-col gap-1">
            <label
              htmlFor="filter-provider-source"
              className="text-xs font-medium text-muted-foreground"
            >
              Provider Source
            </label>
            <select
              id="filter-provider-source"
              value={filters.provider_source}
              onChange={(e) =>
                setFilters({
                  ...filters,
                  provider_source: e.target.value as "" | "static" | "dynamic",
                })
              }
              className="h-9 w-36 rounded-md border border-input bg-background px-3 text-sm"
            >
              <option value="">All</option>
              <option value="static">Static</option>
              <option value="dynamic">Dynamic</option>
            </select>
          </div>
          <div className="flex flex-col gap-1">
            <label htmlFor="filter-from" className="text-xs font-medium text-muted-foreground">
              From
            </label>
            <Input
              id="filter-from"
              type="datetime-local"
              value={filters.from}
              onChange={(e) => setFilters({ ...filters, from: e.target.value })}
              className="h-9 w-52"
            />
          </div>
          <div className="flex flex-col gap-1">
            <label htmlFor="filter-to" className="text-xs font-medium text-muted-foreground">
              To
            </label>
            <Input
              id="filter-to"
              type="datetime-local"
              value={filters.to}
              onChange={(e) => setFilters({ ...filters, to: e.target.value })}
              className="h-9 w-52"
            />
          </div>
        </div>
      )}

      <ResourceTable
        title="Usage Logs"
        columns={columns}
        data={data?.data || []}
        isLoading={isLoading}
        error={error}
        emptyMessage="No usage logs found."
        errorMessage="Failed to load usage logs. Please try again."
        onRowClick={setSelectedLog}
        paginationProps={{
          pagination: data?.pagination,
          isFirstPage: pagination.info.isFirstPage,
          pageNumber: pagination.info.pageNumber,
          onPrevious: () =>
            data?.pagination && pagination.actions.goToPreviousPage(data.pagination),
          onNext: () => data?.pagination && pagination.actions.goToNextPage(data.pagination),
          onFirst: () => pagination.actions.goToFirstPage(),
        }}
      />

      {selectedLog && (
        <LogDetailModal log={selectedLog} onClose={() => setSelectedLog(null)} userMap={userMap} />
      )}
    </div>
  );
}
