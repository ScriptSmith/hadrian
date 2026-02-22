import UsageDashboard from "@/components/UsageDashboard/UsageDashboard";

interface UsageTabProps {
  orgSlug: string;
  projectSlug: string;
}

export function UsageTab({ orgSlug, projectSlug }: UsageTabProps) {
  return (
    <div role="tabpanel" id="tabpanel-usage" aria-labelledby="tab-usage">
      <UsageDashboard scope={{ type: "project", orgSlug, projectSlug }} />
    </div>
  );
}
