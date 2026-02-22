/**
 * Keycloak OIDC token acquisition helpers for E2E tests.
 *
 * Uses the Resource Owner Password Credentials (ROPC) grant for user tokens
 * and client credentials grant for service accounts.
 *
 * Test credentials (from Keycloak realm-export.json):
 *   | Role        | Username     | Password      |
 *   |-------------|--------------|---------------|
 *   | super_admin | admin_super  | admin123      |
 *   | org_admin   | cs_admin     | orgadmin123   |
 *   | team_admin  | prof_smith   | teamadmin123  |
 *   | user        | phd_bob      | user123       |
 */

import { waitForHealthy } from "./wait-for";

export interface TokenResponse {
  access_token: string;
  id_token?: string;
  refresh_token?: string;
  expires_in: number;
  token_type: string;
  scope?: string;
}

export interface KeycloakConfig {
  /** Keycloak base URL (e.g., http://localhost:8080) */
  baseUrl: string;
  /** Realm name (default: hadrian) */
  realm?: string;
  /** Client ID (default: hadrian-gateway) */
  clientId?: string;
  /** Client secret (default: test-secret-for-e2e) */
  clientSecret?: string;
}

const DEFAULT_REALM = "hadrian";
const DEFAULT_CLIENT_ID = "hadrian-gateway";
const DEFAULT_CLIENT_SECRET = "test-secret-for-e2e";

/**
 * Build the token endpoint URL for a Keycloak realm.
 */
function getTokenEndpoint(config: KeycloakConfig): string {
  const realm = config.realm ?? DEFAULT_REALM;
  return `${config.baseUrl}/realms/${realm}/protocol/openid-connect/token`;
}

/**
 * Wait for Keycloak to be ready.
 * Keycloak can take a while to start, so we use longer timeouts.
 */
export async function waitForKeycloak(
  baseUrl: string,
  options?: { maxRetries?: number; retryInterval?: number }
): Promise<void> {
  await waitForHealthy(`${baseUrl}/health/ready`, {
    maxRetries: options?.maxRetries ?? 90,
    retryInterval: options?.retryInterval ?? 2000,
  });
}

/**
 * Get OIDC token using Resource Owner Password Credentials (ROPC) grant.
 * This is used for automated testing - in production, use authorization code flow.
 *
 * @param config Keycloak configuration
 * @param username User's username
 * @param password User's password
 * @returns Token response with access_token, id_token (if openid scope), and refresh_token
 *
 * @example
 * ```ts
 * const tokens = await getOidcToken(
 *   { baseUrl: "http://localhost:8080" },
 *   "admin_super",
 *   "admin123"
 * );
 * console.log(tokens.access_token);
 * ```
 */
export async function getOidcToken(
  config: KeycloakConfig,
  username: string,
  password: string
): Promise<TokenResponse> {
  const tokenEndpoint = getTokenEndpoint(config);
  const clientId = config.clientId ?? DEFAULT_CLIENT_ID;
  const clientSecret = config.clientSecret ?? DEFAULT_CLIENT_SECRET;

  const response = await fetch(tokenEndpoint, {
    method: "POST",
    headers: { "Content-Type": "application/x-www-form-urlencoded" },
    body: new URLSearchParams({
      grant_type: "password",
      client_id: clientId,
      client_secret: clientSecret,
      username,
      password,
      scope: "openid profile email",
    }),
  });

  if (!response.ok) {
    const errorBody = await response.text();
    throw new Error(
      `Token acquisition failed: ${response.status} ${response.statusText}\n${errorBody}`
    );
  }

  return response.json();
}

/**
 * Get service account token using OAuth client credentials grant.
 * This is used for programmatic admin access (service accounts, automation).
 *
 * @param config Keycloak configuration
 * @returns Token response with access_token (no id_token or refresh_token for client credentials)
 *
 * @example
 * ```ts
 * const tokens = await getServiceAccountToken({ baseUrl: "http://localhost:8080" });
 * console.log(tokens.access_token);
 * ```
 */
export async function getServiceAccountToken(
  config: KeycloakConfig
): Promise<TokenResponse> {
  const tokenEndpoint = getTokenEndpoint(config);
  const clientId = config.clientId ?? DEFAULT_CLIENT_ID;
  const clientSecret = config.clientSecret ?? DEFAULT_CLIENT_SECRET;

  const response = await fetch(tokenEndpoint, {
    method: "POST",
    headers: { "Content-Type": "application/x-www-form-urlencoded" },
    body: new URLSearchParams({
      grant_type: "client_credentials",
      client_id: clientId,
      client_secret: clientSecret,
    }),
  });

  if (!response.ok) {
    const errorBody = await response.text();
    throw new Error(
      `Service account token acquisition failed: ${response.status} ${response.statusText}\n${errorBody}`
    );
  }

  return response.json();
}

/**
 * Get OIDC discovery document for a Keycloak realm.
 *
 * @param config Keycloak configuration
 * @returns Discovery document with endpoints
 */
export async function getOidcDiscovery(
  config: KeycloakConfig
): Promise<OidcDiscovery> {
  const realm = config.realm ?? DEFAULT_REALM;
  const discoveryUrl = `${config.baseUrl}/realms/${realm}/.well-known/openid-configuration`;

  const response = await fetch(discoveryUrl);

  if (!response.ok) {
    throw new Error(
      `OIDC discovery failed: ${response.status} ${response.statusText}`
    );
  }

  return response.json();
}

export interface OidcDiscovery {
  issuer: string;
  authorization_endpoint: string;
  token_endpoint: string;
  userinfo_endpoint: string;
  jwks_uri: string;
  end_session_endpoint?: string;
  introspection_endpoint?: string;
  revocation_endpoint?: string;
  [key: string]: unknown;
}

/**
 * Test user credentials for different roles.
 * Matches the users defined in realm-export.json.
 */
export const TEST_USERS = {
  superAdmin: { username: "admin_super", password: "admin123" },
  orgAdmin: { username: "cs_admin", password: "orgadmin123" },
  teamAdmin: { username: "prof_smith", password: "teamadmin123" },
  user: { username: "phd_bob", password: "user123" },
  // Additional users for specific test scenarios
  backupAdmin: { username: "admin_backup", password: "admin123" },
  medAdmin: { username: "med_admin", password: "orgadmin123" },
  itAdmin: { username: "it_admin", password: "orgadmin123" },
  phdAlice: { username: "phd_alice", password: "teamadmin123" },
  testUser: { username: "testuser", password: "testpassword" },
} as const;

/**
 * Context for tests that need Keycloak authentication.
 * Includes pre-fetched tokens for different roles.
 */
export interface KeycloakTestContext {
  config: KeycloakConfig;
  tokens: {
    superAdmin: TokenResponse;
    orgAdmin: TokenResponse;
    teamAdmin: TokenResponse;
    user: TokenResponse;
    serviceAccount: TokenResponse;
  };
}

/**
 * Set up Keycloak test context with pre-fetched tokens for all test roles.
 * Useful for test suites that need multiple user tokens.
 *
 * @param baseUrl Keycloak base URL
 * @returns Test context with tokens for all roles
 *
 * @example
 * ```ts
 * const ctx = await setupKeycloakTestContext("http://localhost:8080");
 * // Use ctx.tokens.superAdmin.access_token for super admin requests
 * // Use ctx.tokens.user.access_token for regular user requests
 * ```
 */
export async function setupKeycloakTestContext(
  baseUrl: string
): Promise<KeycloakTestContext> {
  const config: KeycloakConfig = { baseUrl };

  // Wait for Keycloak to be ready
  await waitForKeycloak(baseUrl);

  // Fetch tokens for all test roles in parallel
  const [superAdmin, orgAdmin, teamAdmin, user, serviceAccount] =
    await Promise.all([
      getOidcToken(config, TEST_USERS.superAdmin.username, TEST_USERS.superAdmin.password),
      getOidcToken(config, TEST_USERS.orgAdmin.username, TEST_USERS.orgAdmin.password),
      getOidcToken(config, TEST_USERS.teamAdmin.username, TEST_USERS.teamAdmin.password),
      getOidcToken(config, TEST_USERS.user.username, TEST_USERS.user.password),
      getServiceAccountToken(config),
    ]);

  return {
    config,
    tokens: {
      superAdmin,
      orgAdmin,
      teamAdmin,
      user,
      serviceAccount,
    },
  };
}
