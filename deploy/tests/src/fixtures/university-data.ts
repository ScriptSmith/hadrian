/**
 * University deployment data setup fixture.
 *
 * Creates the complete university deployment structure matching the bash
 * setup_university_deployment() function. This includes:
 *   - 1 organization (university)
 *   - 6 teams (cs-faculty, cs-phd-students, cs-undergrad-tas, med-research, med-administration, it-platform)
 *   - 15 users (matching Keycloak realm users)
 *   - 3 projects (nlp-research, course-assistant, clinical-notes)
 *   - 3 API keys (org-scoped, budget-limited, user-scoped)
 *   - 6 SSO group mappings
 */
import type { Client } from "../client/client";
import {
  organizationCreate,
  teamCreate,
  userCreate,
  orgMemberAdd,
  teamMemberAdd,
  projectCreate,
  apiKeyCreate,
  ssoGroupMappingCreate,
} from "../client";

/**
 * Team definitions with slug and display name.
 */
const TEAMS = [
  { slug: "cs-faculty", name: "CS Faculty" },
  { slug: "cs-phd-students", name: "CS PhD Students" },
  { slug: "cs-undergrad-tas", name: "CS Undergraduate TAs" },
  { slug: "med-research", name: "Medical Research" },
  { slug: "med-administration", name: "Medical Administration" },
  { slug: "it-platform", name: "IT Platform" },
] as const;

/**
 * User definitions matching Keycloak realm users.
 * external_id corresponds to Keycloak username for OIDC integration.
 */
const USERS = [
  { externalId: "admin_super", email: "admin.super@university.edu", name: "Super Admin" },
  { externalId: "admin_backup", email: "admin.backup@university.edu", name: "Backup Admin" },
  { externalId: "cs_admin", email: "cs.admin@university.edu", name: "CS Administrator" },
  { externalId: "med_admin", email: "med.admin@university.edu", name: "Medical Administrator" },
  { externalId: "it_admin", email: "it.admin@university.edu", name: "IT Administrator" },
  { externalId: "prof_smith", email: "prof.smith@university.edu", name: "John Smith" },
  { externalId: "prof_jones", email: "prof.jones@university.edu", name: "Sarah Jones" },
  { externalId: "phd_alice", email: "phd.alice@university.edu", name: "Alice Chen" },
  { externalId: "phd_bob", email: "phd.bob@university.edu", name: "Bob Martinez" },
  { externalId: "ta_dave", email: "ta.dave@university.edu", name: "Dave Wilson" },
  { externalId: "dr_wilson", email: "dr.wilson@university.edu", name: "Emily Wilson" },
  { externalId: "dr_chen", email: "dr.chen@university.edu", name: "Michael Chen" },
  { externalId: "admin_frank", email: "admin.frank@university.edu", name: "Frank Garcia" },
  { externalId: "platform_lead", email: "platform.lead@university.edu", name: "Platform Lead" },
  { externalId: "testuser", email: "test@example.com", name: "Test User" },
] as const;

/**
 * Team membership assignments.
 * Maps team slug to array of user external_ids.
 */
const TEAM_MEMBERSHIPS: Record<string, readonly string[]> = {
  "cs-faculty": ["prof_smith", "prof_jones"],
  "cs-phd-students": ["phd_alice", "phd_bob"],
  "cs-undergrad-tas": ["ta_dave"],
  "med-research": ["dr_wilson", "dr_chen"],
  "med-administration": ["admin_frank"],
  "it-platform": ["admin_super", "admin_backup", "platform_lead"],
};

/**
 * Project definitions.
 */
const PROJECTS = [
  { slug: "nlp-research", name: "NLP Research" },
  { slug: "course-assistant", name: "Course Assistant Bot" },
  { slug: "clinical-notes", name: "Clinical Notes Analysis" },
] as const;

/**
 * SSO group mappings: IdP group -> team slug.
 */
const SSO_GROUP_MAPPINGS = [
  { idpGroup: "/cs/faculty", teamSlug: "cs-faculty" },
  { idpGroup: "/cs/phd-students", teamSlug: "cs-phd-students" },
  { idpGroup: "/cs/undergrad-tas", teamSlug: "cs-undergrad-tas" },
  { idpGroup: "/med/research", teamSlug: "med-research" },
  { idpGroup: "/med/administration", teamSlug: "med-administration" },
  { idpGroup: "/it/platform", teamSlug: "it-platform" },
] as const;

/**
 * Context returned by setupUniversityData with all created IDs.
 */
export interface UniversityDataContext {
  /** Organization UUID */
  orgId: string;
  /** Map of team slug to team UUID */
  teamIds: Record<string, string>;
  /** Map of user external_id to user UUID */
  userIds: Record<string, string>;
  /** API keys created during setup */
  apiKeys: {
    /** Org-scoped API key (no budget limit) */
    org: { id: string; key: string };
    /** Budget-limited API key (1 cent daily) */
    budget: { id: string; key: string };
    /** User-scoped API key (for phd_bob) */
    user: { id: string; key: string };
  };
}

/**
 * Options for setupUniversityData to use pre-created resources.
 */
export interface SetupUniversityDataOptions {
  /** Pre-created organization ID (skip org creation) */
  orgId?: string;
  /** Pre-created team IDs (skip team creation for these) */
  teamIds?: Record<string, string>;
  /** User external_ids to skip (already created) */
  skipUsers?: string[];
  /** ID of the first user (admin_super) if already created */
  firstUserId?: string;
}

/**
 * Set up the complete university deployment data.
 *
 * @param client - Authenticated admin client (Bearer token)
 * @param options - Optional pre-created resource IDs to skip creation
 * @returns Context with all created IDs and API keys
 * @throws Error if any creation fails
 */
export async function setupUniversityData(
  client: Client,
  options?: SetupUniversityDataOptions
): Promise<UniversityDataContext> {
  // 1. Use provided org ID or create organization
  let orgId: string;
  if (options?.orgId) {
    orgId = options.orgId;
  } else {
    const orgResponse = await organizationCreate({
      client,
      body: {
        slug: "university",
        name: "State University",
      },
    });

    if (orgResponse.response.status === 201 && orgResponse.data) {
      orgId = orgResponse.data.id;
    } else {
      throw new Error(
        `Failed to create university organization: ${orgResponse.response.status}`
      );
    }
  }

  // 2. Use provided team IDs or create teams
  const teamIds: Record<string, string> = { ...(options?.teamIds || {}) };

  for (const team of TEAMS) {
    // Skip if team ID already provided
    if (teamIds[team.slug]) {
      continue;
    }

    const teamResponse = await teamCreate({
      client,
      path: { org_slug: "university" },
      body: { slug: team.slug, name: team.name },
    });

    if (teamResponse.response.status === 201 && teamResponse.data) {
      teamIds[team.slug] = teamResponse.data.id;
    } else {
      throw new Error(
        `Failed to create team ${team.slug}: ${teamResponse.response.status}`
      );
    }
  }

  // 3. Create users (skip any that are in skipUsers)
  const userIds: Record<string, string> = {};
  const skipUsersSet = new Set(options?.skipUsers || []);

  // Add pre-created user ID if provided
  if (options?.firstUserId && options?.skipUsers?.length) {
    userIds[options.skipUsers[0]] = options.firstUserId;
  }

  for (const user of USERS) {
    // Skip if user was already created
    if (skipUsersSet.has(user.externalId)) {
      continue;
    }

    const userResponse = await userCreate({
      client,
      body: {
        external_id: user.externalId,
        email: user.email,
        name: user.name,
      },
    });

    if (userResponse.response.status !== 201 || !userResponse.data) {
      throw new Error(
        `Failed to create user ${user.externalId}: ${userResponse.response.status}`
      );
    }
    userIds[user.externalId] = userResponse.data.id;
  }

  // 4. Add users to organization (skip any already added)
  for (const user of USERS) {
    // Skip if user was already added to org (via bootstrap)
    if (skipUsersSet.has(user.externalId)) {
      continue;
    }

    const addResponse = await orgMemberAdd({
      client,
      path: { org_slug: "university" },
      body: { user_id: userIds[user.externalId] },
    });

    if (addResponse.response.status !== 201) {
      throw new Error(
        `Failed to add ${user.externalId} to organization: ${addResponse.response.status}`
      );
    }
  }

  // 5. Add users to teams
  for (const [teamSlug, members] of Object.entries(TEAM_MEMBERSHIPS)) {
    for (const externalId of members) {
      const addResponse = await teamMemberAdd({
        client,
        path: { org_slug: "university", team_slug: teamSlug },
        body: { user_id: userIds[externalId], role: "member" },
      });

      if (addResponse.response.status !== 201) {
        throw new Error(
          `Failed to add ${externalId} to team ${teamSlug}: ${addResponse.response.status}`
        );
      }
    }
  }

  // 6. Create projects
  for (const project of PROJECTS) {
    const projectResponse = await projectCreate({
      client,
      path: { org_slug: "university" },
      body: { slug: project.slug, name: project.name },
    });

    if (projectResponse.response.status !== 201) {
      throw new Error(
        `Failed to create project ${project.slug}: ${projectResponse.response.status}`
      );
    }
  }

  // 7. Create API keys
  // 7a. Org-scoped API key (no budget limit)
  const orgKeyResponse = await apiKeyCreate({
    client,
    body: {
      name: "University API Key",
      owner: { type: "organization", org_id: orgId },
    },
  });

  if (orgKeyResponse.response.status !== 201 || !orgKeyResponse.data?.key) {
    throw new Error(
      `Failed to create org API key: ${orgKeyResponse.response.status}`
    );
  }

  // 7b. Budget-limited API key (1 cent daily)
  const budgetKeyResponse = await apiKeyCreate({
    client,
    body: {
      name: "Budget Test Key",
      owner: { type: "organization", org_id: orgId },
      budget_limit_cents: 1,
      budget_period: "daily",
    },
  });

  if (budgetKeyResponse.response.status !== 201 || !budgetKeyResponse.data?.key) {
    throw new Error(
      `Failed to create budget API key: ${budgetKeyResponse.response.status}`
    );
  }

  // 7c. User-scoped API key for phd_bob
  const userKeyResponse = await apiKeyCreate({
    client,
    body: {
      name: "PhD Bob Personal Key",
      owner: { type: "user", user_id: userIds.phd_bob },
    },
  });

  if (userKeyResponse.response.status !== 201 || !userKeyResponse.data?.key) {
    throw new Error(
      `Failed to create user API key: ${userKeyResponse.response.status}`
    );
  }

  // 8. Configure SSO group mappings
  // These may fail if SSO feature is not enabled - matching bash script behavior
  for (const mapping of SSO_GROUP_MAPPINGS) {
    try {
      await ssoGroupMappingCreate({
        client,
        path: { org_slug: "university" },
        body: {
          sso_connection_name: "default",
          idp_group: mapping.idpGroup,
          team_id: teamIds[mapping.teamSlug],
          role: "member",
          priority: 0,
        },
      });
    } catch {
      // SSO group mappings may fail if feature not enabled - continue silently
    }
  }

  return {
    orgId,
    teamIds,
    userIds,
    apiKeys: {
      org: { id: orgKeyResponse.data.id, key: orgKeyResponse.data.key },
      budget: { id: budgetKeyResponse.data.id, key: budgetKeyResponse.data.key },
      user: { id: userKeyResponse.data.id, key: userKeyResponse.data.key },
    },
  };
}

/**
 * Export constants for use in tests.
 */
export { TEAMS, USERS, PROJECTS, TEAM_MEMBERSHIPS, SSO_GROUP_MAPPINGS };
