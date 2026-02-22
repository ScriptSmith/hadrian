import { useQuery, useQueryClient } from "@tanstack/react-query";
import { createColumnHelper } from "@tanstack/react-table";
import { Heart, HeartOff, CircleOff, RefreshCw, Server, Activity } from "lucide-react";
import { useMemo, useCallback } from "react";
import { Link } from "react-router-dom";

import {
  listProviderHealthOptions,
  listCircuitBreakersOptions,
  listProviderStatsOptions,
} from "@/api/generated/@tanstack/react-query.gen";
import type {
  ProviderHealthState,
  CircuitBreakerStatus,
  ProviderStats,
} from "@/api/generated/types.gen";
import { Button } from "@/components/Button/Button";
import {
  ResourceTable,
  StatCard,
  StatValue,
  HealthStatusBadge,
  CircuitBreakerBadge,
  ConnectionStatusIndicator,
} from "@/components/Admin";
import { Skeleton } from "@/components/Skeleton/Skeleton";
import { useWebSocketEvents } from "@/hooks";
import { formatDateTime, formatCompactNumber } from "@/utils/formatters";

// Merged type combining health state with optional stats
type MergedProviderData = ProviderHealthState & {
  stats?: ProviderStats;
};

const healthColumnHelper = createColumnHelper<MergedProviderData>();
const circuitColumnHelper = createColumnHelper<CircuitBreakerStatus>();

export default function ProviderHealthPage() {
  const queryClient = useQueryClient();

  const {
    data: healthData,
    isLoading: healthLoading,
    error: healthError,
  } = useQuery(listProviderHealthOptions());

  const {
    data: circuitData,
    isLoading: circuitLoading,
    error: circuitError,
  } = useQuery(listCircuitBreakersOptions());

  const { data: statsData, isLoading: statsLoading } = useQuery(listProviderStatsOptions());

  // Merge health data with stats by provider name
  const mergedProviders = useMemo(() => {
    const providers = healthData?.providers ?? [];
    const statsMap = new Map((statsData?.stats ?? []).map((s) => [s.provider, s]));

    return providers.map((health) => ({
      ...health,
      stats: statsMap.get(health.provider),
    }));
  }, [healthData, statsData]);

  const handleRefresh = useCallback(() => {
    queryClient.invalidateQueries({ queryKey: [{ _id: "listProviderHealth" }] });
    queryClient.invalidateQueries({ queryKey: [{ _id: "listCircuitBreakers" }] });
    queryClient.invalidateQueries({ queryKey: [{ _id: "listProviderStats" }] });
  }, [queryClient]);

  // Subscribe to real-time health events for automatic updates
  const { status: wsStatus } = useWebSocketEvents({
    topics: ["health"],
    onEvent: handleRefresh,
  });

  // Calculate stats
  const stats = useMemo(() => {
    const providers = healthData?.providers ?? [];
    const circuits = circuitData?.circuit_breakers ?? [];

    const healthyCount = providers.filter((p) => p.status === "healthy").length;
    const unhealthyCount = providers.filter((p) => p.status === "unhealthy").length;
    const openCircuits = circuits.filter((c) => c.state === "open").length;

    return {
      total: providers.length,
      healthy: healthyCount,
      unhealthy: unhealthyCount,
      circuitsOpen: openCircuits,
    };
  }, [healthData, circuitData]);

  const healthColumns = useMemo(
    () => [
      // Identity columns
      healthColumnHelper.accessor("provider", {
        header: "Provider",
        cell: (info) => {
          const provider = info.getValue();
          return (
            <Link
              to={`/admin/provider-health/${provider}`}
              className="flex items-center gap-2 hover:text-primary transition-colors text-left"
            >
              <Server className="h-4 w-4 text-muted-foreground" />
              <span className="font-medium">{provider}</span>
            </Link>
          );
        },
      }),
      healthColumnHelper.accessor("status", {
        header: "Status",
        cell: (info) => <HealthStatusBadge status={info.getValue()} />,
      }),
      // Stats columns - latency metrics
      healthColumnHelper.accessor((row) => row.stats?.p50_latency_ms, {
        id: "p50",
        header: "P50",
        cell: (info) => {
          const stats = info.row.original.stats;
          if (statsLoading) return <Skeleton className="h-4 w-12" />;
          if (stats?.p50_latency_ms == null)
            return <span className="text-muted-foreground">-</span>;
          return <span className="tabular-nums">{stats.p50_latency_ms.toFixed(0)}ms</span>;
        },
      }),
      healthColumnHelper.accessor((row) => row.stats?.p95_latency_ms, {
        id: "p95",
        header: "P95",
        cell: (info) => {
          const stats = info.row.original.stats;
          if (statsLoading) return <Skeleton className="h-4 w-12" />;
          if (stats?.p95_latency_ms == null)
            return <span className="text-muted-foreground">-</span>;
          return <span className="tabular-nums">{stats.p95_latency_ms.toFixed(0)}ms</span>;
        },
      }),
      healthColumnHelper.accessor((row) => row.stats?.p99_latency_ms, {
        id: "p99",
        header: "P99",
        cell: (info) => {
          const stats = info.row.original.stats;
          if (statsLoading) return <Skeleton className="h-4 w-12" />;
          if (stats?.p99_latency_ms == null)
            return <span className="text-muted-foreground">-</span>;
          return <span className="tabular-nums">{stats.p99_latency_ms.toFixed(0)}ms</span>;
        },
      }),
      // Stats columns - volume metrics
      healthColumnHelper.accessor((row) => row.stats?.request_count, {
        id: "requests",
        header: "Requests",
        cell: (info) => {
          const stats = info.row.original.stats;
          if (statsLoading) return <Skeleton className="h-4 w-12" />;
          if (!stats) return <span className="text-muted-foreground">-</span>;
          return <span className="tabular-nums">{formatCompactNumber(stats.request_count)}</span>;
        },
      }),
      healthColumnHelper.accessor((row) => row.stats, {
        id: "errorRate",
        header: "Err %",
        cell: (info) => {
          const stats = info.row.original.stats;
          if (statsLoading) return <Skeleton className="h-4 w-12" />;
          if (!stats || stats.request_count === 0)
            return <span className="text-muted-foreground">-</span>;
          const rate = (stats.error_count / stats.request_count) * 100;
          return (
            <span className={`tabular-nums ${rate > 0 ? "text-destructive" : ""}`}>
              {rate.toFixed(1)}%
            </span>
          );
        },
      }),
      healthColumnHelper.accessor((row) => row.stats, {
        id: "tokens",
        header: "Tokens",
        cell: (info) => {
          const stats = info.row.original.stats;
          if (statsLoading) return <Skeleton className="h-4 w-12" />;
          if (!stats) return <span className="text-muted-foreground">-</span>;
          const total = stats.input_tokens + stats.output_tokens;
          return <span className="tabular-nums">{formatCompactNumber(total)}</span>;
        },
      }),
      healthColumnHelper.accessor((row) => row.stats?.total_cost_microcents, {
        id: "cost",
        header: "Cost",
        cell: (info) => {
          const stats = info.row.original.stats;
          if (statsLoading) return <Skeleton className="h-4 w-12" />;
          if (!stats) return <span className="text-muted-foreground">-</span>;
          const dollars = stats.total_cost_microcents / 1_000_000;
          return <span className="tabular-nums">${dollars.toFixed(2)}</span>;
        },
      }),
      // Health check columns
      healthColumnHelper.accessor("latency_ms", {
        header: "Check",
        cell: (info) => {
          const ms = info.getValue();
          return <span className="tabular-nums">{ms}ms</span>;
        },
      }),
      healthColumnHelper.accessor("last_check", {
        header: "Last Check",
        cell: (info) => formatDateTime(info.getValue()),
      }),
      healthColumnHelper.accessor((row) => row, {
        id: "consecutive",
        header: "Consecutive",
        cell: (info) => {
          const row = info.getValue();
          if (row.consecutive_successes > 0) {
            return (
              <span className="text-success tabular-nums">
                {row.consecutive_successes} success{row.consecutive_successes !== 1 ? "es" : ""}
              </span>
            );
          }
          if (row.consecutive_failures > 0) {
            return (
              <span className="text-destructive tabular-nums">
                {row.consecutive_failures} failure{row.consecutive_failures !== 1 ? "s" : ""}
              </span>
            );
          }
          return <span className="text-muted-foreground">-</span>;
        },
      }),
      healthColumnHelper.accessor("error", {
        header: "Error",
        cell: (info) => {
          const error = info.getValue();
          if (!error) return <span className="text-muted-foreground">-</span>;
          return (
            <span className="text-destructive text-sm max-w-xs truncate" title={error}>
              {error}
            </span>
          );
        },
      }),
    ],
    [statsLoading]
  );

  const circuitColumns = useMemo(
    () => [
      circuitColumnHelper.accessor("provider", {
        header: "Provider",
        cell: (info) => (
          <div className="flex items-center gap-2">
            <Server className="h-4 w-4 text-muted-foreground" />
            <span className="font-medium">{info.getValue()}</span>
          </div>
        ),
      }),
      circuitColumnHelper.accessor("state", {
        header: "Circuit State",
        cell: (info) => <CircuitBreakerBadge state={info.getValue()} />,
      }),
      circuitColumnHelper.accessor("failure_count", {
        header: "Failure Count",
        cell: (info) => {
          const count = info.getValue();
          if (count === 0) {
            return <span className="text-muted-foreground tabular-nums">0</span>;
          }
          return <span className="text-destructive tabular-nums">{count}</span>;
        },
      }),
    ],
    []
  );

  const isLoading = healthLoading || circuitLoading;

  return (
    <div className="p-6">
      <div className="mb-6 flex items-center justify-between">
        <div>
          <div className="flex items-center gap-3">
            <h1 className="text-2xl font-semibold">Provider Health</h1>
            <ConnectionStatusIndicator status={wsStatus} />
          </div>
          <p className="text-muted-foreground">
            Monitor provider availability and circuit breaker states
          </p>
        </div>
        <Button variant="outline" onClick={handleRefresh} disabled={isLoading}>
          <RefreshCw className={`mr-2 h-4 w-4 ${isLoading ? "animate-spin" : ""}`} />
          Refresh
        </Button>
      </div>

      {/* Stat cards */}
      <div className="mb-6 grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
        <StatCard
          title="Total Providers"
          icon={<Activity className="h-4 w-4" />}
          isLoading={isLoading}
        >
          <StatValue value={stats.total} />
        </StatCard>
        <StatCard title="Healthy" icon={<Heart className="h-4 w-4" />} isLoading={isLoading}>
          <StatValue value={stats.healthy} className="text-success" />
        </StatCard>
        <StatCard title="Unhealthy" icon={<HeartOff className="h-4 w-4" />} isLoading={isLoading}>
          <StatValue
            value={stats.unhealthy}
            className={stats.unhealthy > 0 ? "text-destructive" : ""}
          />
        </StatCard>
        <StatCard
          title="Circuits Open"
          icon={<CircleOff className="h-4 w-4" />}
          isLoading={isLoading}
        >
          <StatValue
            value={stats.circuitsOpen}
            className={stats.circuitsOpen > 0 ? "text-destructive" : ""}
          />
        </StatCard>
      </div>

      {/* Provider Health Table */}
      <div className="mb-6">
        <ResourceTable
          title="Provider Health Status"
          columns={healthColumns}
          data={mergedProviders}
          isLoading={healthLoading}
          error={healthError}
          emptyMessage="No providers have health checks enabled."
          errorMessage="Failed to load provider health status."
        />
      </div>

      {/* Circuit Breakers Table */}
      <div className="mb-6">
        <ResourceTable
          title="Circuit Breakers"
          columns={circuitColumns}
          data={circuitData?.circuit_breakers ?? []}
          isLoading={circuitLoading}
          error={circuitError}
          emptyMessage="No circuit breakers configured."
          errorMessage="Failed to load circuit breaker status."
        />
      </div>
    </div>
  );
}
