import { useId } from "react";

export interface DateRange {
  start_date: string;
  end_date: string;
}

export interface DateRangeFilterProps {
  value: DateRange;
  onChange: (range: DateRange) => void;
  startLabel?: string;
  endLabel?: string;
  className?: string;
}

export function DateRangeFilter({
  value,
  onChange,
  startLabel = "Start Date",
  endLabel = "End Date",
  className,
}: DateRangeFilterProps) {
  const startId = useId();
  const endId = useId();

  return (
    <div className={`flex flex-wrap gap-4 ${className || ""}`}>
      <div>
        <label htmlFor={startId} className="mb-1 block text-sm font-medium">
          {startLabel}
        </label>
        <input
          id={startId}
          type="date"
          value={value.start_date}
          onChange={(e) => onChange({ ...value, start_date: e.target.value })}
          className="rounded-md border border-input bg-background px-3 py-2 text-sm"
        />
      </div>
      <div>
        <label htmlFor={endId} className="mb-1 block text-sm font-medium">
          {endLabel}
        </label>
        <input
          id={endId}
          type="date"
          value={value.end_date}
          onChange={(e) => onChange({ ...value, end_date: e.target.value })}
          className="rounded-md border border-input bg-background px-3 py-2 text-sm"
        />
      </div>
    </div>
  );
}

// Helper to create default date range (last N days)
export function getDefaultDateRange(days: number = 30): DateRange {
  const end = new Date();
  const start = new Date(Date.now() - days * 24 * 60 * 60 * 1000);
  return {
    start_date: start.toISOString().split("T")[0],
    end_date: end.toISOString().split("T")[0],
  };
}
