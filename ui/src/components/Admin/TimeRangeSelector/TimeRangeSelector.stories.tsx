import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react";
import { TimeRangeSelector, type TimeRange } from "./TimeRangeSelector";

const meta: Meta<typeof TimeRangeSelector> = {
  title: "Components/Admin/TimeRangeSelector",
  component: TimeRangeSelector,
  parameters: {
    layout: "centered",
  },
  decorators: [
    (Story) => (
      <div className="p-8">
        <Story />
      </div>
    ),
  ],
};

export default meta;

type Story = StoryObj<typeof TimeRangeSelector>;

// Interactive story with state
function TimeRangeSelectorDemo() {
  const [selected, setSelected] = useState("24h");
  const [lastRange, setLastRange] = useState<TimeRange | null>(null);

  return (
    <div className="space-y-4">
      <TimeRangeSelector
        value={selected}
        onChange={(range, preset) => {
          setSelected(preset);
          setLastRange(range);
        }}
      />
      {lastRange && (
        <div className="text-sm text-muted-foreground space-y-1">
          <div>
            <span className="font-medium">Start:</span> {lastRange.start}
          </div>
          <div>
            <span className="font-medium">End:</span> {lastRange.end}
          </div>
          <div>
            <span className="font-medium">Granularity:</span> {lastRange.granularity}
          </div>
        </div>
      )}
    </div>
  );
}

export const Default: Story = {
  render: () => <TimeRangeSelectorDemo />,
};

export const OneHourSelected: Story = {
  args: {
    value: "1h",
    onChange: () => {},
  },
};

export const SixHoursSelected: Story = {
  args: {
    value: "6h",
    onChange: () => {},
  },
};

export const TwentyFourHoursSelected: Story = {
  args: {
    value: "24h",
    onChange: () => {},
  },
};

export const SevenDaysSelected: Story = {
  args: {
    value: "7d",
    onChange: () => {},
  },
};

export const ThirtyDaysSelected: Story = {
  args: {
    value: "30d",
    onChange: () => {},
  },
};
