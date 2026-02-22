import type { Meta, StoryObj } from "@storybook/react";
import { Users, Key, Server, DollarSign, FolderKanban } from "lucide-react";
import { useState } from "react";

import { TabNavigation } from "./TabNavigation";

const meta: Meta<typeof TabNavigation> = {
  title: "Admin/TabNavigation",
  component: TabNavigation,
  parameters: {
    layout: "padded",
    a11y: {
      config: {
        rules: [{ id: "aria-valid-attr-value", enabled: false }],
      },
    },
  },
};

export default meta;
type Story = StoryObj<typeof TabNavigation>;

type OrgTabId = "projects" | "members" | "api-keys" | "providers" | "pricing";

const orgTabs = [
  { id: "projects" as const, label: "Projects", icon: <FolderKanban className="h-4 w-4" /> },
  { id: "members" as const, label: "Members", icon: <Users className="h-4 w-4" /> },
  { id: "api-keys" as const, label: "API Keys", icon: <Key className="h-4 w-4" /> },
  { id: "providers" as const, label: "Providers", icon: <Server className="h-4 w-4" /> },
  { id: "pricing" as const, label: "Pricing", icon: <DollarSign className="h-4 w-4" /> },
];

function DefaultStory() {
  const [activeTab, setActiveTab] = useState<OrgTabId>("projects");
  return <TabNavigation tabs={orgTabs} activeTab={activeTab} onTabChange={setActiveTab} />;
}

export const Default: Story = {
  render: () => <DefaultStory />,
};

function WithoutIconsStory() {
  const [activeTab, setActiveTab] = useState<"overview" | "settings" | "logs">("overview");
  return (
    <TabNavigation
      tabs={[
        { id: "overview", label: "Overview" },
        { id: "settings", label: "Settings" },
        { id: "logs", label: "Logs" },
      ]}
      activeTab={activeTab}
      onTabChange={setActiveTab}
    />
  );
}

export const WithoutIcons: Story = {
  render: () => <WithoutIconsStory />,
};

function TwoTabsStory() {
  const [activeTab, setActiveTab] = useState<"details" | "history">("details");
  return (
    <TabNavigation
      tabs={[
        { id: "details", label: "Details", icon: <FolderKanban className="h-4 w-4" /> },
        { id: "history", label: "History", icon: <Server className="h-4 w-4" /> },
      ]}
      activeTab={activeTab}
      onTabChange={setActiveTab}
    />
  );
}

export const TwoTabs: Story = {
  render: () => <TwoTabsStory />,
};
