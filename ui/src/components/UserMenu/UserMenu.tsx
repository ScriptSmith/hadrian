import { useNavigate } from "react-router-dom";
import { User, LogOut, Settings, Bug } from "lucide-react";

import { useAuth } from "@/auth";
import {
  Dropdown,
  DropdownContent,
  DropdownItem,
  DropdownSeparator,
  DropdownLabel,
  DropdownTrigger,
} from "@/components/Dropdown/Dropdown";
import { cn } from "@/utils/cn";

interface UserMenuProps {
  className?: string;
}

export function UserMenu({ className }: UserMenuProps) {
  const { user, logout, isAuthenticated } = useAuth();
  const navigate = useNavigate();

  if (!isAuthenticated) {
    return null;
  }

  const displayName = user?.name || user?.email || "User";
  const initials = displayName
    .split(" ")
    .map((n) => n[0])
    .join("")
    .toUpperCase()
    .slice(0, 2);

  return (
    <Dropdown>
      <DropdownTrigger
        showChevron={false}
        className={cn(
          "h-8 w-8 rounded-full bg-primary/10 text-primary text-xs font-medium p-0",
          "hover:bg-primary/20",
          className
        )}
        aria-label="User menu"
      >
        {initials || <User className="h-4 w-4" />}
      </DropdownTrigger>
      <DropdownContent align="end" className="w-56">
        <DropdownLabel>
          <div className="flex flex-col">
            <span className="text-sm font-medium text-foreground normal-case tracking-normal">
              {displayName}
            </span>
            {user?.email && user.email !== displayName && (
              <span className="text-xs font-normal text-muted-foreground normal-case tracking-normal">
                {user.email}
              </span>
            )}
          </div>
        </DropdownLabel>
        <DropdownSeparator />
        <DropdownItem onClick={() => navigate("/account")}>
          <Settings className="mr-2 h-4 w-4" />
          Account Settings
        </DropdownItem>
        <DropdownItem onClick={() => navigate("/session")}>
          <Bug className="mr-2 h-4 w-4" />
          Session Info
        </DropdownItem>
        <DropdownSeparator />
        <DropdownItem onClick={logout} className="text-destructive">
          <LogOut className="mr-2 h-4 w-4" />
          Log out
        </DropdownItem>
      </DropdownContent>
    </Dropdown>
  );
}
