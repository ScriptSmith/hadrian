import { Monitor, Globe, Clock, Calendar, Trash2 } from "lucide-react";
import type { SessionInfo } from "@/api/generated";

export interface SessionCardProps {
  session: SessionInfo;
  onRevoke?: (sessionId: string) => void;
  isRevoking?: boolean;
}

function formatRelativeTime(date: Date): string {
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();

  // Handle future dates (e.g., expires_at)
  if (diffMs < 0) {
    const futureHours = Math.floor(Math.abs(diffMs) / (1000 * 60 * 60));
    const futureDays = Math.floor(Math.abs(diffMs) / (1000 * 60 * 60 * 24));
    if (futureHours < 24) return `in ${futureHours}h`;
    if (futureDays < 7) return `in ${futureDays}d`;
    return date.toLocaleDateString();
  }

  const diffSecs = Math.floor(diffMs / 1000);
  const diffMins = Math.floor(diffSecs / 60);
  const diffHours = Math.floor(diffMins / 60);
  const diffDays = Math.floor(diffHours / 24);

  if (diffSecs < 60) return "just now";
  if (diffMins < 60) return `${diffMins}m ago`;
  if (diffHours < 24) return `${diffHours}h ago`;
  if (diffDays < 7) return `${diffDays}d ago`;
  return date.toLocaleDateString();
}

function formatDateTime(dateStr: string): string {
  const date = new Date(dateStr);
  return date.toLocaleString();
}

export function SessionCard({ session, onRevoke, isRevoking = false }: SessionCardProps) {
  const createdAt = new Date(session.created_at);
  const expiresAt = new Date(session.expires_at);
  const lastActivity = session.last_activity ? new Date(session.last_activity) : null;

  return (
    <div className="bg-card border border-border rounded-lg p-4 space-y-3">
      {/* Device Info */}
      <div className="flex items-start justify-between gap-3">
        <div className="flex items-start gap-3 min-w-0 flex-1">
          <div className="flex-shrink-0 w-10 h-10 bg-primary/10 rounded-lg flex items-center justify-center">
            <Monitor className="w-5 h-5 text-primary" />
          </div>
          <div className="min-w-0 flex-1">
            <p className="text-sm font-medium text-foreground truncate">
              {session.device?.device_description || "Unknown Device"}
            </p>
            {session.device?.ip_address && (
              <div className="flex items-center gap-1.5 mt-1">
                <Globe className="w-3.5 h-3.5 text-muted-foreground" />
                <span className="text-xs text-muted-foreground">{session.device.ip_address}</span>
              </div>
            )}
          </div>
        </div>

        {onRevoke && (
          <button
            onClick={() => onRevoke(session.id)}
            disabled={isRevoking}
            className="flex-shrink-0 p-2 text-muted-foreground hover:text-destructive hover:bg-destructive/10 rounded-lg transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            title="Revoke session"
          >
            <Trash2 className="w-4 h-4" />
          </button>
        )}
      </div>

      {/* Timestamps */}
      <div className="flex flex-wrap gap-4 text-xs text-muted-foreground pt-2 border-t border-border">
        <div
          className="flex items-center gap-1.5"
          title={`Created: ${formatDateTime(session.created_at)}`}
        >
          <Calendar className="w-3.5 h-3.5" />
          <span>Created {formatRelativeTime(createdAt)}</span>
        </div>

        {lastActivity && (
          <div
            className="flex items-center gap-1.5"
            title={`Last active: ${formatDateTime(session.last_activity!)}`}
          >
            <Clock className="w-3.5 h-3.5" />
            <span>Active {formatRelativeTime(lastActivity)}</span>
          </div>
        )}

        <div
          className="flex items-center gap-1.5 ml-auto"
          title={`Expires: ${formatDateTime(session.expires_at)}`}
        >
          <span>Expires {formatRelativeTime(expiresAt)}</span>
        </div>
      </div>
    </div>
  );
}
