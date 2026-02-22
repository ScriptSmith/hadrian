import { Power, PowerOff } from "lucide-react";

import { Badge } from "@/components/Badge/Badge";

export type ApiKeyStatus = "active" | "revoked" | "expired";
export type EnabledStatus = "enabled" | "disabled";

export interface ApiKeyStatusBadgeProps {
  revokedAt?: string | null;
  expiresAt?: string | null;
}

export function ApiKeyStatusBadge({ revokedAt, expiresAt }: ApiKeyStatusBadgeProps) {
  if (revokedAt) {
    return <Badge variant="destructive">Revoked</Badge>;
  }
  if (expiresAt && new Date(expiresAt) < new Date()) {
    return <Badge variant="destructive">Expired</Badge>;
  }
  return <Badge variant="secondary">Active</Badge>;
}

export interface EnabledStatusBadgeProps {
  isEnabled: boolean;
  showIcon?: boolean;
}

export function EnabledStatusBadge({ isEnabled, showIcon = true }: EnabledStatusBadgeProps) {
  if (isEnabled) {
    return (
      <Badge variant="secondary" className={showIcon ? "gap-1" : ""}>
        {showIcon && <Power className="h-3 w-3" />}
        Enabled
      </Badge>
    );
  }
  return (
    <Badge variant="destructive" className={showIcon ? "gap-1" : ""}>
      {showIcon && <PowerOff className="h-3 w-3" />}
      Disabled
    </Badge>
  );
}

export interface SimpleStatusBadgeProps {
  status: "active" | "inactive" | "success" | "error" | "warning" | "pending";
  label?: string;
}

export function SimpleStatusBadge({ status, label }: SimpleStatusBadgeProps) {
  const variants: Record<string, "default" | "secondary" | "destructive" | "outline" | "success"> =
    {
      active: "success",
      inactive: "secondary",
      success: "success",
      error: "destructive",
      warning: "outline",
      pending: "secondary",
    };

  const defaultLabels: Record<string, string> = {
    active: "Active",
    inactive: "Inactive",
    success: "Success",
    error: "Error",
    warning: "Warning",
    pending: "Pending",
  };

  return <Badge variant={variants[status]}>{label || defaultLabels[status]}</Badge>;
}
