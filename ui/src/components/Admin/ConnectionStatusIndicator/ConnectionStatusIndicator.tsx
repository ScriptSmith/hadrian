import { WifiOff, Loader2 } from "lucide-react";

import { cn } from "@/utils/cn";
import type { WebSocketConnectionStatus } from "@/services/websocket";

export interface ConnectionStatusIndicatorProps {
  /** Current connection status */
  status: WebSocketConnectionStatus;
  /** Error message (shown in title when status is "error") */
  error?: string;
  /** Show indicator when disconnected (default: false) */
  showDisconnected?: boolean;
  /** Additional class names */
  className?: string;
}

/**
 * Visual indicator for real-time WebSocket connection status.
 *
 * Shows a "Live" badge when connected, "Connecting..." when connecting/reconnecting,
 * and optionally an "Offline" state when disconnected.
 */
export function ConnectionStatusIndicator({
  status,
  error,
  showDisconnected = false,
  className,
}: ConnectionStatusIndicatorProps) {
  // Don't render anything for disconnected state unless explicitly requested
  if (!showDisconnected && (status === "disconnected" || status === "error")) {
    return null;
  }

  if (status === "connected") {
    return (
      <div
        className={cn(
          "inline-flex items-center gap-1.5 rounded-full bg-success/10 px-2.5 py-1 text-xs font-medium text-success",
          className
        )}
        title="Real-time updates enabled"
      >
        <span className="relative flex h-2 w-2">
          <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-success opacity-75" />
          <span className="relative inline-flex h-2 w-2 rounded-full bg-success" />
        </span>
        Live
      </div>
    );
  }

  if (status === "connecting" || status === "reconnecting") {
    return (
      <div
        className={cn(
          "inline-flex items-center gap-1.5 rounded-full bg-warning/10 px-2.5 py-1 text-xs font-medium text-warning",
          className
        )}
        title={status === "reconnecting" ? "Reconnecting to server..." : "Connecting to server..."}
      >
        <Loader2 className="h-3 w-3 animate-spin" />
        {status === "reconnecting" ? "Reconnecting..." : "Connecting..."}
      </div>
    );
  }

  // Disconnected or error state (only shown when showDisconnected is true)
  return (
    <div
      className={cn(
        "inline-flex items-center gap-1.5 rounded-full bg-muted px-2.5 py-1 text-xs font-medium text-muted-foreground",
        className
      )}
      title={error || "Disconnected from real-time updates"}
    >
      <WifiOff className="h-3 w-3" />
      Offline
    </div>
  );
}

// Re-export the type for convenience
export type { WebSocketConnectionStatus };
