import { useQuery } from "@tanstack/react-query";
import { Shield, AlertCircle, Info } from "lucide-react";
import { ssoConnectionsListOptions } from "@/api/generated/@tanstack/react-query.gen";
import { PageHeader } from "@/components/Admin";
import { SsoConnectionCard } from "@/components/SsoConnections";
import { Card, CardContent } from "@/components/Card/Card";
import { Skeleton } from "@/components/Skeleton/Skeleton";

export default function SsoConnectionsPage() {
  const { data, isLoading, error } = useQuery(ssoConnectionsListOptions());

  return (
    <div className="p-6">
      <PageHeader
        title="SSO Connections"
        description="View configured Single Sign-On authentication providers"
      />

      {/* Info banner */}
      <div className="mb-6 flex items-start gap-3 rounded-lg border bg-muted/30 p-4">
        <Info className="h-5 w-5 text-muted-foreground mt-0.5" />
        <div className="text-sm">
          <p className="font-medium">Configuration is read-only</p>
          <p className="text-muted-foreground">
            SSO connections are configured in <code className="text-xs">gateway.toml</code>. To add
            or modify connections, update the configuration file and restart the gateway.
          </p>
        </div>
      </div>

      {/* Loading state */}
      {isLoading && (
        <div className="space-y-4">
          <Card>
            <CardContent className="p-6">
              <div className="flex items-center gap-3">
                <Skeleton className="h-10 w-10 rounded-lg" />
                <div className="space-y-2">
                  <Skeleton className="h-5 w-32" />
                  <Skeleton className="h-4 w-48" />
                </div>
              </div>
              <div className="mt-4 space-y-3">
                <Skeleton className="h-4 w-full" />
                <Skeleton className="h-4 w-3/4" />
                <Skeleton className="h-4 w-1/2" />
              </div>
            </CardContent>
          </Card>
        </div>
      )}

      {/* Error state */}
      {error && (
        <Card className="border-destructive">
          <CardContent className="flex items-center gap-3 p-6">
            <AlertCircle className="h-5 w-5 text-destructive" />
            <div>
              <p className="font-medium">Failed to load SSO connections</p>
              <p className="text-sm text-muted-foreground">{String(error)}</p>
            </div>
          </CardContent>
        </Card>
      )}

      {/* Empty state */}
      {data && data.data.length === 0 && (
        <Card>
          <CardContent className="flex flex-col items-center justify-center p-12 text-center">
            <div className="flex h-12 w-12 items-center justify-center rounded-full bg-muted">
              <Shield className="h-6 w-6 text-muted-foreground" />
            </div>
            <h3 className="mt-4 text-lg font-medium">No SSO connections configured</h3>
            <p className="mt-2 max-w-md text-sm text-muted-foreground">
              SSO connections are configured in the gateway configuration file. Add an{" "}
              <code className="text-xs">[auth.admin]</code> section with OIDC or proxy auth settings
              to enable single sign-on.
            </p>
          </CardContent>
        </Card>
      )}

      {/* Connection cards */}
      {data && data.data.length > 0 && (
        <div className="space-y-4">
          {data.data.map((connection) => (
            <SsoConnectionCard key={connection.name} connection={connection} />
          ))}
        </div>
      )}
    </div>
  );
}
