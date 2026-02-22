export interface BarChartDataItem {
  label: string;
  value: number;
  subLabel?: string;
}

export interface SimpleBarChartProps {
  data: BarChartDataItem[];
  formatter?: (value: number) => string;
  maxValue?: number;
  color?: string;
}

export function SimpleBarChart({
  data,
  formatter,
  maxValue,
  color = "bg-primary",
}: SimpleBarChartProps) {
  const max = maxValue ?? Math.max(...data.map((d) => d.value), 1);

  return (
    <div className="space-y-2">
      {data.map((item) => (
        <div key={item.label} className="flex items-center gap-2">
          <span className="w-24 truncate text-xs text-muted-foreground" title={item.label}>
            {item.label}
          </span>
          <div className="flex-1">
            <div
              className={`h-4 rounded ${color}`}
              style={{
                width: max > 0 ? `${(item.value / max) * 100}%` : "0%",
                minWidth: item.value > 0 ? "4px" : "0",
              }}
            />
          </div>
          <span className="w-16 text-right font-mono text-xs">
            {formatter ? formatter(item.value) : item.value}
          </span>
        </div>
      ))}
    </div>
  );
}
