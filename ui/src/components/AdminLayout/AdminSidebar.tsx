import { NavLink, useLocation } from "react-router-dom";
import {
  LayoutDashboard,
  Building2,
  FolderOpen,
  Users,
  Users2,
  Bot,
  Key,
  Server,
  Activity,
  DollarSign,
  BarChart3,
  Settings,
  FileText,
  Database,
  ChevronLeft,
  Shield,
  Bug,
} from "lucide-react";
import { cn } from "@/utils/cn";
import { Button } from "@/components/Button/Button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/Tooltip/Tooltip";
import { SIDEBAR_MIN_WIDTH, SIDEBAR_MAX_WIDTH } from "@/preferences/types";

interface NavItem {
  to: string;
  icon: typeof LayoutDashboard;
  label: string;
  exact?: boolean;
}

const adminNavItems: NavItem[] = [
  { to: "/admin", icon: LayoutDashboard, label: "Dashboard", exact: true },
  { to: "/admin/organizations", icon: Building2, label: "Organizations" },
  { to: "/admin/projects", icon: FolderOpen, label: "Projects" },
  { to: "/admin/teams", icon: Users2, label: "Teams" },
  { to: "/admin/service-accounts", icon: Bot, label: "Service Accounts" },
  { to: "/admin/users", icon: Users, label: "Users" },
  { to: "/admin/sso", icon: Shield, label: "SSO" },
  { to: "/session", icon: Bug, label: "Session Info" },
  { to: "/admin/api-keys", icon: Key, label: "API Keys" },
  { to: "/admin/providers", icon: Server, label: "Providers" },
  { to: "/admin/provider-health", icon: Activity, label: "Provider Health" },
  { to: "/admin/vector-stores", icon: Database, label: "Knowledge Bases" },
  { to: "/admin/pricing", icon: DollarSign, label: "Pricing" },
  { to: "/admin/usage", icon: BarChart3, label: "Usage" },
  { to: "/admin/audit-logs", icon: FileText, label: "Audit Logs" },
  { to: "/admin/settings", icon: Settings, label: "Settings" },
];

export interface AdminSidebarProps {
  /** Whether the sidebar is collapsed to icons only */
  collapsed?: boolean;
  /** Callback when collapse state changes */
  onCollapsedChange?: (collapsed: boolean) => void;
  /** Sidebar width in pixels (only used when not collapsed) */
  width?: number;
  /** Whether currently resizing */
  isResizing?: boolean;
  /** Props for the resize handle */
  resizeHandleProps?: {
    onMouseDown: (e: React.MouseEvent) => void;
    onTouchStart: (e: React.TouchEvent) => void;
    onDoubleClick: () => void;
    style: React.CSSProperties;
  };
  /** Optional className for the sidebar container */
  className?: string;
}

const COLLAPSED_WIDTH = 64;
const DEFAULT_WIDTH = 220;

export function AdminSidebar({
  collapsed = false,
  onCollapsedChange,
  width = DEFAULT_WIDTH,
  isResizing = false,
  resizeHandleProps,
  className,
}: AdminSidebarProps) {
  const location = useLocation();

  const isActive = (item: NavItem) => {
    if (item.exact) {
      return location.pathname === item.to;
    }
    return location.pathname.startsWith(item.to);
  };

  const sidebarWidth = collapsed ? COLLAPSED_WIDTH : width;

  return (
    <aside
      className={cn(
        "relative flex h-full flex-col bg-card",
        "transition-[width] duration-200 ease-in-out",
        isResizing && "select-none",
        className
      )}
      style={{ width: sidebarWidth }}
    >
      {/* Header */}
      <div
        className={cn(
          "flex h-14 items-center border-b px-3",
          collapsed ? "justify-center" : "justify-between"
        )}
      >
        {collapsed ? (
          <Tooltip>
            <TooltipTrigger asChild>
              <NavLink to="/admin" className="flex items-center" aria-label="Admin Dashboard">
                <Shield className="h-6 w-6 text-primary" />
              </NavLink>
            </TooltipTrigger>
            <TooltipContent side="right">Admin Dashboard</TooltipContent>
          </Tooltip>
        ) : (
          <NavLink to="/admin" className="flex items-center gap-2">
            <Shield className="h-6 w-6 text-primary" />
            <span className="font-semibold text-foreground">Admin</span>
          </NavLink>
        )}
      </div>

      {/* Navigation */}
      <nav
        className="flex-1 overflow-y-auto px-2 py-3 scrollbar-thin"
        aria-label="Admin navigation"
      >
        <ul className="space-y-1">
          {adminNavItems.map((item) => {
            const active = isActive(item);
            const Icon = item.icon;

            if (collapsed) {
              return (
                <li key={item.to}>
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <NavLink
                        to={item.to}
                        end={item.exact}
                        aria-label={item.label}
                        className={cn(
                          "flex h-9 w-full items-center justify-center rounded-lg",
                          "transition-colors",
                          active
                            ? "bg-accent text-accent-foreground"
                            : "text-muted-foreground hover:bg-accent/50 hover:text-foreground"
                        )}
                      >
                        <Icon className="h-4 w-4" />
                      </NavLink>
                    </TooltipTrigger>
                    <TooltipContent side="right">{item.label}</TooltipContent>
                  </Tooltip>
                </li>
              );
            }

            return (
              <li key={item.to}>
                <NavLink
                  to={item.to}
                  end={item.exact}
                  className={cn(
                    "flex items-center gap-3 rounded-lg px-3 py-2 text-sm",
                    "transition-colors",
                    active
                      ? "bg-accent text-accent-foreground font-medium"
                      : "text-muted-foreground hover:bg-accent/50 hover:text-foreground"
                  )}
                >
                  <Icon className="h-4 w-4 shrink-0" />
                  <span className="truncate">{item.label}</span>
                </NavLink>
              </li>
            );
          })}
        </ul>
      </nav>

      {/* Collapse / Expand toggle */}
      {onCollapsedChange &&
        (collapsed ? (
          <div className="border-t p-2">
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  variant="ghost"
                  size="icon"
                  onClick={() => onCollapsedChange(false)}
                  className="h-9 w-full"
                  aria-label="Expand sidebar"
                >
                  <ChevronLeft className="h-4 w-4 rotate-180" />
                </Button>
              </TooltipTrigger>
              <TooltipContent side="right">Expand sidebar</TooltipContent>
            </Tooltip>
          </div>
        ) : (
          <div className="border-t px-2 py-2">
            <button
              onClick={() => onCollapsedChange(true)}
              className="flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-xs text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
              aria-label="Collapse sidebar"
            >
              <ChevronLeft className="h-3.5 w-3.5" />
              <span>Collapse</span>
            </button>
          </div>
        ))}

      {/* Resize handle */}
      {!collapsed && resizeHandleProps && (
        <div
          className={cn(
            "absolute right-0 top-0 h-full w-1 cursor-ew-resize",
            "hover:bg-primary/20 active:bg-primary/30",
            "transition-colors",
            isResizing && "bg-primary/30"
          )}
          {...resizeHandleProps}
          role="separator"
          aria-orientation="vertical"
          aria-label="Resize sidebar"
          aria-valuenow={width}
          aria-valuemin={SIDEBAR_MIN_WIDTH}
          aria-valuemax={SIDEBAR_MAX_WIDTH}
          // eslint-disable-next-line jsx-a11y/no-noninteractive-tabindex -- focusable separator is a valid WAI-ARIA pattern for resize handles
          tabIndex={0}
        />
      )}
    </aside>
  );
}
