import { PageHeader } from "@/components/Admin";
import UsageDashboard from "@/components/UsageDashboard/UsageDashboard";

export default function MyUsagePage() {
  return (
    <div className="p-6">
      <PageHeader title="My Usage" description="View your personal usage statistics" />
      <UsageDashboard scope={{ type: "me" }} />
    </div>
  );
}
