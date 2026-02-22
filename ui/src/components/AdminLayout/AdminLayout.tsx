import { useState, useCallback, useEffect, type ReactNode } from "react";
import { Outlet, useNavigate } from "react-router-dom";
import {
  LayoutDashboard,
  Building2,
  Users,
  Key,
  Server,
  DollarSign,
  BarChart3,
  Settings,
  FolderOpen,
} from "lucide-react";
import { AlphaBanner } from "@/components/AlphaBanner/AlphaBanner";
import { Header } from "@/components/Header/Header";
import { AdminSidebar } from "./AdminSidebar";
import { usePreferences } from "@/preferences/PreferencesProvider";
import { useCommandPalette } from "@/components/CommandPalette/CommandPalette";
import { useResizable } from "@/hooks/useResizable";
import { SIDEBAR_MIN_WIDTH, SIDEBAR_MAX_WIDTH, SIDEBAR_DEFAULT_WIDTH } from "@/preferences/types";
import { cn } from "@/utils/cn";

export interface AdminLayoutProps {
  children?: ReactNode;
}

export function AdminLayout({ children }: AdminLayoutProps) {
  const [mobileMenuOpen, setMobileMenuOpen] = useState(false);
  const { preferences, setPreferences } = usePreferences();
  const navigate = useNavigate();
  const { registerCommand, unregisterCommand } = useCommandPalette();

  const handleResizeEnd = useCallback(
    (newWidth: number) => {
      setPreferences({ adminSidebarWidth: newWidth });
    },
    [setPreferences]
  );

  // Use adminSidebarWidth or fall back to sidebarWidth
  const initialWidth =
    preferences.adminSidebarWidth ?? preferences.sidebarWidth ?? SIDEBAR_DEFAULT_WIDTH;

  const {
    width: sidebarWidth,
    isDragging: isResizing,
    handleProps: resizeHandleProps,
  } = useResizable({
    initialWidth,
    minWidth: SIDEBAR_MIN_WIDTH,
    maxWidth: SIDEBAR_MAX_WIDTH,
    defaultWidth: SIDEBAR_DEFAULT_WIDTH,
    onResizeEnd: handleResizeEnd,
  });

  // Use separate collapsed state for admin sidebar
  const adminSidebarCollapsed = preferences.adminSidebarCollapsed ?? false;

  const handleCollapsedChange = useCallback(
    (collapsed: boolean) => {
      setPreferences({ adminSidebarCollapsed: collapsed });
    },
    [setPreferences]
  );

  // Register admin navigation commands
  useEffect(() => {
    const commands = [
      {
        id: "admin-go-dashboard",
        label: "Go to Dashboard",
        description: "View the admin dashboard",
        icon: <LayoutDashboard className="h-4 w-4" />,
        shortcut: ["G", "D"],
        category: "Admin",
        onSelect: () => navigate("/admin"),
      },
      {
        id: "admin-go-orgs",
        label: "Go to Organizations",
        description: "Manage organizations",
        icon: <Building2 className="h-4 w-4" />,
        category: "Admin",
        onSelect: () => navigate("/admin/organizations"),
      },
      {
        id: "admin-go-projects",
        label: "Go to Projects",
        description: "Manage projects",
        icon: <FolderOpen className="h-4 w-4" />,
        category: "Admin",
        onSelect: () => navigate("/admin/projects"),
      },
      {
        id: "admin-go-users",
        label: "Go to Users",
        description: "Manage users",
        icon: <Users className="h-4 w-4" />,
        category: "Admin",
        onSelect: () => navigate("/admin/users"),
      },
      {
        id: "admin-go-api-keys",
        label: "Go to API Keys",
        description: "Manage API keys",
        icon: <Key className="h-4 w-4" />,
        category: "Admin",
        onSelect: () => navigate("/admin/api-keys"),
      },
      {
        id: "admin-go-providers",
        label: "Go to Providers",
        description: "Configure LLM providers",
        icon: <Server className="h-4 w-4" />,
        category: "Admin",
        onSelect: () => navigate("/admin/providers"),
      },
      {
        id: "admin-go-pricing",
        label: "Go to Pricing",
        description: "Configure model pricing",
        icon: <DollarSign className="h-4 w-4" />,
        category: "Admin",
        onSelect: () => navigate("/admin/pricing"),
      },
      {
        id: "admin-go-usage",
        label: "Go to Usage",
        description: "View usage statistics",
        icon: <BarChart3 className="h-4 w-4" />,
        category: "Admin",
        onSelect: () => navigate("/admin/usage"),
      },
      {
        id: "admin-go-settings",
        label: "Go to Settings",
        description: "Configure gateway settings",
        icon: <Settings className="h-4 w-4" />,
        category: "Admin",
        onSelect: () => navigate("/admin/settings"),
      },
      {
        id: "admin-toggle-sidebar",
        label: "Toggle Admin Sidebar",
        description: "Collapse or expand the admin sidebar",
        category: "View",
        onSelect: () => handleCollapsedChange(!adminSidebarCollapsed),
      },
    ];

    commands.forEach(registerCommand);

    return () => {
      commands.forEach((cmd) => unregisterCommand(cmd.id));
    };
  }, [navigate, registerCommand, unregisterCommand, adminSidebarCollapsed, handleCollapsedChange]);

  return (
    <div className="flex h-screen flex-col bg-background">
      {/* Skip to main content link */}
      <a
        href="#admin-main-content"
        className="sr-only focus:not-sr-only focus:absolute focus:z-[100] focus:m-2 focus:rounded-lg focus:bg-primary focus:px-4 focus:py-2 focus:text-primary-foreground focus:outline-none focus:ring-2 focus:ring-ring"
      >
        Skip to main content
      </a>

      <AlphaBanner />
      <Header showMenuButton onMenuClick={() => setMobileMenuOpen(true)} />

      <div className="flex flex-1 overflow-hidden">
        {/* Desktop sidebar */}
        <div className="hidden lg:block">
          <AdminSidebar
            collapsed={adminSidebarCollapsed}
            onCollapsedChange={handleCollapsedChange}
            width={sidebarWidth}
            isResizing={isResizing}
            resizeHandleProps={resizeHandleProps}
          />
        </div>

        {/* Mobile sidebar overlay */}
        {mobileMenuOpen && (
          <div
            className="fixed inset-0 z-40 bg-background/80 backdrop-blur-sm lg:hidden"
            onClick={() => setMobileMenuOpen(false)}
            aria-hidden="true"
          />
        )}

        {/* Mobile sidebar */}
        <div
          className={cn(
            "fixed inset-y-0 left-0 z-50 lg:hidden",
            "transform transition-transform duration-200 ease-in-out",
            mobileMenuOpen ? "translate-x-0" : "-translate-x-full"
          )}
        >
          <AdminSidebar
            collapsed={false}
            onCollapsedChange={() => setMobileMenuOpen(false)}
            width={280}
          />
        </div>

        <main
          id="admin-main-content"
          className={cn(
            "flex-1 overflow-y-auto bg-background border-l",
            "transition-all duration-300"
          )}
        >
          {children ?? <Outlet />}
        </main>
      </div>
    </div>
  );
}
