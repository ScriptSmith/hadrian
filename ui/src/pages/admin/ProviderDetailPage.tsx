import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Link, useParams } from "react-router-dom";
import { ArrowLeft, Activity, AlertTriangle, Clock, Zap, DollarSign, Hash } from "lucide-react";

import {
  getProviderHealthOptions,
  getCircuitBreakerOptions,
  getProviderStatsOptions,
} from "@/api/generated/@tanstack/react-query.gen";
import type { CircuitState } from "@/api/generated/types.gen";
import {
  StatCard,
  StatValue,
  HealthStatusBadge,
  CircuitBreakerBadge,
  ConnectionStatusIndicator,
  ProviderHistoryCharts,
} from "@/components/Admin";
import { Skeleton } from "@/components/Skeleton/Skeleton";
import { useWebSocketEvents } from "@/hooks";
import { formatDateTime, formatCompactNumber } from "@/utils/formatters";

export default function ProviderDetailPage() {
  const { providerName } = useParams<{ providerName: string }>();
  const queryClient = useQueryClient();

  const {
    data: healthData,
    isLoading: healthLoading,
    error: healthError,
  } = useQuery({
    ...getProviderHealthOptions({ path: { provider_name: providerName! } }),
    enabled: !!providerName,
  });

  const {
    data: circuitData,
    isLoading: circuitLoading,
    error: circuitError,
  } = useQuery({
    ...getCircuitBreakerOptions({ path: { provider_name: providerName! } }),
    enabled: !!providerName,
  });

  const { data: statsData, isLoading: statsLoading } = useQuery({
    ...getProviderStatsOptions({ path: { provider_name: providerName! } }),
    enabled: !!providerName,
  });

  // Subscribe to real-time health events for automatic updates
  const { status: wsStatus } = useWebSocketEvents({
    topics: ["health"],
    onEvent: () => {
      if (providerName) {
        queryClient.invalidateQueries({ queryKey: [{ _id: "getProviderHealth" }] });
        queryClient.invalidateQueries({ queryKey: [{ _id: "getCircuitBreaker" }] });
        queryClient.invalidateQueries({ queryKey: [{ _id: "getProviderStats" }] });
      }
    },
  });

  const isLoading = healthLoading || circuitLoading;

  // Calculate error rate
  const errorRate =
    statsData && statsData.request_count > 0
      ? (statsData.error_count / statsData.request_count) * 100
      : 0;

  // Calculate total tokens
  const totalTokens = statsData ? statsData.input_tokens + statsData.output_tokens : 0;

  // Calculate cost in dollars
  const costDollars = statsData ? statsData.total_cost_microcents / 1_000_000 : 0;

  // Show error state with back link
  if (healthError || circuitError) {
    return (
      <div className="p-6">
        <Link
          to="/admin/provider-health"
          className="mb-4 inline-flex items-center gap-2 text-sm text-muted-foreground hover:text-foreground"
        >
          <ArrowLeft className="h-4 w-4" />
          Back to Providers
        </Link>
        <div className="rounded-lg border border-destructive/50 bg-destructive/10 p-6">
          <h2 className="text-lg font-medium text-destructive">Error Loading Provider Data</h2>
          <p className="mt-2 text-sm text-muted-foreground">
            Failed to load data for provider &quot;{providerName}&quot;. The provider may not exist
            or health checks may not be enabled.
          </p>
        </div>
      </div>
    );
  }

  return (
    <div className="p-6">
      {/* Header */}
      <div className="mb-6">
        <Link
          to="/admin/provider-health"
          className="mb-4 inline-flex items-center gap-2 text-sm text-muted-foreground hover:text-foreground"
        >
          <ArrowLeft className="h-4 w-4" />
          Back to Providers
        </Link>

        <div className="flex items-center justify-between">
          <div>
            <div className="flex items-center gap-3">
              <h1 className="text-2xl font-semibold">{providerName}</h1>
              {isLoading ? (
                <Skeleton className="h-6 w-20" />
              ) : (
                <>
                  {healthData && <HealthStatusBadge status={healthData.status} />}
                  {circuitData && <CircuitBreakerBadge state={circuitData.state as CircuitState} />}
                </>
              )}
              <ConnectionStatusIndicator status={wsStatus} />
            </div>
            {healthData && (
              <p className="mt-1 text-sm text-muted-foreground">
                Last check: {formatDateTime(healthData.last_check)}
              </p>
            )}
          </div>
        </div>
      </div>

      {/* Summary Stats Cards - Row 1: Performance metrics */}
      <div className="mb-4 grid gap-4 sm:grid-cols-2 lg:grid-cols-5">
        <StatCard title="Requests" icon={<Activity className="h-4 w-4" />} isLoading={statsLoading}>
          {statsData ? (
            <StatValue value={formatCompactNumber(statsData.request_count)} />
          ) : (
            <StatValue value="-" className="text-muted-foreground" />
          )}
        </StatCard>

        <StatCard
          title="Error Rate"
          icon={<AlertTriangle className="h-4 w-4" />}
          isLoading={statsLoading}
        >
          {statsData ? (
            <StatValue
              value={`${errorRate.toFixed(1)}%`}
              className={errorRate > 0 ? "text-destructive" : ""}
            />
          ) : (
            <StatValue value="-" className="text-muted-foreground" />
          )}
        </StatCard>

        <StatCard title="P50 Latency" icon={<Clock className="h-4 w-4" />} isLoading={statsLoading}>
          {statsData?.p50_latency_ms != null ? (
            <StatValue value={`${statsData.p50_latency_ms.toFixed(0)}ms`} />
          ) : (
            <StatValue value="-" className="text-muted-foreground" />
          )}
        </StatCard>

        <StatCard title="P95 Latency" icon={<Clock className="h-4 w-4" />} isLoading={statsLoading}>
          {statsData?.p95_latency_ms != null ? (
            <StatValue value={`${statsData.p95_latency_ms.toFixed(0)}ms`} />
          ) : (
            <StatValue value="-" className="text-muted-foreground" />
          )}
        </StatCard>

        <StatCard title="P99 Latency" icon={<Clock className="h-4 w-4" />} isLoading={statsLoading}>
          {statsData?.p99_latency_ms != null ? (
            <StatValue value={`${statsData.p99_latency_ms.toFixed(0)}ms`} />
          ) : (
            <StatValue value="-" className="text-muted-foreground" />
          )}
        </StatCard>
      </div>

      {/* Summary Stats Cards - Row 2: Volume metrics */}
      <div className="mb-6 grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
        <StatCard title="Total Tokens" icon={<Hash className="h-4 w-4" />} isLoading={statsLoading}>
          {statsData ? (
            <StatValue value={formatCompactNumber(totalTokens)} />
          ) : (
            <StatValue value="-" className="text-muted-foreground" />
          )}
        </StatCard>

        <StatCard
          title="Total Cost"
          icon={<DollarSign className="h-4 w-4" />}
          isLoading={statsLoading}
        >
          {statsData ? (
            <StatValue value={`$${costDollars.toFixed(2)}`} />
          ) : (
            <StatValue value="-" className="text-muted-foreground" />
          )}
        </StatCard>

        <StatCard
          title="Health Check Latency"
          icon={<Zap className="h-4 w-4" />}
          isLoading={healthLoading}
        >
          {healthData ? (
            <StatValue value={`${healthData.latency_ms}ms`} />
          ) : (
            <StatValue value="-" className="text-muted-foreground" />
          )}
        </StatCard>
      </div>

      {/* Historical Charts */}
      {providerName && <ProviderHistoryCharts provider={providerName} />}
    </div>
  );
}
