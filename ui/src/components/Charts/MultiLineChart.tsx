import type { TooltipProps as RechartsTooltipProps } from "recharts";
import {
  LineChart as RechartsLineChart,
  Line,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
  Legend,
} from "recharts";
import { CHART_COLORS } from "./constants";

interface ChartTooltipProps {
  active?: boolean;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  payload?: any[];
  label?: string;
  formatter?: (value: number) => string;
  xFormatter?: (value: string) => string;
}

function ChartTooltip({ active, payload, label, formatter, xFormatter }: ChartTooltipProps) {
  if (!active || !payload?.length) return null;

  const formattedLabel = label && xFormatter ? xFormatter(label) : label;

  return (
    <div className="rounded-lg border bg-popover px-3 py-2 text-sm shadow-md">
      {formattedLabel && <div className="mb-1 font-medium text-foreground">{formattedLabel}</div>}
      {payload.map((entry, index) => (
        <div key={index} className="flex items-center gap-2 text-muted-foreground">
          <span
            className="h-2 w-2 rounded-full"
            style={{ backgroundColor: entry.color || entry.stroke }}
          />
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
    </div>
  );
}

type CustomTooltipContent = (
  props: RechartsTooltipProps<number, string>
) => React.ReactElement | null;

export interface ChartSeries {
  /** Key in the data object for this series */
  dataKey: string;
  /** Display name for the series */
  name: string;
  /** Color for the line (auto-assigned from CHART_COLORS if not provided) */
  color?: string;
}

export interface MultiLineChartProps {
  /** Array of data points, each containing values for xKey and all series dataKeys */
  data: Array<Record<string, unknown>>;
  /** Key in the data object for the X axis */
  xKey: string;
  /** Series definitions for each line */
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
}

export function MultiLineChart({
  data,
  xKey,
  series,
  height = 200,
  formatter,
  xFormatter,
  showGrid = true,
  showLegend = true,
}: MultiLineChartProps) {
  if (!data.length || !series.length) return null;

  return (
    <ResponsiveContainer width="100%" height={height}>
      <RechartsLineChart data={data} margin={{ top: 5, right: 5, left: 5, bottom: 5 }}>
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
            iconType="line"
            iconSize={10}
            formatter={(value) => <span className="text-muted-foreground">{value}</span>}
          />
        )}
        {series.map((s, index) => (
          <Line
            key={s.dataKey}
            type="monotone"
            dataKey={s.dataKey}
            name={s.name}
            stroke={s.color ?? CHART_COLORS[index % CHART_COLORS.length]}
            strokeWidth={2}
            dot={false}
            activeDot={{ r: 4, strokeWidth: 0 }}
            connectNulls
          />
        ))}
      </RechartsLineChart>
    </ResponsiveContainer>
  );
}
