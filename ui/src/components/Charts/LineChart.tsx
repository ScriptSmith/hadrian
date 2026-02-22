import type { TooltipProps as RechartsTooltipProps } from "recharts";
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
import { CHART_COLORS } from "./constants";

interface ChartTooltipProps {
  active?: boolean;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  payload?: any[];
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
}: LineChartProps) {
  if (!data.length) return null;

  const Chart = showArea ? AreaChart : RechartsLineChart;

  return (
    <ResponsiveContainer width="100%" height={height}>
      <Chart data={data} margin={{ top: 5, right: 5, left: 5, bottom: 5 }}>
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
              <ChartTooltip active={active} payload={payload} label={label} formatter={formatter} />
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
  );
}
