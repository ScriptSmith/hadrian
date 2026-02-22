import { cn } from "@/utils/cn";
import type { StatsGranularity } from "@/api/generated/types.gen";

export interface TimeRange {
  /** ISO datetime string for start of range */
  start: string;
  /** ISO datetime string for end of range */
  end: string;
  /** Granularity for the data points */
  granularity: StatsGranularity;
}

interface TimeRangePreset {
  label: string;
  hours: number;
  granularity: StatsGranularity;
}

const TIME_RANGE_PRESETS: Record<string, TimeRangePreset> = {
  "1h": { label: "1h", hours: 1, granularity: "hour" },
  "6h": { label: "6h", hours: 6, granularity: "hour" },
  "24h": { label: "24h", hours: 24, granularity: "hour" },
  "7d": { label: "7d", hours: 168, granularity: "hour" },
  "30d": { label: "30d", hours: 720, granularity: "day" },
};

export interface TimeRangeSelectorProps {
  /** Currently selected preset key */
  value: string;
  /** Callback when a preset is selected */
  onChange: (range: TimeRange, preset: string) => void;
  /** Additional class names */
  className?: string;
}

/**
 * Converts a preset key to a TimeRange object with actual ISO datetime strings
 */
export function getTimeRangeFromPreset(presetKey: string): TimeRange {
  const preset = TIME_RANGE_PRESETS[presetKey] ?? TIME_RANGE_PRESETS["24h"];
  const end = new Date();
  const start = new Date(end.getTime() - preset.hours * 60 * 60 * 1000);

  return {
    start: start.toISOString(),
    end: end.toISOString(),
    granularity: preset.granularity,
  };
}

export function TimeRangeSelector({ value, onChange, className }: TimeRangeSelectorProps) {
  const handleClick = (presetKey: string) => {
    const range = getTimeRangeFromPreset(presetKey);
    onChange(range, presetKey);
  };

  return (
    <div className={cn("inline-flex rounded-lg border border-input p-1", className)}>
      {Object.entries(TIME_RANGE_PRESETS).map(([key, preset]) => (
        <button
          key={key}
          onClick={() => handleClick(key)}
          className={cn(
            "px-3 py-1.5 text-sm font-medium transition-colors rounded-md",
            "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-1",
            value === key
              ? "bg-primary text-primary-foreground"
              : "text-muted-foreground hover:text-foreground hover:bg-accent"
          )}
        >
          {preset.label}
        </button>
      ))}
    </div>
  );
}
