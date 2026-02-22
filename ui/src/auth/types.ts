import type { OidcConfig as UiOidcConfig } from "@/config/types";

export type AuthMethod = "none" | "api_key" | "oidc" | "header" | "per_org_sso";

export interface User {
  id: string;
  email?: string;
  name?: string;
  roles?: string[];
}

/** Admin roles that grant access to the admin UI */
export const ADMIN_ROLES = ["super_admin", "org_admin", "team_admin"] as const;

/** Check if a user has any admin role */
export function hasAdminAccess(user: User | null): boolean {
  // In dev mode, always show admin pages for easier development
  if (import.meta.env.DEV) return true;

  if (!user?.roles) return false;
  return user.roles.some((role) => ADMIN_ROLES.includes(role as (typeof ADMIN_ROLES)[number]));
}

export interface AuthState {
  isAuthenticated: boolean;
  isLoading: boolean;
  user: User | null;
  method: AuthMethod | null;
  token: string | null;
}

// Re-export from config for convenience
export type OidcConfig = UiOidcConfig;

export interface AuthContextValue extends AuthState {
  login: (method: AuthMethod, credentials?: LoginCredentials) => Promise<void>;
  logout: () => void;
  setApiKey: (apiKey: string) => void;
}

export interface LoginCredentials {
  apiKey?: string;
  orgId?: string;
}

/** Domain verification status */
export type DomainVerificationStatus = "pending" | "verified" | "failed";

/** SSO enforcement mode */
export type SsoEnforcementMode = "optional" | "required" | "test";

/** SSO provider type */
export type SsoProviderType = "oidc" | "saml";

/** Response from the /auth/discover endpoint */
export interface DiscoveryResult {
  org_id: string;
  org_slug: string;
  org_name: string;
  /** Whether SSO is configured and the domain is verified. SSO is only available when both conditions are met. */
  has_sso: boolean;
  /** Whether SSO is required (only true if has_sso is also true). */
  sso_required: boolean;
  /**
   * The SSO enforcement mode for this organization.
   * - "optional": SSO is available but not required
   * - "required": SSO is required; non-SSO auth will be blocked
   * - "test": SSO enforcement is being tested; non-SSO auth is logged but allowed
   */
  enforcement_mode: SsoEnforcementMode;
  /**
   * The SSO provider type - determines which auth flow to use.
   * - "oidc": Use OpenID Connect flow (/auth/login)
   * - "saml": Use SAML 2.0 flow (/auth/saml/login)
   */
  provider_type: SsoProviderType;
  idp_name: string | null;
  /** Whether the email domain has been verified via DNS TXT record. */
  domain_verified: boolean;
  /** Current verification status of the domain (pending, verified, failed). */
  domain_verification_status?: DomainVerificationStatus;
  /** When the domain was successfully verified (ISO 8601 date string). */
  verified_at?: string;
}
