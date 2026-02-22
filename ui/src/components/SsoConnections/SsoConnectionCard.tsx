import { Link } from "react-router-dom";
import {
  Shield,
  CheckCircle2,
  XCircle,
  Users,
  Building2,
  ExternalLink,
  Settings2,
} from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/Card/Card";
import { Badge } from "@/components/Badge/Badge";
import { CodeBadge } from "@/components/CodeBadge/CodeBadge";
import type { SsoConnection } from "@/api/generated/types.gen";

export interface SsoConnectionCardProps {
  /** SSO connection data */
  connection: SsoConnection;
}

/**
 * Card displaying SSO connection details.
 * SSO connections are read-only (configured in gateway.toml).
 */
export function SsoConnectionCard({ connection }: SsoConnectionCardProps) {
  const isOidc = connection.type === "oidc";
  const isProxyAuth = connection.type === "proxy_auth";

  return (
    <Card>
      <CardHeader>
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-primary/10">
              <Shield className="h-5 w-5 text-primary" />
            </div>
            <div>
              <CardTitle className="flex items-center gap-2">
                {connection.name}
                <Badge variant={isOidc ? "default" : "secondary"}>
                  {isOidc ? "OIDC" : isProxyAuth ? "Proxy Auth" : connection.type}
                </Badge>
              </CardTitle>
              <CardDescription>
                {isOidc && connection.issuer
                  ? new URL(connection.issuer).host
                  : isProxyAuth
                    ? "Reverse proxy authentication"
                    : "SSO Connection"}
              </CardDescription>
            </div>
          </div>
          <JitStatusBadge enabled={connection.jit_enabled} />
        </div>
      </CardHeader>
      <CardContent className="space-y-4">
        {isOidc && (
          <>
            {/* Issuer */}
            {connection.issuer && (
              <DetailRow
                label="Issuer"
                value={
                  <a
                    href={connection.issuer}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="flex items-center gap-1 text-primary hover:underline"
                  >
                    {connection.issuer}
                    <ExternalLink className="h-3 w-3" />
                  </a>
                }
              />
            )}

            {/* Client ID */}
            {connection.client_id && (
              <DetailRow
                label="Client ID"
                value={<CodeBadge className="text-xs">{connection.client_id}</CodeBadge>}
              />
            )}

            {/* Scopes */}
            {connection.scopes && connection.scopes.length > 0 && (
              <DetailRow
                label="Scopes"
                value={
                  <div className="flex flex-wrap gap-1">
                    {connection.scopes.map((scope) => (
                      <Badge key={scope} variant="outline" className="text-xs">
                        {scope}
                      </Badge>
                    ))}
                  </div>
                }
              />
            )}

            {/* Claims */}
            <div className="grid grid-cols-2 gap-4">
              {connection.identity_claim && (
                <DetailRow label="Identity Claim" value={connection.identity_claim} />
              )}
              {connection.groups_claim && (
                <DetailRow label="Groups Claim" value={connection.groups_claim} />
              )}
            </div>
          </>
        )}

        {/* JIT Provisioning Section */}
        {connection.jit_enabled && (
          <div className="rounded-lg border bg-muted/30 p-4">
            <h4 className="mb-3 flex items-center gap-2 text-sm font-medium">
              <Users className="h-4 w-4" />
              JIT Provisioning Settings
            </h4>
            <div className="grid gap-3 sm:grid-cols-2">
              {connection.organization_id && (
                <DetailRow
                  label="Organization"
                  value={
                    <span className="flex items-center gap-1">
                      <Building2 className="h-3 w-3" />
                      {connection.organization_id}
                    </span>
                  }
                  compact
                />
              )}
              {connection.default_team_id && (
                <DetailRow
                  label="Default Team"
                  value={
                    <span className="flex items-center gap-1">
                      <Users className="h-3 w-3" />
                      {connection.default_team_id}
                    </span>
                  }
                  compact
                />
              )}
              {connection.default_org_role && (
                <DetailRow
                  label="Default Org Role"
                  value={<Badge variant="secondary">{connection.default_org_role}</Badge>}
                  compact
                />
              )}
              {connection.default_team_role && (
                <DetailRow
                  label="Default Team Role"
                  value={<Badge variant="secondary">{connection.default_team_role}</Badge>}
                  compact
                />
              )}
            </div>
            <div className="mt-3 pt-3 border-t flex items-center justify-between">
              <DetailRow
                label="Sync on Login"
                value={
                  connection.sync_memberships_on_login ? (
                    <span className="flex items-center gap-1 text-green-700 dark:text-green-400">
                      <CheckCircle2 className="h-3 w-3" />
                      Enabled
                    </span>
                  ) : (
                    <span className="flex items-center gap-1 text-muted-foreground">
                      <XCircle className="h-3 w-3" />
                      Disabled
                    </span>
                  )
                }
                compact
              />
              {connection.organization_id && (
                <Link
                  to={`/admin/organizations/${connection.organization_id}/sso-group-mappings?connection=${connection.name}`}
                  className="inline-flex items-center justify-center gap-2 whitespace-nowrap text-sm font-medium transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-50 border border-input bg-background hover:bg-accent hover:text-accent-foreground hover:border-accent-foreground/20 h-8 rounded-md px-3"
                >
                  <Settings2 className="h-4 w-4" />
                  Manage Group Mappings
                </Link>
              )}
            </div>
          </div>
        )}

        {/* Proxy Auth section */}
        {isProxyAuth && (
          <div className="rounded-lg border bg-muted/30 p-4">
            <p className="text-sm text-muted-foreground">
              Authentication is handled by a reverse proxy. User identity is extracted from request
              headers configured in the gateway.
            </p>
          </div>
        )}
      </CardContent>
    </Card>
  );
}

function JitStatusBadge({ enabled }: { enabled: boolean }) {
  if (enabled) {
    return (
      <Badge variant="default" className="flex items-center gap-1">
        <CheckCircle2 className="h-3 w-3" />
        JIT Enabled
      </Badge>
    );
  }
  return (
    <Badge variant="secondary" className="flex items-center gap-1">
      <XCircle className="h-3 w-3" />
      JIT Disabled
    </Badge>
  );
}

interface DetailRowProps {
  label: string;
  value: React.ReactNode;
  compact?: boolean;
}

function DetailRow({ label, value, compact }: DetailRowProps) {
  if (compact) {
    return (
      <div className="flex flex-col gap-0.5">
        <span className="text-xs text-muted-foreground">{label}</span>
        <span className="text-sm">{value}</span>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-1 sm:flex-row sm:items-center sm:justify-between">
      <span className="text-sm text-muted-foreground">{label}</span>
      <span className="text-sm">{value}</span>
    </div>
  );
}
