import type { Meta, StoryObj } from "@storybook/react";
import { DollarSign, TrendingUp, Cpu, Calendar } from "lucide-react";

import { StatCard, StatValue } from "./StatCard";

const meta: Meta<typeof StatCard> = {
  title: "Admin/StatCard",
  component: StatCard,
  parameters: {
    layout: "padded",
  },
};

export default meta;
type Story = StoryObj<typeof StatCard>;

export const Default: Story = {
  args: {
    title: "Total Cost",
    icon: <DollarSign className="h-4 w-4" />,
    children: <StatValue value="$1,234.56" />,
  },
};

export const Loading: Story = {
  args: {
    title: "Total Cost",
    icon: <DollarSign className="h-4 w-4" />,
    isLoading: true,
    children: <StatValue value="$1,234.56" />,
  },
};

export const WithNumber: Story = {
  args: {
    title: "Total Requests",
    icon: <TrendingUp className="h-4 w-4" />,
    children: <StatValue value="12,345" />,
  },
};

export const CustomContent: Story = {
  args: {
    title: "Date Range",
    icon: <Calendar className="h-4 w-4" />,
    children: (
      <div className="text-sm">
        <div>Jan 1, 2024</div>
        <div className="text-muted-foreground">to</div>
        <div>Jan 31, 2024</div>
      </div>
    ),
  },
};

export const Grid: Story = {
  render: () => (
    <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
      <StatCard title="Total Cost" icon={<DollarSign className="h-4 w-4" />}>
        <StatValue value="$1,234.56" />
      </StatCard>
      <StatCard title="Total Requests" icon={<TrendingUp className="h-4 w-4" />}>
        <StatValue value="12,345" />
      </StatCard>
      <StatCard title="Total Tokens" icon={<Cpu className="h-4 w-4" />}>
        <StatValue value="1.2M" />
      </StatCard>
      <StatCard title="Active Keys" icon={<Calendar className="h-4 w-4" />}>
        <StatValue value="8" />
      </StatCard>
    </div>
  ),
};
