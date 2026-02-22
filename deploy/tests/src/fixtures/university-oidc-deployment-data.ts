/**
 * University OIDC deployment data setup fixture.
 *
 * Creates the university deployment structure with per-org OIDC SSO:
 *   - 1 organization (university)
 *   - 6 teams (cs-faculty, cs-phd-students, cs-undergrad-tas, med-research, med-administration, it-platform)
 *   - OIDC SSO configuration (Keycloak)
 *   - 6 SSO group mappings
 *
 * This uses the same pattern as SAML deployment:
 *   - Bootstrap API key authentication for initial setup
 *   - SSO config created via Admin API
 *   - Gateway restart after SSO config creation
 *   - SSO group mappings for JIT user provisioning
 */

import { waitForHealthy, sleep } from "./wait-for";
import type { StartedComposeEnvironment } from "./compose";

/**
 * Bootstrap API key for initial setup (matches hadrian.university.toml).
 * This key only works when no users exist in the database.
 */
const BOOTSTRAP_API_KEY = "gw_test_bootstrap_key_for_e2e";

/**
 * Team definitions with slug and display name.
 */
export const UNIVERSITY_TEAMS = [
  { slug: "cs-faculty", name: "CS Faculty" },
  { slug: "cs-phd-students", name: "CS PhD Students" },
  { slug: "cs-undergrad-tas", name: "CS Undergraduate TAs" },
  { slug: "med-research", name: "Medical Research" },
  { slug: "med-administration", name: "Medical Administration" },
  { slug: "it-platform", name: "IT Platform" },
] as const;

/**
 * SSO group mappings for OIDC.
 * Maps Keycloak groups (paths like /cs/faculty) to gateway teams.
 */
export const OIDC_SSO_GROUP_MAPPINGS = [
  { idpGroup: "/cs/faculty", teamSlug: "cs-faculty" },
  { idpGroup: "/cs/phd-students", teamSlug: "cs-phd-students" },
  { idpGroup: "/cs/undergrad-tas", teamSlug: "cs-undergrad-tas" },
  { idpGroup: "/med/research", teamSlug: "med-research" },
  { idpGroup: "/med/administration", teamSlug: "med-administration" },
  { idpGroup: "/it/platform", teamSlug: "it-platform" },
] as const;

/**
 * Context returned by setup functions with all created IDs.
 */
export interface UniversityOidcDeploymentContext {
  /** Organization UUID */
  orgId: string;
  /** Map of team slug to team UUID */
  teamIds: Record<string, string>;
  /** Bootstrap API key used for setup */
  bootstrapKey: string;
}

/**
 * Set up university deployment data using bootstrap API key.
 * Creates organization and teams.
 *
 * @param gatewayUrl Gateway base URL
 * @returns Context with org and team IDs
 */
export async function setupUniversityOidcDeploymentData(
  gatewayUrl: string
): Promise<UniversityOidcDeploymentContext> {
  const authHeaders = { Authorization: `Bearer ${BOOTSTRAP_API_KEY}` };

  // 1. Create organization
  const orgResponse = await fetch(`${gatewayUrl}/admin/v1/organizations`, {
    method: "POST",
    headers: {
      ...authHeaders,
      "Content-Type": "application/json",
    },
    body: JSON.stringify({
      slug: "university",
      name: "State University",
    }),
  });

  if (!orgResponse.ok) {
    const error = await orgResponse.text();
    throw new Error(
      `Failed to create university organization: ${orgResponse.status} ${error}`
    );
  }

  const orgData = await orgResponse.json();
  const orgId = orgData.id;

  // 2. Create teams
  const teamIds: Record<string, string> = {};

  for (const team of UNIVERSITY_TEAMS) {
    const teamResponse = await fetch(
      `${gatewayUrl}/admin/v1/organizations/university/teams`,
      {
        method: "POST",
        headers: {
          ...authHeaders,
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          slug: team.slug,
          name: team.name,
        }),
      }
    );

    if (!teamResponse.ok) {
      const error = await teamResponse.text();
      throw new Error(
        `Failed to create team ${team.slug}: ${teamResponse.status} ${error}`
      );
    }

    const teamData = await teamResponse.json();
    teamIds[team.slug] = teamData.id;
  }

  return {
    orgId,
    teamIds,
    bootstrapKey: BOOTSTRAP_API_KEY,
  };
}

/**
 * Configure OIDC SSO for the university organization.
 *
 * @param gatewayUrl Gateway base URL
 * @param keycloakUrl Keycloak URL (external/browser accessible, e.g., http://localhost:8080)
 * @param keycloakInternalUrl Keycloak URL as seen from gateway container (e.g., http://keycloak:8080)
 */
export async function configureOidcSso(
  gatewayUrl: string,
  keycloakUrl: string,
  keycloakInternalUrl: string = "http://keycloak:8080"
): Promise<void> {
  const authHeaders = { Authorization: `Bearer ${BOOTSTRAP_API_KEY}` };

  // Wait for Keycloak OIDC discovery to be available
  const discoveryUrl = `${keycloakUrl}/realms/hadrian/.well-known/openid-configuration`;
  let discoveryReady = false;

  for (let i = 0; i < 30; i++) {
    try {
      const response = await fetch(discoveryUrl);
      if (response.ok) {
        const data = await response.json();
        if (data.issuer) {
          discoveryReady = true;
          break;
        }
      }
    } catch {
      // Continue retrying
    }
    await sleep(2000);
  }

  if (!discoveryReady) {
    console.warn(
      "Keycloak OIDC discovery not available after 60s, continuing anyway"
    );
  }

  // Build SSO config for OIDC
  // Keycloak advertises its issuer based on KC_HOSTNAME settings (localhost:8080),
  // regardless of what port we actually access it on via testcontainers.
  // We need to use the advertised issuer for JWT validation to work.
  const keycloakAdvertisedIssuer = "http://localhost:8080/realms/hadrian";

  const ssoConfig = {
    provider_type: "oidc",
    enabled: true,
    allowed_email_domains: ["university.edu"],

    // OIDC configuration
    // Use Keycloak's advertised issuer (from KC_HOSTNAME) for token validation
    issuer: keycloakAdvertisedIssuer,
    // Use internal URL for discovery (gateway can't reach localhost:8080)
    discovery_url: `${keycloakInternalUrl}/realms/hadrian`,
    client_id: "hadrian-gateway",
    client_secret: "test-secret-for-e2e",
    redirect_uri: `${gatewayUrl}/auth/callback`,

    // Claim configuration
    identity_claim: "preferred_username", // Use username as external_id
    groups_claim: "groups", // Keycloak provides groups as paths like /cs/faculty

    // Provisioning
    provisioning_enabled: true,
    create_users: true,
    sync_attributes_on_login: true,
    sync_memberships_on_login: true,
  };

  // Create SSO config
  const response = await fetch(
    `${gatewayUrl}/admin/v1/organizations/university/sso-config`,
    {
      method: "POST",
      headers: {
        ...authHeaders,
        "Content-Type": "application/json",
      },
      body: JSON.stringify(ssoConfig),
    }
  );

  if (!response.ok && response.status !== 409) {
    const error = await response.text();
    throw new Error(
      `Failed to create OIDC SSO config: ${response.status} ${error}`
    );
  }

  console.log("OIDC SSO configuration created for university organization");
}

/**
 * Configure SSO group mappings for team membership.
 *
 * @param gatewayUrl Gateway base URL
 * @param teamIds Map of team slug to team UUID
 */
export async function configureSsoGroupMappings(
  gatewayUrl: string,
  teamIds: Record<string, string>
): Promise<{ created: number; failed: number }> {
  const authHeaders = { Authorization: `Bearer ${BOOTSTRAP_API_KEY}` };
  let created = 0;
  let failed = 0;

  for (const mapping of OIDC_SSO_GROUP_MAPPINGS) {
    const teamId = teamIds[mapping.teamSlug];
    if (!teamId) {
      console.warn(`Team ${mapping.teamSlug} not found for SSO group mapping`);
      failed++;
      continue;
    }

    try {
      const response = await fetch(
        `${gatewayUrl}/admin/v1/organizations/university/sso-group-mappings`,
        {
          method: "POST",
          headers: {
            ...authHeaders,
            "Content-Type": "application/json",
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
        const error = await response.text();
        console.warn(
          `Failed to create SSO group mapping ${mapping.idpGroup}: ${response.status} ${error}`
        );
        failed++;
      }
    } catch (err) {
      console.warn(`Error creating SSO group mapping ${mapping.idpGroup}:`, err);
      failed++;
    }
  }

  return { created, failed };
}

/**
 * Restart gateway to reload SSO configuration.
 *
 * The gateway loads SSO config at startup, so we need to restart it
 * after creating the SSO config.
 *
 * @param env Compose environment with restartService
 * @param gatewayUrl Gateway URL for health check
 */
export async function restartGatewayForOidc(
  env: StartedComposeEnvironment,
  gatewayUrl: string
): Promise<void> {
  await env.restartService("gateway");

  // Wait for gateway to be healthy again
  await sleep(2000);
  await waitForHealthy(`${gatewayUrl}/health`, {
    maxRetries: 30,
    retryInterval: 1000,
  });
}

/**
 * Complete OIDC deployment setup.
 *
 * Performs all steps:
 * 1. Setup deployment data (org, teams)
 * 2. Configure OIDC SSO
 * 3. Restart gateway
 * 4. Configure SSO group mappings
 *
 * NOTE: User creation happens via JIT provisioning when users log in via OIDC.
 * The bootstrap API key is disabled once the first user is created.
 *
 * @param env Compose environment
 * @param gatewayUrl Gateway URL (external)
 * @param keycloakUrl Keycloak URL (external)
 * @param keycloakInternalUrl Keycloak URL (internal, as seen from gateway container)
 * @returns Deployment context
 */
export async function setupCompleteOidcDeployment(
  env: StartedComposeEnvironment,
  gatewayUrl: string,
  keycloakUrl: string,
  keycloakInternalUrl: string = "http://keycloak:8080"
): Promise<UniversityOidcDeploymentContext> {
  console.log("Setting up OIDC deployment data...");

  // 1. Setup deployment data
  const context = await setupUniversityOidcDeploymentData(gatewayUrl);
  console.log(`Created organization: ${context.orgId}`);
  console.log(`Created teams: ${Object.keys(context.teamIds).join(", ")}`);

  // 2. Configure OIDC SSO
  await configureOidcSso(gatewayUrl, keycloakUrl, keycloakInternalUrl);

  // 3. Restart gateway to load SSO config
  console.log("Restarting gateway to load SSO configuration...");
  await restartGatewayForOidc(env, gatewayUrl);

  // 4. Configure SSO group mappings
  const mappingsResult = await configureSsoGroupMappings(
    gatewayUrl,
    context.teamIds
  );
  console.log(
    `SSO group mappings: ${mappingsResult.created} created, ${mappingsResult.failed} failed`
  );

  return context;
}

/**
 * Get bootstrap API key for initial setup.
 */
export function getBootstrapApiKey(): string {
  return BOOTSTRAP_API_KEY;
}
