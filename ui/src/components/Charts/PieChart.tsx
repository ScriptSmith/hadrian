import { PieChart as RechartsPieChart, Pie, Cell, Tooltip, ResponsiveContainer } from "recharts";
import type { PieLabelRenderProps } from "recharts";
import { ChartA11y } from "./a11y";
import { CHART_COLORS } from "./constants";

export interface PieChartProps {
  data: Array<{ name: string; value: number }>;
  height?: number;
  innerRadius?: number;
  outerRadius?: number;
  formatter?: (value: number) => string;
  showLabel?: boolean;
  /** Short description for screen readers (used as figure aria-label). */
  ariaLabel?: string;
}

export function PieChart({
  data,
  height = 200,
  innerRadius = 50,
  outerRadius = 80,
  formatter,
  showLabel = false,
  ariaLabel,
}: PieChartProps) {
  if (!data.length) return null;

  const total = data.reduce((sum, entry) => sum + entry.value, 0);

  return (
    <ChartA11y
      ariaLabel={ariaLabel ?? "Distribution chart"}
      data={data as unknown as ReadonlyArray<Record<string, unknown>>}
      columns={[
        { header: "Name", render: (row) => row.name as string },
        {
          header: "Value",
          render: (row) =>
            formatter && typeof row.value === "number"
              ? formatter(row.value as number)
              : (row.value as number),
        },
        {
          header: "Share",
          render: (row) =>
            total > 0 ? `${(((row.value as number) / total) * 100).toFixed(1)}%` : "0%",
        },
      ]}
    >
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
                ? (props: PieLabelRenderProps) =>
                    `${props.name ?? ""} (${((props.percent ?? 0) * 100).toFixed(0)}%)`
                : undefined
            }
            labelLine={showLabel}
          >
            {data.map((_, index) => (
              <Cell key={`cell-${index}`} fill={CHART_COLORS[index % CHART_COLORS.length]} />
            ))}
          </Pie>
          <Tooltip
            isAnimationActive={false}
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
    </ChartA11y>
  );
}
