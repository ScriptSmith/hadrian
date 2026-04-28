import type {
  TooltipProps as RechartsTooltipProps,
  TooltipPayloadEntry as RechartsTooltipPayloadEntry,
} from "recharts";
import {
  BarChart as RechartsBarChart,
  Bar,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
  Legend,
} from "recharts";
import { ChartA11y, downsampleForChart } from "./a11y";
import type { ChartSeries } from "./MultiLineChart";
import { CHART_COLORS } from "./constants";

interface ChartTooltipProps {
  active?: boolean;
  payload?: ReadonlyArray<RechartsTooltipPayloadEntry<number, string>>;
  label?: string;
  formatter?: (value: number) => string;
  xFormatter?: (value: string) => string;
}

function ChartTooltip({ active, payload, label, formatter, xFormatter }: ChartTooltipProps) {
  if (!active || !payload?.length) return null;

  const formattedLabel = label && xFormatter ? xFormatter(label) : label;
  // Show total across all stacked segments
  const total = payload.reduce(
    (sum, entry) => sum + (typeof entry.value === "number" ? entry.value : 0),
    0
  );

  return (
    <div className="rounded-lg border bg-popover px-3 py-2 text-sm shadow-md">
      {formattedLabel && <div className="mb-1 font-medium text-foreground">{formattedLabel}</div>}
      {payload.map((entry, index) => (
        <div key={index} className="flex items-center gap-2 text-muted-foreground">
          <span className="h-2 w-2 rounded-full" style={{ backgroundColor: entry.color }} />
          <span>{entry.name ?? "Value"}:</span>
          <span className="font-mono font-medium text-foreground">
            {entry.value == null
              ? "-"
              : formatter && typeof entry.value === "number"
                ? formatter(entry.value)
                : entry.value}
          </span>
        </div>
      ))}
      {payload.length > 1 && (
        <div className="mt-1 border-t pt-1 font-mono font-medium text-foreground">
          Total: {formatter ? formatter(total) : total}
        </div>
      )}
    </div>
  );
}

type CustomTooltipContent = (
  props: RechartsTooltipProps<number, string>
) => React.ReactElement | null;

export interface StackedBarChartProps {
  /** Array of data points, each containing values for xKey and all series dataKeys */
  data: Array<Record<string, unknown>>;
  /** Key in the data object for the X axis */
  xKey: string;
  /** Series definitions for each stacked segment */
  series: ChartSeries[];
  /** Chart height in pixels */
  height?: number;
  /** Formatter for Y axis values */
  formatter?: (value: number) => string;
  /** Formatter for X axis labels */
  xFormatter?: (value: string) => string;
  /** Whether to show the grid */
  showGrid?: boolean;
  /** Whether to show the legend */
  showLegend?: boolean;
  /** Short description for screen readers (used as figure aria-label). */
  ariaLabel?: string;
  /** Optional X-axis label for the SR-only data table (defaults to xKey). */
  xLabel?: string;
  /** Hard cap on rendered points before downsampling kicks in. */
  maxPoints?: number;
}

export function StackedBarChart({
  data,
  xKey,
  series,
  height = 200,
  formatter,
  xFormatter,
  showGrid = true,
  showLegend = true,
  ariaLabel,
  xLabel,
  maxPoints = 500,
}: StackedBarChartProps) {
  if (!data.length || !series.length) return null;

  const renderData = downsampleForChart(data, maxPoints);
  const seriesNames = series.map((s) => s.name).join(", ");

  return (
    <ChartA11y
      ariaLabel={ariaLabel ?? `Stacked ${seriesNames} over ${xLabel ?? xKey}`}
      data={renderData}
      columns={[
        {
          header: xLabel ?? xKey,
          render: (row) => {
            const v = row[xKey];
            return xFormatter && typeof v === "string" ? xFormatter(v) : (v as string | number);
          },
        },
        ...series.map((s) => ({
          header: s.name,
          render: (row: Record<string, unknown>) => {
            const v = row[s.dataKey];
            return formatter && typeof v === "number" ? formatter(v) : (v as number | undefined);
          },
        })),
      ]}
    >
      <ResponsiveContainer width="100%" height={height}>
        <RechartsBarChart data={renderData} margin={{ top: 5, right: 5, left: 5, bottom: 5 }}>
          {showGrid && <CartesianGrid strokeDasharray="3 3" className="stroke-border opacity-50" />}
          <XAxis
            dataKey={xKey}
            tick={{ fontSize: 11 }}
            tickFormatter={xFormatter}
            className="text-muted-foreground"
            stroke="currentColor"
            tickLine={false}
            axisLine={false}
          />
          <YAxis
            tick={{ fontSize: 11 }}
            tickFormatter={formatter}
            className="text-muted-foreground"
            stroke="currentColor"
            tickLine={false}
            axisLine={false}
            width={60}
          />
          <Tooltip
            isAnimationActive={false}
            content={
              (({ active, payload, label }: ChartTooltipProps) => (
                <ChartTooltip
                  active={active}
                  payload={payload}
                  label={label}
                  formatter={formatter}
                  xFormatter={xFormatter}
                />
              )) as CustomTooltipContent
            }
          />
          {showLegend && (
            <Legend
              wrapperStyle={{ fontSize: "12px" }}
              iconType="rect"
              iconSize={10}
              formatter={(value) => <span className="text-muted-foreground">{value}</span>}
            />
          )}
          {series.map((s, index) => (
            <Bar
              key={s.dataKey}
              dataKey={s.dataKey}
              name={s.name}
              stackId="stack"
              fill={s.color ?? CHART_COLORS[index % CHART_COLORS.length]}
            />
          ))}
        </RechartsBarChart>
      </ResponsiveContainer>
    </ChartA11y>
  );
}
