import { useState, useMemo, useCallback } from "react";
import { useQuery } from "@tanstack/react-query";
import { X } from "lucide-react";

import { getProviderStatsHistoryOptions } from "@/api/generated/@tanstack/react-query.gen";
import { MultiLineChart, LineChart, CHART_COLORS } from "@/components/Charts";
import { Skeleton } from "@/components/Skeleton/Skeleton";
import { Button } from "@/components/Button/Button";
import { TimeRangeSelector, getTimeRangeFromPreset, type TimeRange } from "../TimeRangeSelector";

export interface ProviderHistoryChartsProps {
  /** Provider name to show history for */
  provider: string;
  /** Callback when the close button is clicked */
  onClose?: () => void;
}

interface ChartDataPoint {
  time: string;
  p50: number | null;
  p95: number | null;
  p99: number | null;
  errorRate: number;
  requests: number;
  [key: string]: string | number | null;
}

const formatMs = (value: number) => `${value.toFixed(0)}ms`;
const formatPercent = (value: number) => `${value.toFixed(1)}%`;
const formatNumber = (value: number) => {
  if (value >= 1000000) return `${(value / 1000000).toFixed(1)}M`;
  if (value >= 1000) return `${(value / 1000).toFixed(1)}K`;
  return value.toString();
};

function formatTimeLabel(isoString: string, granularity: "hour" | "day"): string {
  const date = new Date(isoString);
  if (granularity === "hour") {
    return date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  }
  return date.toLocaleDateString([], { month: "short", day: "numeric" });
}

export function ProviderHistoryCharts({ provider, onClose }: ProviderHistoryChartsProps) {
  const [selectedPreset, setSelectedPreset] = useState("24h");
  const [timeRange, setTimeRange] = useState<TimeRange>(() => getTimeRangeFromPreset("24h"));

  const handleTimeRangeChange = useCallback((range: TimeRange, preset: string) => {
    setTimeRange(range);
    setSelectedPreset(preset);
  }, []);

  const { data: historyData, isLoading } = useQuery({
    ...getProviderStatsHistoryOptions({
      path: { provider_name: provider },
      query: {
        start: timeRange.start,
        end: timeRange.end,
        granularity: timeRange.granularity,
      },
    }),
    refetchInterval: 60000, // Refresh every minute
  });

  // Transform API response to chart data
  const chartData = useMemo<ChartDataPoint[]>(() => {
    if (!historyData?.data) return [];

    return historyData.data.map((bucket) => ({
      time: bucket.bucket_start,
      p50: bucket.p50_latency_ms ?? null,
      p95: bucket.p95_latency_ms ?? null,
      p99: bucket.p99_latency_ms ?? null,
      errorRate: bucket.request_count > 0 ? (bucket.error_count / bucket.request_count) * 100 : 0,
      requests: bucket.request_count,
    }));
  }, [historyData]);

  // Check if we have any latency data
  const hasLatencyData = useMemo(() => {
    return chartData.some((d) => d.p50 !== null || d.p95 !== null || d.p99 !== null);
  }, [chartData]);

  // Check if we have any request data
  const hasRequestData = useMemo(() => {
    return chartData.some((d) => d.requests > 0);
  }, [chartData]);

  const xFormatter = useCallback(
    (value: string) => formatTimeLabel(value, timeRange.granularity),
    [timeRange.granularity]
  );

  return (
    <div className="rounded-lg border bg-card p-4">
      <div className="mb-4 flex items-center justify-between">
        <h3 className="text-lg font-medium">
          Historical Trends for <span className="text-primary">{provider}</span>
        </h3>
        <div className="flex items-center gap-2">
          <TimeRangeSelector value={selectedPreset} onChange={handleTimeRangeChange} />
          {onClose && (
            <Button variant="ghost" size="icon" onClick={onClose} aria-label="Close charts">
              <X className="h-4 w-4" />
            </Button>
          )}
        </div>
      </div>

      {isLoading ? (
        <div className="space-y-6">
          <ChartSkeleton title="Latency (ms)" />
          <div className="grid gap-6 md:grid-cols-2">
            <ChartSkeleton title="Error Rate (%)" />
            <ChartSkeleton title="Request Volume" />
          </div>
        </div>
      ) : chartData.length === 0 ? (
        <div className="flex h-48 flex-col items-center justify-center gap-2 text-muted-foreground">
          {historyData?.prometheus_configured === false ? (
            <>
              <p className="font-medium">Historical metrics require Prometheus</p>
              <p className="text-sm">
                Configure{" "}
                <code className="rounded bg-muted px-1 py-0.5">
                  observability.metrics.prometheus_query_url
                </code>{" "}
                to enable.
              </p>
            </>
          ) : (
            <p>No data available for the selected time range</p>
          )}
        </div>
      ) : (
        <div className="space-y-6">
          {/* Latency Chart */}
          <div>
            <h4 className="mb-2 text-sm font-medium text-muted-foreground">Latency (ms)</h4>
            {hasLatencyData ? (
              <MultiLineChart
                data={chartData}
                xKey="time"
                series={[
                  { dataKey: "p50", name: "P50", color: CHART_COLORS[0] },
                  { dataKey: "p95", name: "P95", color: CHART_COLORS[1] },
                  { dataKey: "p99", name: "P99", color: CHART_COLORS[2] },
                ]}
                height={200}
                formatter={formatMs}
                xFormatter={xFormatter}
              />
            ) : (
              <div className="flex h-[200px] items-center justify-center text-muted-foreground">
                No latency data available
              </div>
            )}
          </div>

          {/* Error Rate and Request Volume */}
          <div className="grid gap-6 md:grid-cols-2">
            <div>
              <h4 className="mb-2 text-sm font-medium text-muted-foreground">Error Rate (%)</h4>
              {hasRequestData ? (
                <LineChart
                  data={chartData}
                  xKey="time"
                  yKey="errorRate"
                  height={180}
                  formatter={formatPercent}
                  xFormatter={xFormatter}
                  color="#ef4444"
                  showArea
                />
              ) : (
                <div className="flex h-[180px] items-center justify-center text-muted-foreground">
                  No error data available
                </div>
              )}
            </div>

            <div>
              <h4 className="mb-2 text-sm font-medium text-muted-foreground">Request Volume</h4>
              {hasRequestData ? (
                <LineChart
                  data={chartData}
                  xKey="time"
                  yKey="requests"
                  height={180}
                  formatter={formatNumber}
                  xFormatter={xFormatter}
                  color={CHART_COLORS[3]}
                  showArea
                />
              ) : (
                <div className="flex h-[180px] items-center justify-center text-muted-foreground">
                  No request data available
                </div>
              )}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

function ChartSkeleton({ title }: { title: string }) {
  return (
    <div>
      <h4 className="mb-2 text-sm font-medium text-muted-foreground">{title}</h4>
      <Skeleton className="h-[180px] w-full" />
    </div>
  );
}
