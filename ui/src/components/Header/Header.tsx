import { Link, NavLink, useLocation } from "react-router-dom";
import {
  BarChart3,
  BookOpen,
  FolderOpen,
  Key,
  Menu,
  MessageSquare,
  Palette,
  Server,
  Shield,
  WandSparkles,
  UsersRound,
} from "lucide-react";
import { Button } from "@/components/Button/Button";
import { HadrianIcon } from "@/components/HadrianIcon/HadrianIcon";
import { ThemeToggle } from "@/components/ThemeToggle/ThemeToggle";
import { UserMenu } from "@/components/UserMenu/UserMenu";
import { useWasmSetup } from "@/components/WasmSetup/WasmSetupGuard";
import { useConfig } from "@/config/ConfigProvider";
import { getPageConfig, getFirstEnabledRoute } from "@/components/PageGuard/PageGuard";
import { usePreferences } from "@/preferences/PreferencesProvider";
import { useAuth, hasAdminAccess } from "@/auth";
import { cn } from "@/utils/cn";

export interface NavItem {
  to: string;
  icon: React.ComponentType<{ className?: string }>;
  label: string;
  matchPrefix?: string;
  pageKey?: string;
}

export const navItems: NavItem[] = [
  { to: "/chat", icon: MessageSquare, label: "Chat", pageKey: "chat" },
  { to: "/studio", icon: Palette, label: "Studio", pageKey: "studio" },
  { to: "/projects", icon: FolderOpen, label: "Projects", pageKey: "projects" },
  { to: "/teams", icon: UsersRound, label: "Teams", pageKey: "teams" },
  { to: "/knowledge-bases", icon: BookOpen, label: "Knowledge", pageKey: "knowledge_bases" },
  { to: "/api-keys", icon: Key, label: "API Keys", pageKey: "api_keys" },
  { to: "/providers", icon: Server, label: "Providers", pageKey: "providers" },
  { to: "/usage", icon: BarChart3, label: "Usage", pageKey: "usage" },
];

export const adminNavItem: NavItem = {
  to: "/admin",
  icon: Shield,
  label: "Admin",
  matchPrefix: "/admin",
};

interface HeaderProps {
  onMenuClick?: () => void;
  showMenuButton?: boolean;
  className?: string;
}

export function Header({ onMenuClick, showMenuButton = false, className }: HeaderProps) {
  const { config } = useConfig();
  const { resolvedTheme } = usePreferences();
  const { user } = useAuth();
  const { isWasm, openSetupWizard } = useWasmSetup();
  const location = useLocation();

  // Determine which logo to use based on theme
  const logoUrl =
    resolvedTheme === "dark" && config?.branding.logo_dark_url
      ? config.branding.logo_dark_url
      : config?.branding.logo_url;

  // Filter nav items by page visibility
  const visibleNavItems = navItems.filter((item) => {
    if (!item.pageKey) return true;
    return getPageConfig(config.pages, item.pageKey).status !== "disabled";
  });

  // Only show admin nav if admin is enabled AND user has admin access
  const showAdmin = config?.admin.enabled && hasAdminAccess(user);
  const allNavItems = showAdmin ? [...visibleNavItems, adminNavItem] : visibleNavItems;

  const isActive = (item: NavItem) => {
    if (item.matchPrefix) {
      return location.pathname.startsWith(item.matchPrefix);
    }
    return location.pathname === item.to || location.pathname.startsWith(item.to + "/");
  };

  return (
    <header
      className={cn(
        "sticky top-0 z-40 flex h-14 items-center justify-between border-b bg-background/80 backdrop-blur-sm px-4",
        className
      )}
    >
      {/* Left: Logo + Menu button (mobile) */}
      <div className="flex items-center gap-2">
        {showMenuButton && (
          <Button variant="ghost" size="icon" onClick={onMenuClick} className="lg:hidden">
            <Menu className="h-5 w-5" />
            <span className="sr-only">Toggle menu</span>
          </Button>
        )}
        <Link to={getFirstEnabledRoute(config.pages)} className="flex items-center gap-2.5">
          {logoUrl ? (
            <img
              src={logoUrl}
              alt={config?.branding.title || "Logo"}
              className="h-8 w-8 rounded-lg object-contain"
            />
          ) : (
            <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-primary/10">
              <HadrianIcon size={24} className="text-primary" />
            </div>
          )}
          <span className="hidden md:inline font-semibold text-foreground tracking-tight">
            {config?.branding.title || "Hadrian"}
          </span>
        </Link>
      </div>

      {/* Center: Navigation tabs */}
      <nav
        className="hidden xl:flex items-center gap-1"
        role="navigation"
        aria-label="Main navigation"
      >
        {allNavItems.map((item) => {
          const Icon = item.icon;
          const active = isActive(item);
          return (
            <NavLink
              key={item.to}
              to={item.to}
              className={cn(
                "flex items-center gap-1.5 px-3 py-1.5 rounded-md text-sm font-medium transition-colors",
                "hover:bg-muted hover:text-foreground",
                active ? "bg-muted text-foreground" : "text-muted-foreground"
              )}
            >
              <Icon className="h-4 w-4" aria-hidden="true" />
              <span>{item.label}</span>
            </NavLink>
          );
        })}
      </nav>

      {/* Right: Theme toggle and user menu */}
      <div className="flex items-center gap-2">
        <ThemeToggle />
        {isWasm && (
          <Button variant="ghost" size="icon" onClick={openSetupWizard} aria-label="Setup Wizard">
            <WandSparkles className="h-4 w-4" />
          </Button>
        )}
        <UserMenu />
      </div>
    </header>
  );
}
