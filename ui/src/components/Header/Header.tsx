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
  UsersRound,
} from "lucide-react";
import { Button } from "@/components/Button/Button";
import { HadrianIcon } from "@/components/HadrianIcon/HadrianIcon";
import { ThemeToggle } from "@/components/ThemeToggle/ThemeToggle";
import { UserMenu } from "@/components/UserMenu/UserMenu";
import { useConfig } from "@/config/ConfigProvider";
import { usePreferences } from "@/preferences/PreferencesProvider";
import { useAuth, hasAdminAccess } from "@/auth";
import { cn } from "@/utils/cn";

interface NavItem {
  to: string;
  icon: React.ComponentType<{ className?: string }>;
  label: string;
  matchPrefix?: string;
}

const navItems: NavItem[] = [
  { to: "/chat", icon: MessageSquare, label: "Chat" },
  { to: "/studio", icon: Palette, label: "Studio" },
  { to: "/projects", icon: FolderOpen, label: "Projects" },
  { to: "/teams", icon: UsersRound, label: "Teams" },
  { to: "/knowledge-bases", icon: BookOpen, label: "Knowledge" },
  { to: "/api-keys", icon: Key, label: "API Keys" },
  { to: "/providers", icon: Server, label: "Providers" },
  { to: "/usage", icon: BarChart3, label: "Usage" },
];

const adminNavItem: NavItem = {
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
  const location = useLocation();

  // Determine which logo to use based on theme
  const logoUrl =
    resolvedTheme === "dark" && config?.branding.logo_dark_url
      ? config.branding.logo_dark_url
      : config?.branding.logo_url;

  // Only show admin nav if admin is enabled AND user has admin access
  const showAdmin = config?.admin.enabled && hasAdminAccess(user);
  const allNavItems = showAdmin ? [...navItems, adminNavItem] : navItems;

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
        <Link to="/chat" className="flex items-center gap-2.5">
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
        className="hidden sm:flex items-center gap-1"
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
        <UserMenu />
      </div>
    </header>
  );
}
