import { Badge } from "@/components/Badge/Badge";

export type OwnerType = "global" | "organization" | "team" | "project" | "user" | "service_account";

export interface Owner {
  type: OwnerType;
  org_id?: string;
  team_id?: string;
  project_id?: string;
  user_id?: string;
  service_account_id?: string;
}

export interface OwnerBadgeProps {
  owner: Owner;
  showId?: boolean;
}

export function OwnerBadge({ owner, showId = false }: OwnerBadgeProps) {
  const getId = () => {
    switch (owner.type) {
      case "organization":
        return owner.org_id;
      case "team":
        return owner.team_id;
      case "project":
        return owner.project_id;
      case "user":
        return owner.user_id;
      case "service_account":
        return owner.service_account_id;
      default:
        return undefined;
    }
  };

  const id = getId();
  const truncatedId = id ? `${id.slice(0, 8)}...` : undefined;

  switch (owner.type) {
    case "global":
      return <Badge variant="secondary">Global</Badge>;
    case "organization":
      return (
        <Badge variant="secondary">Org{showId && truncatedId ? `: ${truncatedId}` : ""}</Badge>
      );
    case "team":
      return (
        <Badge variant="secondary">Team{showId && truncatedId ? `: ${truncatedId}` : ""}</Badge>
      );
    case "project":
      return (
        <Badge variant="outline">Project{showId && truncatedId ? `: ${truncatedId}` : ""}</Badge>
      );
    case "user":
      return <Badge>User{showId && truncatedId ? `: ${truncatedId}` : ""}</Badge>;
    case "service_account":
      return (
        <Badge variant="outline">
          Service Account{showId && truncatedId ? `: ${truncatedId}` : ""}
        </Badge>
      );
    default:
      return <Badge variant="secondary">Unknown</Badge>;
  }
}
