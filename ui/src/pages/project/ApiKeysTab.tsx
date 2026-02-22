import { useQuery } from "@tanstack/react-query";
import { createColumnHelper, type ColumnDef } from "@tanstack/react-table";

import { apiKeyListByProjectOptions } from "@/api/generated/@tanstack/react-query.gen";
import type { ApiKey } from "@/api/generated/types.gen";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/Card/Card";
import { CodeBadge } from "@/components/CodeBadge/CodeBadge";
import { DataTable } from "@/components/DataTable/DataTable";
import { ApiKeyStatusBadge } from "@/components/Admin";
import { formatDateTime } from "@/utils/formatters";

const columnHelper = createColumnHelper<ApiKey>();

interface ApiKeysTabProps {
  orgSlug: string;
  projectSlug: string;
}

export function ApiKeysTab({ orgSlug, projectSlug }: ApiKeysTabProps) {
  const { data: apiKeys, isLoading } = useQuery(
    apiKeyListByProjectOptions({
      path: { org_slug: orgSlug, project_slug: projectSlug },
    })
  );

  const columns = [
    columnHelper.accessor("name", {
      header: "Name",
      cell: (info) => <span className="font-medium">{info.getValue()}</span>,
    }),
    columnHelper.accessor("key_prefix", {
      header: "Key Prefix",
      cell: (info) => <CodeBadge>{info.getValue()}...</CodeBadge>,
    }),
    columnHelper.accessor("revoked_at", {
      header: "Status",
      cell: (info) => (
        <ApiKeyStatusBadge revokedAt={info.getValue()} expiresAt={info.row.original.expires_at} />
      ),
    }),
    columnHelper.accessor("created_at", {
      header: "Created",
      cell: (info) => formatDateTime(info.getValue()),
    }),
  ];

  return (
    <Card role="tabpanel" id="tabpanel-api-keys" aria-labelledby="tab-api-keys">
      <CardHeader>
        <CardTitle>API Keys</CardTitle>
      </CardHeader>
      <CardContent>
        <DataTable
          columns={columns as ColumnDef<ApiKey>[]}
          data={apiKeys?.data || []}
          isLoading={isLoading}
          emptyMessage="No API keys for this project."
          searchColumn="name"
          searchPlaceholder="Search API keys..."
        />
      </CardContent>
    </Card>
  );
}
