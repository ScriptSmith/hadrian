import { CircleDot, CircleOff, CircleSlash } from "lucide-react";

import { Badge } from "@/components/Badge/Badge";
import type { CircuitState } from "@/api/generated";

export interface CircuitBreakerBadgeProps {
  state: CircuitState;
  showIcon?: boolean;
}

export function CircuitBreakerBadge({ state, showIcon = true }: CircuitBreakerBadgeProps) {
  switch (state) {
    case "closed":
      return (
        <Badge variant="success" className={showIcon ? "gap-1" : ""}>
          {showIcon && <CircleDot className="h-3 w-3" />}
          Closed
        </Badge>
      );
    case "open":
      return (
        <Badge variant="destructive" className={showIcon ? "gap-1" : ""}>
          {showIcon && <CircleOff className="h-3 w-3" />}
          Open
        </Badge>
      );
    case "half_open":
      return (
        <Badge variant="warning" className={showIcon ? "gap-1" : ""}>
          {showIcon && <CircleSlash className="h-3 w-3" />}
          Half Open
        </Badge>
      );
    default:
      return (
        <Badge variant="secondary" className={showIcon ? "gap-1" : ""}>
          Unknown
        </Badge>
      );
  }
}
