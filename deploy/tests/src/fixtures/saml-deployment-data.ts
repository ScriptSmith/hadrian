/**
 * SAML deployment data setup fixture.
 *
 * Creates the SAML deployment structure matching the bash setup_saml_deployment()
 * function. This includes:
 *   - 1 organization (university)
 *   - 6 teams (cs-faculty, cs-phd-students, cs-undergrad-tas, med-research, med-administration, it-platform)
 *   - SAML SSO configuration
 *   - 6 SSO group mappings
 *
 * Unlike the OIDC university deployment, this uses:
 *   - Bootstrap API key authentication (no OIDC tokens)
 *   - SAML SSO config created via Admin API
 *   - Gateway restart after SSO config creation
 *   - SSO group mappings for JIT user provisioning
 */

import { waitForHealthy, sleep } from "./wait-for";
import type { StartedComposeEnvironment } from "./compose";

/**
 * Bootstrap API key for initial setup (matches hadrian.saml.toml).
 * This key only works when the database is empty.
 */
const BOOTSTRAP_API_KEY = "gw_test_bootstrap_key_for_e2e";

/**
 * Team definitions with slug and display name.
 * Same as university deployment.
 */
export const SAML_TEAMS = [
  { slug: "cs-faculty", name: "CS Faculty" },
  { slug: "cs-phd-students", name: "CS PhD Students" },
  { slug: "cs-undergrad-tas", name: "CS Undergraduate TAs" },
  { slug: "med-research", name: "Medical Research" },
  { slug: "med-administration", name: "Medical Administration" },
  { slug: "it-platform", name: "IT Platform" },
] as const;

/**
 * SSO group mappings for SAML.
 * Maps Authentik groups (paths like /cs/cs-faculty) to gateway teams.
 */
export const SAML_SSO_GROUP_MAPPINGS = [
  { idpGroup: "/cs/cs-faculty", teamSlug: "cs-faculty" },
  { idpGroup: "/cs/cs-phd-students", teamSlug: "cs-phd-students" },
  { idpGroup: "/cs/cs-undergrad-tas", teamSlug: "cs-undergrad-tas" },
  { idpGroup: "/med/med-research", teamSlug: "med-research" },
  { idpGroup: "/med/med-administration", teamSlug: "med-administration" },
  { idpGroup: "/it/it-platform", teamSlug: "it-platform" },
] as const;

/**
 * Context returned by setupSamlDeploymentData with all created IDs.
 */
export interface SamlDeploymentContext {
  /** Organization UUID */
  orgId: string;
  /** Map of team slug to team UUID */
  teamIds: Record<string, string>;
  /** Bootstrap API key used for setup */
  bootstrapKey: string;
}

/**
 * Create Authentik API token via docker exec.
 *
 * Blueprints can't reliably set token keys, so we create a token with a known
 * key for test authentication via ak shell.
 *
 * @param env Compose environment with execInService
 * @returns The created token key
 */
export async function createAuthentikApiToken(
  env: StartedComposeEnvironment
): Promise<string> {
  const tokenKey = "test-bootstrap-token-for-e2e";

  // Wait for akadmin user to be ready (Authentik bootstrap)
  let userReady = false;
  for (let i = 0; i < 30; i++) {
    try {
      const checkResult = await env.execInService("authentik-server", [
        "ak",
        "shell",
        "-c",
        `
from authentik.core.models import User
try:
    User.objects.get(username='akadmin')
    print('READY')
except User.DoesNotExist:
    print('NOT_READY')
`.trim(),
      ]);

      if (checkResult.output.includes("READY")) {
        userReady = true;
        break;
      }
    } catch {
      // Continue retrying
    }
    await sleep(2000);
  }

  if (!userReady) {
    console.warn("akadmin user not ready after 60s, continuing anyway");
  }

  // Create or update the API token
  const createTokenResult = await env.execInService("authentik-server", [
    "ak",
    "shell",
    "-c",
    `
from authentik.core.models import Token, User, TokenIntents
admin = User.objects.get(username='akadmin')
token, created = Token.objects.get_or_create(
    identifier='e2e-test-token',
    defaults={
        'user': admin,
        'key': '${tokenKey}',
        'expiring': False,
        'intent': TokenIntents.INTENT_API,
    }
)
if not created:
    token.key = '${tokenKey}'
    token.save()
print('OK')
`.trim(),
  ]);

  if (!createTokenResult.output.includes("OK")) {
    throw new Error(
      `Failed to create Authentik API token: ${createTokenResult.output}`
    );
  }

  return tokenKey;
}

/**
 * Verify Authentik API is accessible with a token.
 *
 * @param authentikUrl Authentik base URL
 * @param token API token
 * @returns True if accessible
 */
export async function verifyAuthentikApiAccess(
  authentikUrl: string,
  token: string
): Promise<boolean> {
  try {
    const response = await fetch(`${authentikUrl}/api/v3/core/users/`, {
      headers: { Authorization: `Bearer ${token}` },
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
 * Update the Authentik SAML provider configuration to use the correct gateway URL.
 *
 * The blueprint configures the SAML provider with default localhost:3000 URLs,
 * but tests may use different ports. This function updates the provider config
 * to match the actual gateway URL being used.
 *
 * @param authentikUrl Authentik base URL
 * @param gatewayUrl Actual gateway URL (e.g., http://localhost:8090)
 * @param token Authentik API token
 */
export async function updateAuthentikSamlProvider(
  authentikUrl: string,
  gatewayUrl: string,
  token: string
): Promise<void> {
  // First, find the SAML provider ID
  const providersResponse = await fetch(
    `${authentikUrl}/api/v3/providers/saml/?name=hadrian-gateway`,
    {
      headers: { Authorization: `Bearer ${token}` },
    }
  );

  if (!providersResponse.ok) {
    throw new Error(
      `Failed to fetch SAML providers: ${providersResponse.status}`
    );
  }

  const providersData = await providersResponse.json();
  if (!providersData.results || providersData.results.length === 0) {
    throw new Error("SAML provider 'hadrian-gateway' not found in Authentik");
  }

  const provider = providersData.results[0];
  const providerId = provider.pk;

  // Update the provider with the correct gateway URL
  const updateResponse = await fetch(
    `${authentikUrl}/api/v3/providers/saml/${providerId}/`,
    {
      method: "PATCH",
      headers: {
        Authorization: `Bearer ${token}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify({
        acs_url: `${gatewayUrl}/auth/saml/acs`,
        audience: `${gatewayUrl}/saml`,
        default_relay_state: `${gatewayUrl}/`,
      }),
    }
  );

  if (!updateResponse.ok) {
    const error = await updateResponse.text();
    throw new Error(
      `Failed to update SAML provider: ${updateResponse.status} ${error}`
    );
  }

  console.log(`Updated Authentik SAML provider to use gateway URL: ${gatewayUrl}`);
}

/**
 * Set up SAML deployment data using bootstrap API key.
 * Creates organization and teams.
 *
 * @param gatewayUrl Gateway base URL
 * @returns Context with org and team IDs
 */
export async function setupSamlDeploymentData(
  gatewayUrl: string
): Promise<SamlDeploymentContext> {
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

  for (const team of SAML_TEAMS) {
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
 * Extract SAML IdP configuration from metadata XML.
 *
 * @param metadata SAML metadata XML string
 * @param authentikUrl Authentik URL for fallback values
 * @returns Extracted IdP configuration
 */
function extractSamlIdpConfig(
  metadata: string,
  authentikUrl: string
): {
  entityId: string;
  ssoUrl: string;
  certificate: string | null;
} {
  // Entity ID: Use Authentik server URL as the IdP entity ID
  const entityId = authentikUrl;

  // SSO URL: Extract from metadata SingleSignOnService
  let ssoUrl: string;
  const ssoMatch = metadata.match(
    /SingleSignOnService[^>]*Location="([^"]*)"/
  );
  if (ssoMatch && ssoMatch[1]) {
    // Convert internal URL to external for gateway access
    ssoUrl = ssoMatch[1].replace("authentik-server:9000", "localhost:9000");
  } else {
    ssoUrl = `${authentikUrl}/application/saml/hadrian-gateway/sso/binding/redirect/`;
  }

  // Certificate: Extract from metadata X509Certificate
  let certificate: string | null = null;
  const certMatch = metadata.match(
    /<ds:X509Certificate>([^<]*)<\/ds:X509Certificate>/
  );
  if (certMatch && certMatch[1]) {
    certificate = `-----BEGIN CERTIFICATE-----\n${certMatch[1]}\n-----END CERTIFICATE-----`;
  }

  return { entityId, ssoUrl, certificate };
}

/**
 * Configure SAML SSO for the university organization.
 *
 * Fetches SAML metadata from Authentik and creates SSO config in gateway.
 *
 * @param gatewayUrl Gateway base URL
 * @param authentikUrl Authentik URL (external/browser accessible)
 * @param authentikInternalUrl Authentik URL as seen from gateway container
 */
export async function configureSamlSso(
  gatewayUrl: string,
  authentikUrl: string,
  _authentikInternalUrl: string = "http://authentik-server:9000"
): Promise<void> {
  const authHeaders = { Authorization: `Bearer ${BOOTSTRAP_API_KEY}` };

  // Wait for SAML metadata to be available (blueprint must load first)
  let samlMetadata = "";
  const metadataUrl = `${authentikUrl}/application/saml/hadrian-gateway/metadata/`;

  for (let i = 0; i < 60; i++) {
    try {
      const response = await fetch(metadataUrl);
      if (response.ok) {
        const text = await response.text();
        if (text.includes("EntityDescriptor")) {
          samlMetadata = text;
          break;
        }
      }
    } catch {
      // Continue retrying
    }
    await sleep(2000);
  }

  if (!samlMetadata) {
    console.warn(
      "SAML IdP metadata not available after 2 minutes, using fallback values"
    );
  }

  // Extract IdP configuration from metadata
  const { entityId, ssoUrl, certificate } = extractSamlIdpConfig(
    samlMetadata,
    authentikUrl
  );

  // Build SSO config
  const ssoConfig: Record<string, unknown> = {
    provider_type: "saml",
    enabled: true,
    allowed_email_domains: ["university.edu"],
    saml_idp_entity_id: entityId,
    saml_idp_sso_url: ssoUrl,
    saml_sp_entity_id: `${gatewayUrl}/saml`,
    saml_identity_attribute: "username", // Use username as external_id (matches Keycloak tests)
    saml_email_attribute: "email",
    saml_name_attribute: "displayName",
    saml_groups_attribute: "groups",
    provisioning_enabled: true,
    create_users: true,
    sync_memberships_on_login: true,
  };

  if (certificate) {
    ssoConfig.saml_idp_certificate = certificate;
  }

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
    console.warn(`SSO config creation may have failed: ${response.status} ${error}`);
    // Don't throw - SSO is optional for basic testing
  }
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

  for (const mapping of SAML_SSO_GROUP_MAPPINGS) {
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
        failed++;
      }
    } catch {
      failed++;
    }
  }

  return { created, failed };
}

/**
 * Restart gateway to reload SAML configuration.
 *
 * The gateway loads SAML config at startup, so we need to restart it
 * after creating the SSO config.
 *
 * @param env Compose environment with restartService
 * @param gatewayUrl Gateway URL for health check
 */
export async function restartGatewayForSaml(
  env: StartedComposeEnvironment,
  gatewayUrl: string
): Promise<void> {
  await env.restartService("gateway");

  // Wait for gateway to be healthy again
  await sleep(3000);
  await waitForHealthy(`${gatewayUrl}/health`, {
    maxRetries: 30,
    retryInterval: 2000,
  });
}

/**
 * User provisioning data for SAML users.
 * Maps usernames to their org/team memberships.
 */
export interface SamlUserProvisioningData {
  externalId: string;
  email: string;
  name: string;
  teamSlug?: string;
}

/**
 * Test users with their expected provisioning data.
 * Matches the Authentik blueprint user definitions.
 */
export const SAML_USER_PROVISIONING: SamlUserProvisioningData[] = [
  {
    externalId: "admin_super",
    email: "admin.super@university.edu",
    name: "Super Admin",
    teamSlug: "it-platform",
  },
  {
    externalId: "cs_admin",
    email: "cs.admin@university.edu",
    name: "CS Administrator",
    // cs_admin is org admin, not assigned to specific team
  },
  {
    externalId: "prof_smith",
    email: "prof.smith@university.edu",
    name: "John Smith",
    teamSlug: "cs-faculty",
  },
  {
    externalId: "phd_bob",
    email: "phd.bob@university.edu",
    name: "Bob Martinez",
    teamSlug: "cs-phd-students",
  },
];

/**
 * Complete SAML deployment setup.
 *
 * Performs all steps:
 * 1. Setup deployment data (org, teams)
 * 2. Configure SAML SSO
 * 3. Restart gateway
 * 4. Configure SSO group mappings
 *
 * NOTE: User provisioning must be done AFTER SAML login using an admin session,
 * because the bootstrap API key is disabled once a user is created.
 *
 * @param env Compose environment
 * @param gatewayUrl Gateway URL
 * @param authentikUrl Authentik URL (external)
 * @returns Deployment context
 */
export async function setupCompleteSamlDeployment(
  env: StartedComposeEnvironment,
  gatewayUrl: string,
  authentikUrl: string
): Promise<SamlDeploymentContext> {
  // 1. Setup deployment data
  const context = await setupSamlDeploymentData(gatewayUrl);

  // 2. Configure SAML SSO
  await configureSamlSso(gatewayUrl, authentikUrl);

  // 3. Restart gateway to load SAML config
  await restartGatewayForSaml(env, gatewayUrl);

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
 * Provision SAML users using an authenticated admin session.
 *
 * This must be called AFTER SAML login, using a session cookie from a user
 * with super_admin role (since the bootstrap API key is disabled once users exist).
 *
 * @param gatewayUrl Gateway base URL
 * @param sessionCookie The __gw_session cookie value from a super_admin session
 * @param teamIds Map of team slug to team UUID
 */
export async function provisionSamlUsersWithSession(
  gatewayUrl: string,
  sessionCookie: string,
  teamIds: Record<string, string>
): Promise<{ provisioned: number; failed: number }> {
  const authHeaders = { Cookie: `__gw_session=${sessionCookie}` };
  let provisioned = 0;
  let failed = 0;

  for (const user of SAML_USER_PROVISIONING) {
    try {
      // Create user
      const createUserResponse = await fetch(`${gatewayUrl}/admin/v1/users`, {
        method: "POST",
        headers: {
          ...authHeaders,
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          external_id: user.externalId,
          email: user.email,
          name: user.name,
        }),
      });

      let userId: string;
      if (createUserResponse.ok) {
        const userData = await createUserResponse.json();
        userId = userData.id;
      } else if (createUserResponse.status === 409) {
        // User already exists, fetch by external_id
        const existingResponse = await fetch(
          `${gatewayUrl}/admin/v1/users?external_id=${user.externalId}`,
          { headers: authHeaders }
        );
        if (!existingResponse.ok) {
          console.warn(`Failed to fetch existing user ${user.externalId}`);
          failed++;
          continue;
        }
        const existingData = await existingResponse.json();
        const existingUser = existingData.data?.find(
          (u: { external_id: string }) => u.external_id === user.externalId
        );
        if (!existingUser) {
          console.warn(`User ${user.externalId} not found after 409`);
          failed++;
          continue;
        }
        userId = existingUser.id;
      } else {
        const error = await createUserResponse.text();
        console.warn(
          `Failed to create user ${user.externalId}: ${createUserResponse.status} ${error}`
        );
        failed++;
        continue;
      }

      // Add user to organization
      const addToOrgResponse = await fetch(
        `${gatewayUrl}/admin/v1/organizations/university/members`,
        {
          method: "POST",
          headers: {
            ...authHeaders,
            "Content-Type": "application/json",
          },
          body: JSON.stringify({
            user_id: userId,
            role: "member",
          }),
        }
      );

      if (!addToOrgResponse.ok && addToOrgResponse.status !== 409) {
        console.warn(
          `Failed to add ${user.externalId} to org: ${addToOrgResponse.status}`
        );
      }

      // Add user to team if specified
      if (user.teamSlug) {
        const teamId = teamIds[user.teamSlug];
        if (teamId) {
          const addToTeamResponse = await fetch(
            `${gatewayUrl}/admin/v1/organizations/university/teams/${user.teamSlug}/members`,
            {
              method: "POST",
              headers: {
                ...authHeaders,
                "Content-Type": "application/json",
              },
              body: JSON.stringify({
                user_id: userId,
                role: "member",
              }),
            }
          );

          if (!addToTeamResponse.ok && addToTeamResponse.status !== 409) {
            console.warn(
              `Failed to add ${user.externalId} to team ${user.teamSlug}: ${addToTeamResponse.status}`
            );
          }
        }
      }

      provisioned++;
    } catch (error) {
      console.warn(`Error provisioning user ${user.externalId}:`, error);
      failed++;
    }
  }

  return { provisioned, failed };
}
