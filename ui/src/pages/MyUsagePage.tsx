import { useState } from "react";

import { PageHeader, TabNavigation } from "@/components/Admin";
import UsageDashboard from "@/components/UsageDashboard/UsageDashboard";
import UsageLogsTable from "@/components/UsageLogs/UsageLogsTable";

type UsageTab = "analytics" | "logs";

export default function MyUsagePage() {
  const [activeTab, setActiveTab] = useState<UsageTab>("analytics");

  return (
    <div className="p-6">
      <PageHeader title="My Usage" description="View your personal usage statistics" />
      <TabNavigation
        tabs={[
          { id: "analytics" as const, label: "Analytics" },
          { id: "logs" as const, label: "Logs" },
        ]}
        activeTab={activeTab}
        onTabChange={setActiveTab}
      />
      <div
        role="tabpanel"
        id={`tabpanel-${activeTab}`}
        aria-labelledby={`tab-${activeTab}`}
        className="mt-6"
      >
        {activeTab === "analytics" ? (
          <UsageDashboard scope={{ type: "me" }} />
        ) : (
          <UsageLogsTable scope={{ type: "me" }} />
        )}
      </div>
    </div>
  );
}
