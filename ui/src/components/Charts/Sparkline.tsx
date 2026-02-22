import { useMemo } from "react";
import { AreaChart, Area } from "recharts";
import { CHART_COLORS } from "./constants";

export interface SparklineProps {
  data: number[];
  width?: number;
  height?: number;
  color?: string;
  showArea?: boolean;
}

export function Sparkline({
  data,
  width = 80,
  height = 24,
  color = CHART_COLORS[0],
  showArea = true,
}: SparklineProps) {
  const chartData = useMemo(() => data.map((value, index) => ({ value, index })), [data]);

  if (!data.length) return null;

  return (
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
  );
}
