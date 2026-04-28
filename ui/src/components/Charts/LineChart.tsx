import type {
  TooltipProps as RechartsTooltipProps,
  TooltipPayloadEntry as RechartsTooltipPayloadEntry,
} from "recharts";
import {
  LineChart as RechartsLineChart,
  Line,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
  AreaChart,
  Area,
} from "recharts";
import { ChartA11y, downsampleForChart } from "./a11y";
import { CHART_COLORS } from "./constants";

interface ChartTooltipProps {
  active?: boolean;
  payload?: ReadonlyArray<RechartsTooltipPayloadEntry<number, string>>;
  label?: string;
  formatter?: (value: number) => string;
}

function ChartTooltip({ active, payload, label, formatter }: ChartTooltipProps) {
  if (!active || !payload?.length) return null;

  return (
    <div className="rounded-lg border bg-popover px-3 py-2 text-sm shadow-md">
      {label && <div className="mb-1 font-medium text-foreground">{label}</div>}
      {payload.map((entry, index) => (
        <div key={index} className="flex items-center gap-2 text-muted-foreground">
          <span>{entry.name ?? "Value"}:</span>
          <span className="font-mono font-medium text-foreground">
            {formatter && typeof entry.value === "number" ? formatter(entry.value) : entry.value}
          </span>
        </div>
      ))}
    </div>
  );
}

type CustomTooltipContent = (
  props: RechartsTooltipProps<number, string>
) => React.ReactElement | null;

export interface LineChartProps {
  data: Array<Record<string, unknown>>;
  xKey: string;
  yKey: string;
  height?: number;
  formatter?: (value: number) => string;
  xFormatter?: (value: string) => string;
  showGrid?: boolean;
  showArea?: boolean;
  color?: string;
  /** Short description of the chart for screen readers (used as figure aria-label). */
  ariaLabel?: string;
  /** Optional per-axis labels for the SR-only data table (defaults to xKey/yKey). */
  xLabel?: string;
  yLabel?: string;
  /** Hard cap on rendered points before LTTB-style downsampling kicks in. */
  maxPoints?: number;
}

export function LineChart({
  data,
  xKey,
  yKey,
  height = 200,
  formatter,
  xFormatter,
  showGrid = true,
  showArea = false,
  color = CHART_COLORS[0],
  ariaLabel,
  xLabel,
  yLabel,
  maxPoints = 500,
}: LineChartProps) {
  if (!data.length) return null;

  const Chart = showArea ? AreaChart : RechartsLineChart;
  // Downsample once so both the SVG and the SR-only table reflect what the user
  // actually sees; passing the un-downsampled `data` to the table would create
  // a 1000-row a11y tree for charts that visually only show ~200 points.
  const renderData = downsampleForChart(data, maxPoints);

  return (
    <ChartA11y
      ariaLabel={ariaLabel ?? `${yLabel ?? yKey} over ${xLabel ?? xKey}`}
      data={renderData}
      columns={[
        {
          header: xLabel ?? xKey,
          render: (row) => {
            const v = row[xKey];
            return xFormatter && typeof v === "string" ? xFormatter(v) : (v as string | number);
          },
        },
        {
          header: yLabel ?? yKey,
          render: (row) => {
            const v = row[yKey];
            return formatter && typeof v === "number" ? formatter(v) : (v as number);
          },
        },
      ]}
    >
      <ResponsiveContainer width="100%" height={height}>
        <Chart data={renderData} margin={{ top: 5, right: 5, left: 5, bottom: 5 }}>
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
                />
              )) as CustomTooltipContent
            }
          />
          {showArea ? (
            <Area
              type="monotone"
              dataKey={yKey}
              stroke={color}
              fill={color}
              fillOpacity={0.2}
              strokeWidth={2}
            />
          ) : (
            <Line
              type="monotone"
              dataKey={yKey}
              stroke={color}
              strokeWidth={2}
              dot={false}
              activeDot={{ r: 4, strokeWidth: 0 }}
            />
          )}
        </Chart>
      </ResponsiveContainer>
    </ChartA11y>
  );
}
