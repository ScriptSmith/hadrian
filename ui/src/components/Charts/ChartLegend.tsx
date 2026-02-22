import { CHART_COLORS } from "./constants";

export interface ChartLegendProps {
  items: Array<{ name: string; value?: number; color?: string }>;
  formatter?: (value: number) => string;
}

export function ChartLegend({ items, formatter }: ChartLegendProps) {
  return (
    <div className="flex flex-wrap gap-x-4 gap-y-1">
      {items.map((item, index) => (
        <div key={item.name} className="flex items-center gap-1.5 text-sm">
          <span
            className="h-2.5 w-2.5 rounded-full"
            style={{ backgroundColor: item.color || CHART_COLORS[index % CHART_COLORS.length] }}
          />
          <span className="text-muted-foreground">{item.name}</span>
          {item.value !== undefined && (
            <span className="font-mono font-medium">
              {formatter ? formatter(item.value) : item.value}
            </span>
          )}
        </div>
      ))}
    </div>
  );
}
