import { useQuery } from "@tanstack/react-query";
import { useEffect, useState, useCallback } from "react";
import {
  User,
  Building2,
  Users,
  FolderKanban,
  Shield,
  Clock,
  Key,
  AlertCircle,
  CheckCircle2,
  XCircle,
  Monitor,
  HardDrive,
  Copy,
  Check,
} from "lucide-react";
import { sessionInfoGetOptions } from "@/api/generated/@tanstack/react-query.gen";
import type {
  SessionInfoResponse,
  OrgMembershipInfo,
  TeamMembershipInfo,
  ProjectMembershipInfo,
} from "@/api/generated/types.gen";
import { PageHeader } from "@/components/Admin";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/Card/Card";
import { Badge } from "@/components/Badge/Badge";
import { CodeBadge } from "@/components/CodeBadge/CodeBadge";
import { Skeleton } from "@/components/Skeleton/Skeleton";
import { Button } from "@/components/Button/Button";
import { isAvailable as isOpfsAvailable, getAudioStorageStats } from "@/services/opfs/opfsService";

interface BrowserInfo {
  userAgent: string;
  platform: string;
  language: string;
  languages: string[];
  cookiesEnabled: boolean;
  onlineStatus: boolean;
  screenWidth: number;
  screenHeight: number;
  screenColorDepth: number;
  viewportWidth: number;
  viewportHeight: number;
  devicePixelRatio: number;
  touchSupport: boolean;
  hardwareConcurrency: number | null;
  deviceMemory: number | null;
  timezone: string;
  timezoneOffset: number;
}

interface StorageInfo {
  localStorage: {
    available: boolean;
    keyCount: number;
    estimatedSize: number;
  };
  sessionStorage: {
    available: boolean;
    keyCount: number;
    estimatedSize: number;
  };
  indexedDB: {
    available: boolean;
    databases: string[];
  };
  opfs: {
    available: boolean;
    fileCount: number;
    totalBytes: number;
  };
  originQuota: number | null;
  originUsage: number | null;
}

function getBrowserInfo(): BrowserInfo {
  return {
    userAgent: navigator.userAgent,
    platform: navigator.platform,
    language: navigator.language,
    languages: [...navigator.languages],
    cookiesEnabled: navigator.cookieEnabled,
    onlineStatus: navigator.onLine,
    screenWidth: screen.width,
    screenHeight: screen.height,
    screenColorDepth: screen.colorDepth,
    viewportWidth: window.innerWidth,
    viewportHeight: window.innerHeight,
    devicePixelRatio: window.devicePixelRatio,
    touchSupport: "ontouchstart" in window || navigator.maxTouchPoints > 0,
    hardwareConcurrency: navigator.hardwareConcurrency ?? null,
    deviceMemory: (navigator as Navigator & { deviceMemory?: number }).deviceMemory ?? null,
    timezone: Intl.DateTimeFormat().resolvedOptions().timeZone,
    timezoneOffset: new Date().getTimezoneOffset(),
  };
}

function getStorageSize(storage: Storage): number {
  let size = 0;
  for (let i = 0; i < storage.length; i++) {
    const key = storage.key(i);
    if (key) {
      size += key.length + (storage.getItem(key)?.length ?? 0);
    }
  }
  return size * 2; // UTF-16 uses 2 bytes per character
}

async function getStorageInfo(): Promise<StorageInfo> {
  const info: StorageInfo = {
    localStorage: { available: false, keyCount: 0, estimatedSize: 0 },
    sessionStorage: { available: false, keyCount: 0, estimatedSize: 0 },
    indexedDB: { available: false, databases: [] },
    opfs: { available: false, fileCount: 0, totalBytes: 0 },
    originQuota: null,
    originUsage: null,
  };

  // Check localStorage
  try {
    info.localStorage.available = typeof localStorage !== "undefined";
    if (info.localStorage.available) {
      info.localStorage.keyCount = localStorage.length;
      info.localStorage.estimatedSize = getStorageSize(localStorage);
    }
  } catch {
    info.localStorage.available = false;
  }

  // Check sessionStorage
  try {
    info.sessionStorage.available = typeof sessionStorage !== "undefined";
    if (info.sessionStorage.available) {
      info.sessionStorage.keyCount = sessionStorage.length;
      info.sessionStorage.estimatedSize = getStorageSize(sessionStorage);
    }
  } catch {
    info.sessionStorage.available = false;
  }

  // Check IndexedDB
  try {
    info.indexedDB.available = typeof indexedDB !== "undefined";
    if (info.indexedDB.available && "databases" in indexedDB) {
      const databases = await indexedDB.databases();
      info.indexedDB.databases = databases.map((db) => db.name ?? "unnamed").filter(Boolean);
    }
    if (navigator.storage?.estimate) {
      const estimate = await navigator.storage.estimate();
      info.originQuota = estimate.quota ?? null;
      info.originUsage = estimate.usage ?? null;
    }
  } catch {
    info.indexedDB.available = false;
  }

  // Check OPFS
  try {
    info.opfs.available = isOpfsAvailable();
    if (info.opfs.available) {
      const stats = await getAudioStorageStats();
      info.opfs.fileCount = stats.fileCount;
      info.opfs.totalBytes = stats.totalBytes;
    }
  } catch {
    info.opfs.available = false;
  }

  return info;
}

function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B";
  const k = 1024;
  const sizes = ["B", "KB", "MB", "GB"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(2))} ${sizes[i]}`;
}

function generateMarkdownReport(
  sessionData: SessionInfoResponse | undefined,
  browserInfo: BrowserInfo,
  storageInfo: StorageInfo
): string {
  const lines: string[] = [];
  const now = new Date().toISOString();

  lines.push("# Session Debug Report");
  lines.push("");
  lines.push(`Generated: ${now}`);
  lines.push("");

  // Identity section
  lines.push("## Identity");
  lines.push("");
  if (sessionData) {
    lines.push(`- **External ID:** \`${sessionData.identity.external_id}\``);
    if (sessionData.identity.email) lines.push(`- **Email:** ${sessionData.identity.email}`);
    if (sessionData.identity.name) lines.push(`- **Name:** ${sessionData.identity.name}`);
    lines.push(`- **Roles:** ${sessionData.identity.roles.join(", ") || "None"}`);
    lines.push(`- **IdP Groups:** ${sessionData.identity.idp_groups.join(", ") || "None"}`);
  } else {
    lines.push("_Session data not available_");
  }
  lines.push("");

  // Database User section
  lines.push("## Database User");
  lines.push("");
  if (sessionData?.user) {
    lines.push(`- **User ID:** \`${sessionData.user.id}\``);
    if (sessionData.user.email) lines.push(`- **Email:** ${sessionData.user.email}`);
    if (sessionData.user.name) lines.push(`- **Name:** ${sessionData.user.name}`);
    lines.push(`- **Created:** ${new Date(sessionData.user.created_at).toISOString()}`);
  } else {
    lines.push("_No database user record_");
  }
  lines.push("");

  // Memberships section
  lines.push("## Memberships");
  lines.push("");
  if (sessionData) {
    lines.push("### Organizations");
    if (sessionData.organizations.length > 0) {
      sessionData.organizations.forEach((org: OrgMembershipInfo) => {
        lines.push(`- ${org.org_name} (\`${org.org_slug}\`) - ${org.role}`);
      });
    } else {
      lines.push("_None_");
    }
    lines.push("");

    lines.push("### Teams");
    if (sessionData.teams.length > 0) {
      sessionData.teams.forEach((team: TeamMembershipInfo) => {
        lines.push(`- ${team.team_name} (\`${team.org_slug}/${team.team_slug}\`) - ${team.role}`);
      });
    } else {
      lines.push("_None_");
    }
    lines.push("");

    lines.push("### Projects");
    if (sessionData.projects.length > 0) {
      sessionData.projects.forEach((project: ProjectMembershipInfo) => {
        lines.push(
          `- ${project.project_name} (\`${project.org_slug}/${project.project_slug}\`) - ${project.role}`
        );
      });
    } else {
      lines.push("_None_");
    }
  }
  lines.push("");

  // SSO Connection
  lines.push("## SSO Connection");
  lines.push("");
  if (sessionData?.sso_connection) {
    lines.push(`- **Type:** ${sessionData.sso_connection.type.toUpperCase()}`);
    if (sessionData.sso_connection.issuer)
      lines.push(`- **Issuer:** ${sessionData.sso_connection.issuer}`);
    if (sessionData.sso_connection.groups_claim)
      lines.push(`- **Groups Claim:** \`${sessionData.sso_connection.groups_claim}\``);
    lines.push(
      `- **JIT Provisioning:** ${sessionData.sso_connection.jit_enabled ? "Enabled" : "Disabled"}`
    );
  } else {
    lines.push("_No SSO connection_");
  }
  lines.push("");

  // Auth & Server Info
  lines.push("## Auth & Server");
  lines.push("");
  if (sessionData) {
    lines.push(`- **Auth Method:** ${sessionData.auth_method}`);
    lines.push(`- **Server Time:** ${sessionData.server_time}`);
  }
  lines.push("");

  // Browser Information
  lines.push("## Browser Information");
  lines.push("");
  lines.push(`- **User Agent:** \`${browserInfo.userAgent}\``);
  lines.push(`- **Platform:** ${browserInfo.platform}`);
  lines.push(`- **Language:** ${browserInfo.language} (${browserInfo.languages.join(", ")})`);
  lines.push(
    `- **Timezone:** ${browserInfo.timezone} (UTC${browserInfo.timezoneOffset >= 0 ? "-" : "+"}${Math.abs(browserInfo.timezoneOffset / 60)})`
  );
  lines.push(
    `- **Screen:** ${browserInfo.screenWidth}x${browserInfo.screenHeight} @ ${browserInfo.screenColorDepth}-bit`
  );
  lines.push(`- **Viewport:** ${browserInfo.viewportWidth}x${browserInfo.viewportHeight}`);
  lines.push(`- **Device Pixel Ratio:** ${browserInfo.devicePixelRatio}`);
  lines.push(`- **Touch Support:** ${browserInfo.touchSupport ? "Yes" : "No"}`);
  lines.push(`- **Online:** ${browserInfo.onlineStatus ? "Yes" : "No"}`);
  lines.push(`- **Cookies Enabled:** ${browserInfo.cookiesEnabled ? "Yes" : "No"}`);
  if (browserInfo.hardwareConcurrency)
    lines.push(`- **CPU Cores:** ${browserInfo.hardwareConcurrency}`);
  if (browserInfo.deviceMemory) lines.push(`- **Device Memory:** ${browserInfo.deviceMemory} GB`);
  lines.push("");

  // Storage Information
  lines.push("## Storage");
  lines.push("");
  lines.push("### localStorage");
  lines.push(`- **Available:** ${storageInfo.localStorage.available ? "Yes" : "No"}`);
  if (storageInfo.localStorage.available) {
    lines.push(`- **Keys:** ${storageInfo.localStorage.keyCount}`);
    lines.push(`- **Size:** ${formatBytes(storageInfo.localStorage.estimatedSize)}`);
  }
  lines.push("");

  lines.push("### sessionStorage");
  lines.push(`- **Available:** ${storageInfo.sessionStorage.available ? "Yes" : "No"}`);
  if (storageInfo.sessionStorage.available) {
    lines.push(`- **Keys:** ${storageInfo.sessionStorage.keyCount}`);
    lines.push(`- **Size:** ${formatBytes(storageInfo.sessionStorage.estimatedSize)}`);
  }
  lines.push("");

  lines.push("### IndexedDB");
  lines.push(`- **Available:** ${storageInfo.indexedDB.available ? "Yes" : "No"}`);
  if (storageInfo.indexedDB.available) {
    lines.push(
      `- **Databases:** ${storageInfo.indexedDB.databases.length > 0 ? storageInfo.indexedDB.databases.join(", ") : "None"}`
    );
  }
  lines.push("");

  lines.push("### OPFS");
  lines.push(`- **Available:** ${storageInfo.opfs.available ? "Yes" : "No"}`);
  if (storageInfo.opfs.available) {
    lines.push(`- **Files:** ${storageInfo.opfs.fileCount}`);
    lines.push(`- **Size:** ${formatBytes(storageInfo.opfs.totalBytes)}`);
  }
  lines.push("");

  if (storageInfo.originUsage !== null || storageInfo.originQuota !== null) {
    lines.push("### Origin Storage Quota");
    if (storageInfo.originUsage !== null) {
      lines.push(`- **Usage:** ${formatBytes(storageInfo.originUsage)}`);
    }
    if (storageInfo.originQuota !== null) {
      lines.push(`- **Quota:** ${formatBytes(storageInfo.originQuota)}`);
    }
  }
  lines.push("");

  return lines.join("\n");
}

export default function SessionInfoPage() {
  const { data, isLoading, error } = useQuery(sessionInfoGetOptions());
  const [browserInfo, setBrowserInfo] = useState<BrowserInfo | null>(null);
  const [storageInfo, setStorageInfo] = useState<StorageInfo | null>(null);
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    setBrowserInfo(getBrowserInfo());
    getStorageInfo().then(setStorageInfo);
  }, []);

  const handleCopyMarkdown = useCallback(async () => {
    if (!browserInfo || !storageInfo) return;

    const markdown = generateMarkdownReport(data, browserInfo, storageInfo);
    try {
      await navigator.clipboard.writeText(markdown);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (err) {
      console.error("Failed to copy:", err);
    }
  }, [data, browserInfo, storageInfo]);

  return (
    <div className="p-6">
      <div className="flex items-start justify-between gap-4">
        <PageHeader
          title="Session Information"
          description="Debug your current authentication state, memberships, and access levels"
        />
        <Button
          variant="outline"
          size="sm"
          onClick={handleCopyMarkdown}
          disabled={!browserInfo || !storageInfo}
          className="shrink-0"
        >
          {copied ? (
            <>
              <Check className="mr-2 h-4 w-4" />
              Copied
            </>
          ) : (
            <>
              <Copy className="mr-2 h-4 w-4" />
              Copy as Markdown
            </>
          )}
        </Button>
      </div>

      {/* Loading state */}
      {isLoading && (
        <div className="grid gap-6 md:grid-cols-2">
          <Card>
            <CardHeader>
              <Skeleton className="h-6 w-32" />
            </CardHeader>
            <CardContent className="space-y-4">
              <Skeleton className="h-4 w-full" />
              <Skeleton className="h-4 w-3/4" />
              <Skeleton className="h-4 w-1/2" />
            </CardContent>
          </Card>
          <Card>
            <CardHeader>
              <Skeleton className="h-6 w-32" />
            </CardHeader>
            <CardContent className="space-y-4">
              <Skeleton className="h-4 w-full" />
              <Skeleton className="h-4 w-3/4" />
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
              <p className="font-medium">Failed to load session information</p>
              <p className="text-sm text-muted-foreground">{String(error)}</p>
            </div>
          </CardContent>
        </Card>
      )}

      {/* Session info */}
      {data && (
        <div className="space-y-6">
          {/* Identity Card */}
          <div className="grid gap-6 lg:grid-cols-2">
            <Card>
              <CardHeader>
                <CardTitle className="flex items-center gap-2">
                  <User className="h-5 w-5" />
                  Identity
                </CardTitle>
                <CardDescription>Information from your identity provider</CardDescription>
              </CardHeader>
              <CardContent className="space-y-4">
                <DetailRow
                  label="External ID"
                  value={<CodeBadge>{data.identity.external_id}</CodeBadge>}
                />
                {data.identity.email && <DetailRow label="Email" value={data.identity.email} />}
                {data.identity.name && <DetailRow label="Name" value={data.identity.name} />}
                <DetailRow
                  label="Roles"
                  value={
                    data.identity.roles.length > 0 ? (
                      <div className="flex flex-wrap gap-1">
                        {data.identity.roles.map((role) => (
                          <Badge key={role} variant="secondary">
                            {role}
                          </Badge>
                        ))}
                      </div>
                    ) : (
                      <span className="text-muted-foreground">None</span>
                    )
                  }
                />
              </CardContent>
            </Card>

            {/* IdP Groups Card */}
            <Card>
              <CardHeader>
                <CardTitle className="flex items-center gap-2">
                  <Key className="h-5 w-5" />
                  IdP Groups
                </CardTitle>
                <CardDescription>
                  Raw groups from your identity provider (before mapping)
                </CardDescription>
              </CardHeader>
              <CardContent>
                {data.identity.idp_groups.length > 0 ? (
                  <div className="flex flex-wrap gap-2">
                    {data.identity.idp_groups.map((group, i) => (
                      <Badge key={i} variant="outline" className="font-mono text-xs">
                        {group}
                      </Badge>
                    ))}
                  </div>
                ) : (
                  <p className="text-sm text-muted-foreground">
                    No groups provided by identity provider
                  </p>
                )}
              </CardContent>
            </Card>
          </div>

          {/* Database User Card */}
          {data.user && (
            <Card>
              <CardHeader>
                <CardTitle className="flex items-center gap-2">
                  <User className="h-5 w-5" />
                  Database User
                </CardTitle>
                <CardDescription>Your user record in Hadrian's database</CardDescription>
              </CardHeader>
              <CardContent>
                <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
                  <DetailRow
                    label="User ID"
                    value={<CodeBadge className="text-xs">{data.user.id}</CodeBadge>}
                    compact
                  />
                  {data.user.email && <DetailRow label="Email" value={data.user.email} compact />}
                  {data.user.name && <DetailRow label="Name" value={data.user.name} compact />}
                  <DetailRow
                    label="Created"
                    value={new Date(data.user.created_at).toLocaleDateString()}
                    compact
                  />
                </div>
              </CardContent>
            </Card>
          )}

          {!data.user && (
            <Card className="border-amber-500/50 bg-amber-50/50 dark:bg-amber-950/20">
              <CardContent className="flex items-center gap-3 p-6">
                <AlertCircle className="h-5 w-5 text-amber-800" />
                <div>
                  <p className="font-medium">No database user record</p>
                  <p className="text-sm text-muted-foreground">
                    Your identity is authenticated, but no corresponding user exists in the
                    database. This may indicate JIT provisioning is disabled or hasn't run yet.
                  </p>
                </div>
              </CardContent>
            </Card>
          )}

          {/* Memberships Grid */}
          <div className="grid gap-6 lg:grid-cols-3">
            {/* Organizations */}
            <Card>
              <CardHeader>
                <CardTitle className="flex items-center gap-2">
                  <Building2 className="h-5 w-5" />
                  Organizations
                  <Badge variant="secondary">{data.organizations.length}</Badge>
                </CardTitle>
              </CardHeader>
              <CardContent>
                {data.organizations.length > 0 ? (
                  <div className="space-y-3">
                    {data.organizations.map((org) => (
                      <div
                        key={org.org_id}
                        className="flex items-center justify-between rounded-lg border p-3"
                      >
                        <div>
                          <p className="font-medium">{org.org_name}</p>
                          <p className="text-xs text-muted-foreground">{org.org_slug}</p>
                        </div>
                        <Badge variant="outline">{org.role}</Badge>
                      </div>
                    ))}
                  </div>
                ) : (
                  <p className="text-sm text-muted-foreground">No organization memberships</p>
                )}
              </CardContent>
            </Card>

            {/* Teams */}
            <Card>
              <CardHeader>
                <CardTitle className="flex items-center gap-2">
                  <Users className="h-5 w-5" />
                  Teams
                  <Badge variant="secondary">{data.teams.length}</Badge>
                </CardTitle>
              </CardHeader>
              <CardContent>
                {data.teams.length > 0 ? (
                  <div className="space-y-3">
                    {data.teams.map((team) => (
                      <div
                        key={team.team_id}
                        className="flex items-center justify-between rounded-lg border p-3"
                      >
                        <div>
                          <p className="font-medium">{team.team_name}</p>
                          <p className="text-xs text-muted-foreground">
                            {team.org_slug}/{team.team_slug}
                          </p>
                        </div>
                        <Badge variant="outline">{team.role}</Badge>
                      </div>
                    ))}
                  </div>
                ) : (
                  <p className="text-sm text-muted-foreground">No team memberships</p>
                )}
              </CardContent>
            </Card>

            {/* Projects */}
            <Card>
              <CardHeader>
                <CardTitle className="flex items-center gap-2">
                  <FolderKanban className="h-5 w-5" />
                  Projects
                  <Badge variant="secondary">{data.projects.length}</Badge>
                </CardTitle>
              </CardHeader>
              <CardContent>
                {data.projects.length > 0 ? (
                  <div className="space-y-3">
                    {data.projects.map((project) => (
                      <div
                        key={project.project_id}
                        className="flex items-center justify-between rounded-lg border p-3"
                      >
                        <div>
                          <p className="font-medium">{project.project_name}</p>
                          <p className="text-xs text-muted-foreground">
                            {project.org_slug}/{project.project_slug}
                          </p>
                        </div>
                        <Badge variant="outline">{project.role}</Badge>
                      </div>
                    ))}
                  </div>
                ) : (
                  <p className="text-sm text-muted-foreground">No project memberships</p>
                )}
              </CardContent>
            </Card>
          </div>

          {/* SSO Connection & Server Info */}
          <div className={`grid gap-6 ${data.sso_connection ? "lg:grid-cols-2" : ""}`}>
            {/* SSO Connection */}
            {data.sso_connection && (
              <Card>
                <CardHeader>
                  <CardTitle className="flex items-center gap-2">
                    <Shield className="h-5 w-5" />
                    SSO Connection
                  </CardTitle>
                  <CardDescription>Single Sign-On configuration</CardDescription>
                </CardHeader>
                <CardContent className="space-y-4">
                  <DetailRow
                    label="Type"
                    value={
                      <Badge variant="default">{data.sso_connection.type.toUpperCase()}</Badge>
                    }
                  />
                  {data.sso_connection.issuer && (
                    <DetailRow label="Issuer" value={data.sso_connection.issuer} />
                  )}
                  {data.sso_connection.groups_claim && (
                    <DetailRow
                      label="Groups Claim"
                      value={<CodeBadge>{data.sso_connection.groups_claim}</CodeBadge>}
                    />
                  )}
                  <DetailRow
                    label="JIT Provisioning"
                    value={
                      data.sso_connection.jit_enabled ? (
                        <span className="flex items-center gap-1 text-green-700">
                          <CheckCircle2 className="h-4 w-4" />
                          Enabled
                        </span>
                      ) : (
                        <span className="flex items-center gap-1 text-muted-foreground">
                          <XCircle className="h-4 w-4" />
                          Disabled
                        </span>
                      )
                    }
                  />
                </CardContent>
              </Card>
            )}

            {/* Server Info */}
            <Card>
              <CardHeader>
                <CardTitle className="flex items-center gap-2">
                  <Clock className="h-5 w-5" />
                  Server Information
                </CardTitle>
                <CardDescription>Authentication and server details</CardDescription>
              </CardHeader>
              <CardContent className="space-y-4">
                <DetailRow
                  label="Auth Method"
                  value={<Badge variant="outline">{data.auth_method}</Badge>}
                />
                <DetailRow
                  label="Server Time"
                  value={new Date(data.server_time).toLocaleString()}
                />
                <DetailRow label="Your Timezone" value={browserInfo?.timezone ?? "Loading..."} />
                <DetailRow
                  label="UTC Offset"
                  value={
                    browserInfo
                      ? `UTC${browserInfo.timezoneOffset >= 0 ? "-" : "+"}${Math.abs(browserInfo.timezoneOffset / 60)}`
                      : "Loading..."
                  }
                />
              </CardContent>
            </Card>
          </div>

          {/* Browser & Storage Info */}
          <div className="grid gap-6 lg:grid-cols-2">
            {/* Browser Info */}
            <Card>
              <CardHeader>
                <CardTitle className="flex items-center gap-2">
                  <Monitor className="h-5 w-5" />
                  Browser Information
                </CardTitle>
                <CardDescription>Client environment details</CardDescription>
              </CardHeader>
              <CardContent className="space-y-4">
                {browserInfo ? (
                  <>
                    <DetailRow
                      label="User Agent"
                      value={
                        <span
                          className="max-w-xs truncate text-xs font-mono"
                          title={browserInfo.userAgent}
                        >
                          {browserInfo.userAgent}
                        </span>
                      }
                    />
                    <DetailRow label="Platform" value={browserInfo.platform} />
                    <DetailRow
                      label="Language"
                      value={`${browserInfo.language} (${browserInfo.languages.slice(0, 3).join(", ")}${browserInfo.languages.length > 3 ? "..." : ""})`}
                    />
                    <DetailRow
                      label="Screen"
                      value={`${browserInfo.screenWidth}×${browserInfo.screenHeight} @ ${browserInfo.screenColorDepth}-bit`}
                    />
                    <DetailRow
                      label="Viewport"
                      value={`${browserInfo.viewportWidth}×${browserInfo.viewportHeight}`}
                    />
                    <DetailRow
                      label="Device Pixel Ratio"
                      value={`${browserInfo.devicePixelRatio}x`}
                    />
                    <DetailRow
                      label="Touch Support"
                      value={browserInfo.touchSupport ? "Yes" : "No"}
                    />
                    <DetailRow
                      label="Online"
                      value={
                        browserInfo.onlineStatus ? (
                          <span className="flex items-center gap-1 text-green-700">
                            <CheckCircle2 className="h-3 w-3" />
                            Yes
                          </span>
                        ) : (
                          <span className="flex items-center gap-1 text-red-700">
                            <XCircle className="h-3 w-3" />
                            No
                          </span>
                        )
                      }
                    />
                    <DetailRow
                      label="Cookies"
                      value={browserInfo.cookiesEnabled ? "Enabled" : "Disabled"}
                    />
                    {browserInfo.hardwareConcurrency && (
                      <DetailRow label="CPU Cores" value={browserInfo.hardwareConcurrency} />
                    )}
                    {browserInfo.deviceMemory && (
                      <DetailRow label="Device Memory" value={`${browserInfo.deviceMemory} GB`} />
                    )}
                  </>
                ) : (
                  <div className="space-y-2">
                    <Skeleton className="h-4 w-full" />
                    <Skeleton className="h-4 w-3/4" />
                    <Skeleton className="h-4 w-1/2" />
                  </div>
                )}
              </CardContent>
            </Card>

            {/* Storage Info */}
            <Card>
              <CardHeader>
                <CardTitle className="flex items-center gap-2">
                  <HardDrive className="h-5 w-5" />
                  Storage Information
                </CardTitle>
                <CardDescription>Browser storage status and usage</CardDescription>
              </CardHeader>
              <CardContent className="space-y-6">
                {storageInfo ? (
                  <>
                    {/* localStorage */}
                    <div className="space-y-2">
                      <div className="flex items-center gap-2">
                        <span className="text-sm font-medium">localStorage</span>
                        {storageInfo.localStorage.available ? (
                          <Badge variant="outline" className="text-green-700">
                            Available
                          </Badge>
                        ) : (
                          <Badge variant="outline" className="text-red-700">
                            Unavailable
                          </Badge>
                        )}
                      </div>
                      {storageInfo.localStorage.available && (
                        <div className="grid grid-cols-2 gap-2 text-sm">
                          <span className="text-muted-foreground">Keys:</span>
                          <span>{storageInfo.localStorage.keyCount}</span>
                          <span className="text-muted-foreground">Size:</span>
                          <span>{formatBytes(storageInfo.localStorage.estimatedSize)}</span>
                        </div>
                      )}
                    </div>

                    {/* sessionStorage */}
                    <div className="space-y-2">
                      <div className="flex items-center gap-2">
                        <span className="text-sm font-medium">sessionStorage</span>
                        {storageInfo.sessionStorage.available ? (
                          <Badge variant="outline" className="text-green-700">
                            Available
                          </Badge>
                        ) : (
                          <Badge variant="outline" className="text-red-700">
                            Unavailable
                          </Badge>
                        )}
                      </div>
                      {storageInfo.sessionStorage.available && (
                        <div className="grid grid-cols-2 gap-2 text-sm">
                          <span className="text-muted-foreground">Keys:</span>
                          <span>{storageInfo.sessionStorage.keyCount}</span>
                          <span className="text-muted-foreground">Size:</span>
                          <span>{formatBytes(storageInfo.sessionStorage.estimatedSize)}</span>
                        </div>
                      )}
                    </div>

                    {/* IndexedDB */}
                    <div className="space-y-2">
                      <div className="flex items-center gap-2">
                        <span className="text-sm font-medium">IndexedDB</span>
                        {storageInfo.indexedDB.available ? (
                          <Badge variant="outline" className="text-green-700">
                            Available
                          </Badge>
                        ) : (
                          <Badge variant="outline" className="text-red-700">
                            Unavailable
                          </Badge>
                        )}
                      </div>
                      {storageInfo.indexedDB.available && (
                        <div className="grid grid-cols-2 gap-2 text-sm">
                          <span className="text-muted-foreground">Databases:</span>
                          <span>
                            {storageInfo.indexedDB.databases.length > 0
                              ? storageInfo.indexedDB.databases.join(", ")
                              : "None"}
                          </span>
                        </div>
                      )}
                    </div>

                    {/* OPFS */}
                    <div className="space-y-2">
                      <div className="flex items-center gap-2">
                        <span className="text-sm font-medium">OPFS</span>
                        {storageInfo.opfs.available ? (
                          <Badge variant="outline" className="text-green-700">
                            Available
                          </Badge>
                        ) : (
                          <Badge variant="outline" className="text-red-700">
                            Unavailable
                          </Badge>
                        )}
                      </div>
                      {storageInfo.opfs.available && (
                        <div className="grid grid-cols-2 gap-2 text-sm">
                          <span className="text-muted-foreground">Files:</span>
                          <span>{storageInfo.opfs.fileCount}</span>
                          <span className="text-muted-foreground">Size:</span>
                          <span>{formatBytes(storageInfo.opfs.totalBytes)}</span>
                        </div>
                      )}
                    </div>

                    {/* Origin Storage Quota */}
                    {(storageInfo.originUsage !== null || storageInfo.originQuota !== null) && (
                      <div className="space-y-2">
                        <span className="text-sm font-medium">Origin Storage Quota</span>
                        <div className="grid grid-cols-2 gap-2 text-sm">
                          {storageInfo.originUsage !== null && (
                            <>
                              <span className="text-muted-foreground">Usage:</span>
                              <span>{formatBytes(storageInfo.originUsage)}</span>
                            </>
                          )}
                          {storageInfo.originQuota !== null && (
                            <>
                              <span className="text-muted-foreground">Quota:</span>
                              <span>{formatBytes(storageInfo.originQuota)}</span>
                            </>
                          )}
                        </div>
                      </div>
                    )}
                  </>
                ) : (
                  <div className="space-y-2">
                    <Skeleton className="h-4 w-full" />
                    <Skeleton className="h-4 w-3/4" />
                    <Skeleton className="h-4 w-1/2" />
                  </div>
                )}
              </CardContent>
            </Card>
          </div>
        </div>
      )}
    </div>
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
