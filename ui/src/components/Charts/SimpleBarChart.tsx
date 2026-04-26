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
  /** Short description for screen readers (used as group aria-label). */
  ariaLabel?: string;
}

export function SimpleBarChart({
  data,
  formatter,
  maxValue,
  color = "bg-primary",
  ariaLabel = "Bar chart",
}: SimpleBarChartProps) {
  const max = maxValue ?? Math.max(...data.map((d) => d.value), 1);

  return (
    <div className="space-y-2" role="group" aria-label={ariaLabel}>
      {data.map((item) => {
        const formatted = formatter ? formatter(item.value) : String(item.value);
        const percent = max > 0 ? Math.round((item.value / max) * 100) : 0;
        return (
          <div
            key={item.label}
            className="flex items-center gap-2"
            role="img"
            aria-label={`${item.label}: ${formatted} (${percent}% of max)`}
          >
            <span
              className="w-24 truncate text-xs text-muted-foreground"
              title={item.label}
              aria-hidden="true"
            >
              {item.label}
            </span>
            <div className="flex-1" aria-hidden="true">
              <div
                className={`h-4 rounded ${color}`}
                style={{
                  width: max > 0 ? `${(item.value / max) * 100}%` : "0%",
                  minWidth: item.value > 0 ? "4px" : "0",
                }}
              />
            </div>
            <span className="w-16 text-right font-mono text-xs" aria-hidden="true">
              {formatted}
            </span>
          </div>
        );
      })}
    </div>
  );
}
