import { useMemo } from "react";
import { AreaChart, Area } from "recharts";
import { CHART_COLORS } from "./constants";

export interface SparklineProps {
  data: number[];
  width?: number;
  height?: number;
  color?: string;
  showArea?: boolean;
  /** Short description for screen readers (used as figure aria-label). */
  ariaLabel?: string;
}

export function Sparkline({
  data,
  width = 80,
  height = 24,
  color = CHART_COLORS[0],
  showArea = true,
  ariaLabel,
}: SparklineProps) {
  const chartData = useMemo(() => data.map((value, index) => ({ value, index })), [data]);

  if (!data.length) return null;

  // Sparklines are too small for a full data table; an aria-label summarising
  // first/last/min/max is more useful to SR users than per-point readouts.
  const first = data[0];
  const last = data[data.length - 1];
  const min = Math.min(...data);
  const max = Math.max(...data);
  const summary =
    ariaLabel ??
    `Sparkline: ${data.length} points, first ${first}, last ${last}, min ${min}, max ${max}`;

  return (
    <span role="img" aria-label={summary} className="inline-block">
      <AreaChart
        data={chartData}
        width={width}
        height={height}
        margin={{ top: 0, right: 0, left: 0, bottom: 0 }}
      >
        <Area
          type="monotone"
          dataKey="value"
          stroke={color}
          fill={showArea ? color : "transparent"}
          fillOpacity={showArea ? 0.2 : 0}
          strokeWidth={1.5}
          dot={false}
        />
      </AreaChart>
    </span>
  );
}
