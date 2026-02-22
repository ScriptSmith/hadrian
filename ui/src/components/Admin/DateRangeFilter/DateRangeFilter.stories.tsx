import type { Meta, StoryObj } from "@storybook/react";
import { useState } from "react";

import { DateRangeFilter, getDefaultDateRange, type DateRange } from "./DateRangeFilter";

const meta: Meta<typeof DateRangeFilter> = {
  title: "Admin/DateRangeFilter",
  component: DateRangeFilter,
  parameters: {
    layout: "padded",
  },
};

export default meta;
type Story = StoryObj<typeof DateRangeFilter>;

function DefaultStory() {
  const [range, setRange] = useState<DateRange>(getDefaultDateRange(30));
  return <DateRangeFilter value={range} onChange={setRange} />;
}

export const Default: Story = {
  render: () => <DefaultStory />,
};

function CustomLabelsStory() {
  const [range, setRange] = useState<DateRange>(getDefaultDateRange(7));
  return <DateRangeFilter value={range} onChange={setRange} startLabel="From" endLabel="To" />;
}

export const CustomLabels: Story = {
  render: () => <CustomLabelsStory />,
};

function LastWeekStory() {
  const [range, setRange] = useState<DateRange>(getDefaultDateRange(7));
  return <DateRangeFilter value={range} onChange={setRange} />;
}

export const LastWeek: Story = {
  render: () => <LastWeekStory />,
};

function InFilterRowStory() {
  const [range, setRange] = useState<DateRange>(getDefaultDateRange(30));
  const [org, setOrg] = useState("acme");

  return (
    <div className="flex flex-wrap items-end gap-4">
      <div>
        <label htmlFor="story-org" className="mb-1 block text-sm font-medium">
          Organization
        </label>
        <select
          id="story-org"
          value={org}
          onChange={(e) => setOrg(e.target.value)}
          className="rounded-md border border-input bg-background px-3 py-2 text-sm"
        >
          <option value="acme">Acme Corp</option>
          <option value="globex">Globex</option>
        </select>
      </div>
      <DateRangeFilter value={range} onChange={setRange} />
    </div>
  );
}

export const InFilterRow: Story = {
  render: () => <InFilterRowStory />,
};
