import { Heart, HeartOff, HelpCircle } from "lucide-react";

import { Badge } from "@/components/Badge/Badge";
import type { HealthStatus } from "@/api/generated";

export interface HealthStatusBadgeProps {
  status: HealthStatus;
  showIcon?: boolean;
}

export function HealthStatusBadge({ status, showIcon = true }: HealthStatusBadgeProps) {
  switch (status) {
    case "healthy":
      return (
        <Badge variant="success" className={showIcon ? "gap-1" : ""}>
          {showIcon && <Heart className="h-3 w-3" />}
          Healthy
        </Badge>
      );
    case "unhealthy":
      return (
        <Badge variant="destructive" className={showIcon ? "gap-1" : ""}>
          {showIcon && <HeartOff className="h-3 w-3" />}
          Unhealthy
        </Badge>
      );
    case "unknown":
    default:
      return (
        <Badge variant="secondary" className={showIcon ? "gap-1" : ""}>
          {showIcon && <HelpCircle className="h-3 w-3" />}
          Unknown
        </Badge>
      );
  }
}
