import { PieChart as RechartsPieChart, Pie, Cell, Tooltip, ResponsiveContainer } from "recharts";
import { CHART_COLORS } from "./constants";

export interface PieChartProps {
  data: Array<{ name: string; value: number }>;
  height?: number;
  innerRadius?: number;
  outerRadius?: number;
  formatter?: (value: number) => string;
  showLabel?: boolean;
}

export function PieChart({
  data,
  height = 200,
  innerRadius = 50,
  outerRadius = 80,
  formatter,
  showLabel = false,
}: PieChartProps) {
  if (!data.length) return null;

  const total = data.reduce((sum, entry) => sum + entry.value, 0);

  return (
    <ResponsiveContainer width="100%" height={height}>
      <RechartsPieChart>
        <Pie
          data={data}
          cx="50%"
          cy="50%"
          innerRadius={innerRadius}
          outerRadius={outerRadius}
          paddingAngle={2}
          dataKey="value"
          label={
            showLabel
              ? // eslint-disable-next-line @typescript-eslint/no-explicit-any
                (props: any) => `${props.name ?? ""} (${((props.percent ?? 0) * 100).toFixed(0)}%)`
              : undefined
          }
          labelLine={showLabel}
        >
          {data.map((_, index) => (
            <Cell key={`cell-${index}`} fill={CHART_COLORS[index % CHART_COLORS.length]} />
          ))}
        </Pie>
        <Tooltip
          content={({ active, payload }) => {
            if (!active || !payload?.length) return null;
            const entry = payload[0];
            const percent = ((entry.value as number) / total) * 100;
            return (
              <div className="rounded-lg border bg-popover px-3 py-2 text-sm shadow-md">
                <div className="font-medium text-foreground">{entry.name}</div>
                <div className="flex items-center gap-2 text-muted-foreground">
                  <span className="font-mono font-medium text-foreground">
                    {formatter ? formatter(entry.value as number) : entry.value}
                  </span>
                  <span>({percent.toFixed(1)}%)</span>
                </div>
              </div>
            );
          }}
        />
      </RechartsPieChart>
    </ResponsiveContainer>
  );
}
