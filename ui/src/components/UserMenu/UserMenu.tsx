import { useNavigate } from "react-router-dom";
import { User, LogOut, Settings, Bug } from "lucide-react";

import { useAuth, hasAdminAccess } from "@/auth";
import { useConfig } from "@/config/ConfigProvider";
import {
  Dropdown,
  DropdownContent,
  DropdownItem,
  DropdownSeparator,
  DropdownLabel,
  DropdownTrigger,
} from "@/components/Dropdown/Dropdown";
import { navItems, adminNavItem } from "@/components/Header/Header";
import { getPageConfig } from "@/components/PageGuard/PageGuard";
import { cn } from "@/utils/cn";

interface UserMenuProps {
  className?: string;
}

export function UserMenu({ className }: UserMenuProps) {
  const { user, logout, isAuthenticated, method } = useAuth();
  const { config } = useConfig();
  const navigate = useNavigate();
  const visibleNavItems = navItems.filter((item) => {
    if (!item.pageKey) return true;
    return getPageConfig(config.pages, item.pageKey).status !== "disabled";
  });
  const showAdmin = config?.admin.enabled && hasAdminAccess(user);
  const allNavItems = showAdmin ? [...visibleNavItems, adminNavItem] : visibleNavItems;
  const isAnonymous = method === "none";

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
        {isAnonymous ? <User className="h-4 w-4" /> : initials || <User className="h-4 w-4" />}
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
        {/* Page navigation — only visible on mobile where the header tabs are hidden */}
        <div className="xl:hidden">
          <DropdownSeparator />
          <DropdownLabel>Navigate</DropdownLabel>
          {allNavItems.map((item) => {
            const Icon = item.icon;
            return (
              <DropdownItem key={item.to} onClick={() => navigate(item.to)}>
                <Icon className="mr-2 h-4 w-4" />
                {item.label}
              </DropdownItem>
            );
          })}
        </div>
        <DropdownSeparator />
        <DropdownItem onClick={() => navigate("/account")}>
          <Settings className="mr-2 h-4 w-4" />
          Account Settings
        </DropdownItem>
        {getPageConfig(config.pages, "admin.session_info").status !== "disabled" && (
          <DropdownItem onClick={() => navigate("/session")}>
            <Bug className="mr-2 h-4 w-4" />
            Session Info
          </DropdownItem>
        )}
        <DropdownSeparator />
        <DropdownItem onClick={logout} className="text-destructive">
          <LogOut className="mr-2 h-4 w-4" />
          Log out
        </DropdownItem>
      </DropdownContent>
    </Dropdown>
  );
}
