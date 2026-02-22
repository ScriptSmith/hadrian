import { useQuery } from "@tanstack/react-query";
import { createColumnHelper, type ColumnDef } from "@tanstack/react-table";

import { modelPricingListByProjectOptions } from "@/api/generated/@tanstack/react-query.gen";
import type { DbModelPricing } from "@/api/generated/types.gen";
import { Badge } from "@/components/Badge/Badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/Card/Card";
import { DataTable } from "@/components/DataTable/DataTable";
import { formatCurrency } from "@/utils/formatters";

const columnHelper = createColumnHelper<DbModelPricing>();

interface PricingTabProps {
  orgSlug: string;
  projectSlug: string;
}

export function PricingTab({ orgSlug, projectSlug }: PricingTabProps) {
  const { data: pricing, isLoading } = useQuery(
    modelPricingListByProjectOptions({
      path: { org_slug: orgSlug, project_slug: projectSlug },
    })
  );

  const columns = [
    columnHelper.accessor("model", {
      header: "Model",
      cell: (info) => <span className="font-medium">{info.getValue()}</span>,
    }),
    columnHelper.accessor("provider", {
      header: "Provider",
      cell: (info) => <Badge variant="secondary">{info.getValue()}</Badge>,
    }),
    columnHelper.accessor("input_per_1m_tokens", {
      header: "Input/1M",
      cell: (info) => formatCurrency(info.getValue() / 1_000_000),
    }),
    columnHelper.accessor("output_per_1m_tokens", {
      header: "Output/1M",
      cell: (info) => formatCurrency(info.getValue() / 1_000_000),
    }),
    columnHelper.accessor("source", {
      header: "Source",
      cell: (info) => <Badge variant="outline">{info.getValue()}</Badge>,
    }),
  ];

  return (
    <Card role="tabpanel" id="tabpanel-pricing" aria-labelledby="tab-pricing">
      <CardHeader>
        <CardTitle>Pricing</CardTitle>
      </CardHeader>
      <CardContent>
        <DataTable
          columns={columns as ColumnDef<DbModelPricing>[]}
          data={pricing?.data || []}
          isLoading={isLoading}
          emptyMessage="No custom pricing for this project."
          searchColumn="model"
          searchPlaceholder="Search models..."
        />
      </CardContent>
    </Card>
  );
}
