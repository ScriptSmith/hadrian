import { useState, useEffect, useCallback, type ReactNode } from "react";
import { Outlet, useLocation, useNavigate } from "react-router-dom";
import {
  MessageSquare,
  LayoutDashboard,
  Building2,
  Users,
  Key,
  Server,
  DollarSign,
  BarChart3,
  Settings,
  Plus,
  FolderOpen,
  Palette,
} from "lucide-react";
import { AlphaBanner } from "@/components/AlphaBanner/AlphaBanner";
import { Header } from "@/components/Header/Header";
import { Sidebar } from "@/components/Sidebar/Sidebar";
import { usePreferences } from "@/preferences/PreferencesProvider";
import { useCommandPalette } from "@/components/CommandPalette/CommandPalette";
import { useResizable } from "@/hooks/useResizable";
import { SIDEBAR_MIN_WIDTH, SIDEBAR_MAX_WIDTH, SIDEBAR_DEFAULT_WIDTH } from "@/preferences/types";
import { cn } from "@/utils/cn";

interface AppLayoutProps {
  children?: ReactNode;
}

export function AppLayout({ children }: AppLayoutProps) {
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const { preferences, setPreferences } = usePreferences();
  const navigate = useNavigate();
  const location = useLocation();
  const { registerCommand, unregisterCommand } = useCommandPalette();

  const isChatRoute =
    location.pathname === "/" ||
    location.pathname === "/chat" ||
    location.pathname.startsWith("/chat/");

  const handleResizeEnd = useCallback(
    (newWidth: number) => {
      setPreferences({ sidebarWidth: newWidth });
    },
    [setPreferences]
  );

  const {
    width: sidebarWidth,
    isDragging: isResizing,
    handleProps: resizeHandleProps,
  } = useResizable({
    initialWidth: preferences.sidebarWidth ?? SIDEBAR_DEFAULT_WIDTH,
    minWidth: SIDEBAR_MIN_WIDTH,
    maxWidth: SIDEBAR_MAX_WIDTH,
    defaultWidth: SIDEBAR_DEFAULT_WIDTH,
    onResizeEnd: handleResizeEnd,
  });

  // Register navigation commands
  useEffect(() => {
    const commands = [
      {
        id: "new-chat",
        label: "New Chat",
        description: "Start a new conversation",
        icon: <Plus className="h-4 w-4" />,
        shortcut: ["N"],
        category: "Chat",
        onSelect: () => navigate("/chat"),
      },
      {
        id: "go-chat",
        label: "Go to Chat",
        description: "Open the chat interface",
        icon: <MessageSquare className="h-4 w-4" />,
        shortcut: ["G", "C"],
        category: "Navigation",
        onSelect: () => navigate("/chat"),
      },
      {
        id: "go-studio",
        label: "Go to Studio",
        description: "Open the creative studio",
        icon: <Palette className="h-4 w-4" />,
        shortcut: ["G", "S"],
        category: "Navigation",
        onSelect: () => navigate("/studio"),
      },
      {
        id: "go-dashboard",
        label: "Go to Dashboard",
        description: "View the admin dashboard",
        icon: <LayoutDashboard className="h-4 w-4" />,
        shortcut: ["G", "D"],
        category: "Navigation",
        onSelect: () => navigate("/admin"),
      },
      {
        id: "go-orgs",
        label: "Go to Organizations",
        description: "Manage organizations",
        icon: <Building2 className="h-4 w-4" />,
        category: "Admin",
        onSelect: () => navigate("/admin/organizations"),
      },
      {
        id: "go-projects",
        label: "Go to Projects",
        description: "Manage projects",
        icon: <FolderOpen className="h-4 w-4" />,
        category: "Admin",
        onSelect: () => navigate("/admin/projects"),
      },
      {
        id: "go-users",
        label: "Go to Users",
        description: "Manage users",
        icon: <Users className="h-4 w-4" />,
        category: "Admin",
        onSelect: () => navigate("/admin/users"),
      },
      {
        id: "go-api-keys",
        label: "Go to API Keys",
        description: "Manage API keys",
        icon: <Key className="h-4 w-4" />,
        category: "Admin",
        onSelect: () => navigate("/admin/api-keys"),
      },
      {
        id: "go-providers",
        label: "Go to Providers",
        description: "Configure LLM providers",
        icon: <Server className="h-4 w-4" />,
        category: "Admin",
        onSelect: () => navigate("/admin/providers"),
      },
      {
        id: "go-pricing",
        label: "Go to Pricing",
        description: "Configure model pricing",
        icon: <DollarSign className="h-4 w-4" />,
        category: "Admin",
        onSelect: () => navigate("/admin/pricing"),
      },
      {
        id: "go-usage",
        label: "Go to Usage",
        description: "View usage statistics",
        icon: <BarChart3 className="h-4 w-4" />,
        category: "Admin",
        onSelect: () => navigate("/admin/usage"),
      },
      {
        id: "go-settings",
        label: "Go to Settings",
        description: "Configure gateway settings",
        icon: <Settings className="h-4 w-4" />,
        category: "Admin",
        onSelect: () => navigate("/admin/settings"),
      },
      {
        id: "toggle-sidebar",
        label: "Toggle Sidebar",
        description: "Collapse or expand the sidebar",
        category: "View",
        onSelect: () => setPreferences({ sidebarCollapsed: !preferences.sidebarCollapsed }),
      },
    ];

    commands.forEach(registerCommand);

    return () => {
      commands.forEach((cmd) => unregisterCommand(cmd.id));
    };
  }, [navigate, registerCommand, unregisterCommand, preferences.sidebarCollapsed, setPreferences]);

  return (
    <div className="flex h-screen flex-col bg-background">
      {/* Skip to main content link - visible on focus for keyboard users */}
      <a
        href="#main-content"
        className="sr-only focus:not-sr-only focus:absolute focus:z-[100] focus:m-2 focus:rounded-lg focus:bg-primary focus:px-4 focus:py-2 focus:text-primary-foreground focus:outline-none focus:ring-2 focus:ring-ring"
      >
        Skip to main content
      </a>

      <AlphaBanner />
      <Header showMenuButton={isChatRoute} onMenuClick={() => setSidebarOpen(true)} />

      <div className="flex flex-1 overflow-hidden">
        {isChatRoute && (
          <Sidebar
            open={sidebarOpen}
            onClose={() => setSidebarOpen(false)}
            collapsed={preferences.sidebarCollapsed}
            onCollapsedChange={(collapsed) => setPreferences({ sidebarCollapsed: collapsed })}
            width={sidebarWidth}
            isResizing={isResizing}
            resizeHandleProps={resizeHandleProps}
          />
        )}

        <main
          id="main-content"
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
