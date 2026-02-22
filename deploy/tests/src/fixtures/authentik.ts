/**
 * Authentik SAML fixtures for E2E tests.
 *
 * Provides utilities for waiting on Authentik readiness and test user credentials
 * for SAML browser-based authentication flows.
 *
 * Unlike Keycloak (which supports OIDC ROPC grant for direct token acquisition),
 * Authentik SAML authentication requires browser-based flows. This fixture focuses
 * on readiness checks and test user definitions. Browser automation is handled
 * separately in saml-browser.ts.
 *
 * Test credentials (from Authentik blueprint - deploy/config/authentik/blueprint.yaml):
 *   | Role        | Username     | Password      | Email                      |
 *   |-------------|--------------|---------------|----------------------------|
 *   | super_admin | admin_super  | admin123      | admin.super@university.edu |
 *   | org_admin   | cs_admin     | orgadmin123   | cs.admin@university.edu    |
 *   | team_admin  | prof_smith   | teamadmin123  | prof.smith@university.edu  |
 *   | user        | phd_bob      | user123       | phd.bob@university.edu     |
 */

import { waitForHealthy, sleep } from "./wait-for";

export interface AuthentikConfig {
  /** Authentik base URL (e.g., http://localhost:9000) */
  baseUrl: string;
  /** Bootstrap API token for admin operations (default: test-bootstrap-token-for-e2e) */
  bootstrapToken?: string;
  /** SAML application slug (default: hadrian-gateway) */
  samlAppSlug?: string;
}

const DEFAULT_BOOTSTRAP_TOKEN = "test-bootstrap-token-for-e2e";
const DEFAULT_SAML_APP_SLUG = "hadrian-gateway";

/**
 * Test user for SAML authentication.
 * Includes expected SAML assertion attributes for verification.
 */
export interface SamlTestUser {
  username: string;
  password: string;
  role: string;
  expectedEmail: string;
  expectedName: string;
  expectedGroups: string[];
}

/**
 * Test users matching deploy/config/authentik/blueprint.yaml.
 * Use these for SAML browser-based authentication tests.
 */
export const TEST_SAML_USERS: Record<string, SamlTestUser> = {
  superAdmin: {
    username: "admin_super",
    password: "admin123",
    role: "super_admin",
    expectedEmail: "admin.super@university.edu",
    expectedName: "Super Admin",
    expectedGroups: ["/it/it-platform", "/super_admin", "/user", "/premium", "/tools_enabled", "/rag_enabled"],
  },
  orgAdmin: {
    username: "cs_admin",
    password: "orgadmin123",
    role: "org_admin",
    expectedEmail: "cs.admin@university.edu",
    expectedName: "CS Administrator",
    expectedGroups: ["/cs", "/org_admin", "/user"],
  },
  teamAdmin: {
    username: "prof_smith",
    password: "teamadmin123",
    role: "team_admin",
    expectedEmail: "prof.smith@university.edu",
    expectedName: "John Smith",
    expectedGroups: ["/cs/cs-faculty", "/team_admin", "/user", "/premium", "/tools_enabled", "/rag_enabled"],
  },
  user: {
    username: "phd_bob",
    password: "user123",
    role: "user",
    expectedEmail: "phd.bob@university.edu",
    expectedName: "Bob Martinez",
    expectedGroups: ["/cs/cs-phd-students", "/user"],
  },
  // Additional users for specific test scenarios
  backupAdmin: {
    username: "admin_backup",
    password: "admin123",
    role: "super_admin",
    expectedEmail: "admin.backup@university.edu",
    expectedName: "Backup Admin",
    expectedGroups: ["/it/it-platform", "/super_admin", "/user"],
  },
  medAdmin: {
    username: "med_admin",
    password: "orgadmin123",
    role: "org_admin",
    expectedEmail: "med.admin@university.edu",
    expectedName: "Medical Administrator",
    expectedGroups: ["/med", "/org_admin", "/user"],
  },
  phdAlice: {
    username: "phd_alice",
    password: "teamadmin123",
    role: "team_admin",
    expectedEmail: "phd.alice@university.edu",
    expectedName: "Alice Chen",
    expectedGroups: ["/cs/cs-phd-students", "/team_admin", "/user"],
  },
} as const;

/**
 * Wait for Authentik to be fully ready.
 * Checks both the health endpoint and SAML metadata availability.
 *
 * Authentik can take 2-3 minutes to start and import blueprints, so this
 * uses longer timeouts than typical service health checks.
 *
 * @param config Authentik configuration
 * @param options Wait options
 */
export async function waitForAuthentik(
  config: AuthentikConfig,
  options?: { maxRetries?: number; retryInterval?: number }
): Promise<void> {
  const maxRetries = options?.maxRetries ?? 90;
  const retryInterval = options?.retryInterval ?? 2000;

  // First wait for Authentik health endpoint
  // Authentik's /-/health/ready/ returns 200 with empty body when healthy
  await waitForHealthy(`${config.baseUrl}/-/health/ready/`, {
    maxRetries,
    retryInterval,
  });

  // Then wait for SAML metadata to be available (blueprint must load)
  await waitForSamlMetadata(config, { maxRetries: 60, retryInterval: 2000 });
}

/**
 * Wait for SAML provider metadata to be available.
 * This indicates the Authentik blueprint has finished loading.
 *
 * @param config Authentik configuration
 * @param options Wait options
 */
export async function waitForSamlMetadata(
  config: AuthentikConfig,
  options?: { maxRetries?: number; retryInterval?: number }
): Promise<string> {
  const { maxRetries = 60, retryInterval = 2000 } = options ?? {};
  const samlAppSlug = config.samlAppSlug ?? DEFAULT_SAML_APP_SLUG;
  const metadataUrl = `${config.baseUrl}/application/saml/${samlAppSlug}/metadata/`;

  for (let i = 0; i < maxRetries; i++) {
    try {
      const controller = new AbortController();
      const timeoutId = setTimeout(() => controller.abort(), 5000);

      const response = await fetch(metadataUrl, {
        signal: controller.signal,
      });

      clearTimeout(timeoutId);

      if (response.ok) {
        const metadata = await response.text();
        if (metadata.includes("EntityDescriptor")) {
          return metadata;
        }
      }
    } catch {
      // Connection refused or timeout, continue retrying
    }

    await sleep(retryInterval);
  }

  throw new Error(
    `SAML metadata at ${metadataUrl} did not become available after ${maxRetries} retries. ` +
      `This usually means the Authentik blueprint failed to load.`
  );
}

/**
 * Get SAML IdP metadata from Authentik.
 * Returns the XML metadata document for the SAML provider.
 *
 * @param config Authentik configuration
 * @returns SAML metadata XML string
 */
export async function getSamlMetadata(config: AuthentikConfig): Promise<string> {
  const samlAppSlug = config.samlAppSlug ?? DEFAULT_SAML_APP_SLUG;
  const metadataUrl = `${config.baseUrl}/application/saml/${samlAppSlug}/metadata/`;

  const response = await fetch(metadataUrl);

  if (!response.ok) {
    throw new Error(
      `Failed to fetch SAML metadata: ${response.status} ${response.statusText}`
    );
  }

  return response.text();
}

/**
 * Verify Authentik API is accessible with the bootstrap token.
 * This is useful for debugging and verifying the test environment.
 *
 * @param config Authentik configuration
 * @returns True if API is accessible
 */
export async function verifyAuthentikApi(config: AuthentikConfig): Promise<boolean> {
  const token = config.bootstrapToken ?? DEFAULT_BOOTSTRAP_TOKEN;

  try {
    const response = await fetch(`${config.baseUrl}/api/v3/core/users/`, {
      headers: {
        Authorization: `Bearer ${token}`,
      },
    });

    if (!response.ok) {
      return false;
    }

    const data = await response.json();
    return "results" in data;
  } catch {
    return false;
  }
}

/**
 * Get user information from Authentik API.
 * Useful for verifying user setup from blueprint.
 *
 * @param config Authentik configuration
 * @param username Username to look up
 * @returns User data or null if not found
 */
export async function getAuthentikUser(
  config: AuthentikConfig,
  username: string
): Promise<AuthentikUser | null> {
  const token = config.bootstrapToken ?? DEFAULT_BOOTSTRAP_TOKEN;

  try {
    const response = await fetch(
      `${config.baseUrl}/api/v3/core/users/?username=${encodeURIComponent(username)}`,
      {
        headers: {
          Authorization: `Bearer ${token}`,
        },
      }
    );

    if (!response.ok) {
      return null;
    }

    const data = await response.json();
    if (data.results && data.results.length > 0) {
      return data.results[0] as AuthentikUser;
    }

    return null;
  } catch {
    return null;
  }
}

/**
 * Authentik user object from API.
 */
export interface AuthentikUser {
  pk: number;
  username: string;
  name: string;
  email: string;
  is_active: boolean;
  groups: number[];
  attributes: Record<string, unknown>;
}

/**
 * Context for tests that need Authentik SAML authentication.
 * Unlike KeycloakTestContext, this doesn't include pre-fetched tokens
 * because SAML requires browser-based authentication.
 */
export interface AuthentikTestContext {
  config: AuthentikConfig;
  /** SAML metadata XML (cached after waitForAuthentik) */
  samlMetadata: string;
  /** Test users available for SAML authentication */
  users: typeof TEST_SAML_USERS;
}

/**
 * SAML SSO configuration for an organization.
 * These values are used to create the SSO config via Admin API.
 */
export interface SamlSsoConfigInput {
  /** Authentik server URL as seen from gateway container (default: http://authentik-server:9000) */
  authentikInternalUrl?: string;
  /** Gateway URL for SP entity ID (default: http://localhost:3000) */
  gatewayUrl?: string;
  /** SAML application slug (default: hadrian-gateway) */
  samlAppSlug?: string;
  /** Email attribute in SAML assertion (default: email) */
  emailAttribute?: string;
  /** Name attribute in SAML assertion (default: displayName) */
  nameAttribute?: string;
  /** Groups attribute in SAML assertion (default: groups) */
  groupsAttribute?: string;
  /** Enable user provisioning (default: true) */
  provisioningEnabled?: boolean;
  /** Create users on first login (default: true) */
  createUsers?: boolean;
  /** Sync memberships on each login (default: true) */
  syncMembershipsOnLogin?: boolean;
  /** Allowed email domains (default: ["university.edu"]) */
  emailDomains?: string[];
}

/**
 * Create SAML SSO configuration for an organization via Admin API.
 *
 * This sets up the SAML provider configuration that allows users to
 * authenticate via Authentik. Should be called after creating the
 * organization and teams.
 *
 * @param gatewayUrl Gateway base URL
 * @param orgSlug Organization slug
 * @param authHeaders Authentication headers (Bootstrap API key or proxy auth)
 * @param ssoConfig SAML SSO configuration options
 * @returns True if created successfully, false if already exists or failed
 */
export async function setupSamlSsoConfig(
  gatewayUrl: string,
  orgSlug: string,
  authHeaders: Record<string, string>,
  ssoConfig: SamlSsoConfigInput = {}
): Promise<{ success: boolean; alreadyExists?: boolean; error?: string }> {
  const {
    authentikInternalUrl = "http://authentik-server:9000",
    gatewayUrl: spGatewayUrl = "http://localhost:3000",
    samlAppSlug = "hadrian-gateway",
    emailAttribute = "email",
    nameAttribute = "displayName",
    groupsAttribute = "groups",
    provisioningEnabled = true,
    createUsers = true,
    syncMembershipsOnLogin = true,
    emailDomains = ["university.edu"],
  } = ssoConfig;

  const ssoConfigBody = {
    provider_type: "saml",
    enabled: true,
    saml_metadata_url: `${authentikInternalUrl}/application/saml/${samlAppSlug}/metadata/`,
    saml_sp_entity_id: `${spGatewayUrl}/saml`,
    saml_email_attribute: emailAttribute,
    saml_name_attribute: nameAttribute,
    saml_groups_attribute: groupsAttribute,
    provisioning_enabled: provisioningEnabled,
    create_users: createUsers,
    sync_memberships_on_login: syncMembershipsOnLogin,
    email_domains: emailDomains,
  };

  try {
    const response = await fetch(
      `${gatewayUrl}/admin/v1/organizations/${orgSlug}/sso-config`,
      {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          ...authHeaders,
        },
        body: JSON.stringify(ssoConfigBody),
      }
    );

    if (response.ok) {
      return { success: true };
    }

    if (response.status === 409) {
      return { success: true, alreadyExists: true };
    }

    const errorBody = await response.text();
    return {
      success: false,
      error: `HTTP ${response.status}: ${errorBody}`,
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

/**
 * SSO group mappings for SAML.
 * Maps Authentik groups to gateway teams.
 */
export const SAML_GROUP_MAPPINGS = [
  { idpGroup: "/cs/cs-faculty", teamSlug: "cs-faculty" },
  { idpGroup: "/cs/cs-phd-students", teamSlug: "cs-phd-students" },
  { idpGroup: "/cs/cs-undergrad-tas", teamSlug: "cs-undergrad-tas" },
  { idpGroup: "/med/med-research", teamSlug: "med-research" },
  { idpGroup: "/med/med-administration", teamSlug: "med-administration" },
  { idpGroup: "/it/it-platform", teamSlug: "it-platform" },
] as const;

/**
 * Create SSO group mappings for SAML authentication.
 *
 * @param gatewayUrl Gateway base URL
 * @param orgSlug Organization slug
 * @param authHeaders Authentication headers
 * @param teamIds Map of team slug to team UUID
 * @returns Number of mappings created successfully
 */
export async function setupSamlGroupMappings(
  gatewayUrl: string,
  orgSlug: string,
  authHeaders: Record<string, string>,
  teamIds: Record<string, string>
): Promise<{ created: number; failed: number }> {
  let created = 0;
  let failed = 0;

  for (const mapping of SAML_GROUP_MAPPINGS) {
    const teamId = teamIds[mapping.teamSlug];
    if (!teamId) {
      failed++;
      continue;
    }

    try {
      const response = await fetch(
        `${gatewayUrl}/admin/v1/organizations/${orgSlug}/sso-group-mappings`,
        {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
            ...authHeaders,
          },
          body: JSON.stringify({
            sso_connection_name: "default",
            idp_group: mapping.idpGroup,
            team_id: teamId,
            role: "member",
            priority: 0,
          }),
        }
      );

      if (response.ok || response.status === 409) {
        created++;
      } else {
        failed++;
      }
    } catch {
      failed++;
    }
  }

  return { created, failed };
}

/**
 * Set up Authentik test context.
 * Waits for Authentik to be ready and returns context for SAML tests.
 *
 * @param baseUrl Authentik base URL
 * @returns Test context for SAML tests
 *
 * @example
 * ```ts
 * const ctx = await setupAuthentikTestContext("http://localhost:9000");
 * // Use ctx.users.superAdmin for SAML login
 * // ctx.samlMetadata contains the IdP metadata XML
 * ```
 */
export async function setupAuthentikTestContext(
  baseUrl: string
): Promise<AuthentikTestContext> {
  const config: AuthentikConfig = { baseUrl };

  // Wait for Authentik to be ready
  await waitForAuthentik(config);

  // Get SAML metadata
  const samlMetadata = await getSamlMetadata(config);

  return {
    config,
    samlMetadata,
    users: TEST_SAML_USERS,
  };
}
