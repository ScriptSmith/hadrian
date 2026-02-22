import { Navigate, useLocation } from "react-router-dom";

import { Spinner } from "@/components/Spinner/Spinner";

import { useAuth } from "./AuthProvider";
import { hasAdminAccess } from "./types";

interface RequireAdminProps {
  children: React.ReactNode;
}

/**
 * Wrapper component that requires the user to be authenticated AND have admin access.
 * If the user is not authenticated, redirects to /login.
 * If the user is authenticated but not an admin, redirects to /chat.
 */
export function RequireAdmin({ children }: RequireAdminProps) {
  const { isAuthenticated, isLoading, user } = useAuth();
  const location = useLocation();

  if (isLoading) {
    return (
      <div className="flex min-h-screen items-center justify-center">
        <Spinner size="lg" />
      </div>
    );
  }

  if (!isAuthenticated) {
    // Redirect to login page but save the attempted location
    return <Navigate to="/login" state={{ from: location }} replace />;
  }

  if (!hasAdminAccess(user)) {
    // User is authenticated but doesn't have admin access - redirect to chat
    return <Navigate to="/chat" replace />;
  }

  return <>{children}</>;
}
