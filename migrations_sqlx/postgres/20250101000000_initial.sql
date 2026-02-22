-- Initial schema for Hadrian Gateway (PostgreSQL)

-- Organizations
CREATE TABLE IF NOT EXISTS organizations (
    id UUID PRIMARY KEY NOT NULL,
    slug VARCHAR(64) NOT NULL UNIQUE,
    name VARCHAR(255) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_organizations_slug ON organizations(slug);
-- Partial index for non-deleted organizations (most queries filter by deleted_at IS NULL)
CREATE INDEX IF NOT EXISTS idx_organizations_slug_active ON organizations(slug) WHERE deleted_at IS NULL;

-- Teams (groups within organizations)
CREATE TABLE IF NOT EXISTS teams (
    id UUID PRIMARY KEY NOT NULL,
    org_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    slug VARCHAR(64) NOT NULL,
    name VARCHAR(255) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    UNIQUE(org_id, slug)
);

CREATE INDEX IF NOT EXISTS idx_teams_org_id ON teams(org_id);
CREATE INDEX IF NOT EXISTS idx_teams_slug ON teams(slug);
-- Partial indexes for non-deleted teams (most queries filter by deleted_at IS NULL)
CREATE INDEX IF NOT EXISTS idx_teams_org_active ON teams(org_id) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_teams_org_slug_active ON teams(org_id, slug) WHERE deleted_at IS NULL;

-- Projects (belong to organizations, optionally to teams)
CREATE TABLE IF NOT EXISTS projects (
    id UUID PRIMARY KEY NOT NULL,
    org_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    team_id UUID REFERENCES teams(id) ON DELETE SET NULL,
    slug VARCHAR(64) NOT NULL,
    name VARCHAR(255) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    UNIQUE(org_id, slug)
);

CREATE INDEX IF NOT EXISTS idx_projects_org_id ON projects(org_id);
CREATE INDEX IF NOT EXISTS idx_projects_slug ON projects(slug);
CREATE INDEX IF NOT EXISTS idx_projects_team_id ON projects(team_id) WHERE team_id IS NOT NULL;
-- Partial indexes for non-deleted projects (most queries filter by deleted_at IS NULL)
CREATE INDEX IF NOT EXISTS idx_projects_org_active ON projects(org_id) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_projects_org_slug_active ON projects(org_id, slug) WHERE deleted_at IS NULL;

-- Users (external identity, linked via external_id)
CREATE TABLE IF NOT EXISTS users (
    id UUID PRIMARY KEY NOT NULL,
    external_id VARCHAR(255) NOT NULL UNIQUE,
    email VARCHAR(255) UNIQUE,
    name VARCHAR(255),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_users_external_id ON users(external_id);
CREATE INDEX IF NOT EXISTS idx_users_email ON users(email);

-- Membership source type (how the membership was created)
DO $$ BEGIN
    CREATE TYPE membership_source AS ENUM ('manual', 'jit', 'scim');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

-- Organization memberships (users belong to organizations)
CREATE TABLE IF NOT EXISTS org_memberships (
    org_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role VARCHAR(32) NOT NULL DEFAULT 'member',
    -- Source of membership: manual (admin/API), jit (SSO login), scim (IdP push)
    source membership_source NOT NULL DEFAULT 'manual',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (org_id, user_id)
);

CREATE INDEX IF NOT EXISTS idx_org_members_user_id ON org_memberships(user_id);
-- Index for querying memberships by source (used by sync_memberships_on_login)
CREATE INDEX IF NOT EXISTS idx_org_members_source ON org_memberships(user_id, source);
-- Unique index enforcing single-org membership: each user can belong to at most one organization.
-- This prevents race conditions in add_to_org and provides database-level enforcement.
CREATE UNIQUE INDEX IF NOT EXISTS idx_org_memberships_single_org ON org_memberships(user_id);

-- Project memberships (users belong to projects)
CREATE TABLE IF NOT EXISTS project_memberships (
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role VARCHAR(32) NOT NULL DEFAULT 'member',
    -- Source of membership: manual (admin/API), jit (SSO login), scim (IdP push)
    source membership_source NOT NULL DEFAULT 'manual',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (project_id, user_id)
);

CREATE INDEX IF NOT EXISTS idx_project_members_user_id ON project_memberships(user_id);
-- Index for querying memberships by source
CREATE INDEX IF NOT EXISTS idx_project_members_source ON project_memberships(user_id, source);

-- Team memberships (users belong to teams)
CREATE TABLE IF NOT EXISTS team_memberships (
    team_id UUID NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role VARCHAR(32) NOT NULL DEFAULT 'member',
    -- Source of membership: manual (admin/API), jit (SSO login), scim (IdP push)
    source membership_source NOT NULL DEFAULT 'manual',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (team_id, user_id)
);

CREATE INDEX IF NOT EXISTS idx_team_members_user_id ON team_memberships(user_id);
CREATE INDEX IF NOT EXISTS idx_team_members_team_id ON team_memberships(team_id);
-- Index for querying memberships by source (used by sync_memberships_on_login)
CREATE INDEX IF NOT EXISTS idx_team_members_source ON team_memberships(user_id, source);

-- SSO Group Mappings (maps IdP groups to Hadrian teams/roles)
-- Used for JIT provisioning: when a user logs in via SSO, their IdP groups
-- are looked up in this table to determine which teams they should be added to.
-- sso_connection_name: identifies the SSO connection from config (defaults to 'default')
-- idp_group: the exact group name as it appears in the IdP's groups claim
-- Multiple mappings per IdP group are allowed (e.g., one group -> multiple teams)
CREATE TABLE IF NOT EXISTS sso_group_mappings (
    id UUID PRIMARY KEY NOT NULL,
    -- Which SSO connection this mapping applies to (from config)
    sso_connection_name VARCHAR(64) NOT NULL DEFAULT 'default',
    -- The IdP group name (exactly as it appears in the groups claim)
    idp_group VARCHAR(512) NOT NULL,
    -- Organization context (required - mappings are org-scoped)
    org_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    -- Optional: Team to add user to when they have this IdP group
    team_id UUID REFERENCES teams(id) ON DELETE CASCADE,
    -- Optional: Role to assign (within the team if team_id set, otherwise org role)
    role VARCHAR(32),
    -- Priority for role precedence (higher = wins when multiple mappings target same team)
    priority INTEGER NOT NULL DEFAULT 0,
    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    -- Unique constraint: prevent duplicate mappings (same connection + group + org + team)
    -- NULLS NOT DISTINCT ensures NULL team_id values are treated as equal for uniqueness
    UNIQUE NULLS NOT DISTINCT (sso_connection_name, idp_group, org_id, team_id)
);

-- Index for looking up mappings by SSO connection and org
CREATE INDEX IF NOT EXISTS idx_sso_group_mappings_connection_org ON sso_group_mappings(sso_connection_name, org_id);
-- Index for looking up mappings by IdP group (for resolving user's groups)
CREATE INDEX IF NOT EXISTS idx_sso_group_mappings_idp_group ON sso_group_mappings(idp_group);
-- Index for org-scoped queries
CREATE INDEX IF NOT EXISTS idx_sso_group_mappings_org_id ON sso_group_mappings(org_id);

-- Organization SSO Configurations (per-org OIDC/SAML settings)
-- Each organization can have its own IdP configuration for multi-tenant SSO.
-- When a user logs in, the system can route to the correct IdP based on email domain.
DO $$ BEGIN
    CREATE TYPE sso_provider_type AS ENUM ('oidc', 'saml');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
    CREATE TYPE sso_enforcement_mode AS ENUM ('optional', 'required', 'test');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

CREATE TABLE IF NOT EXISTS org_sso_configs (
    id UUID PRIMARY KEY NOT NULL,
    -- Organization this SSO config belongs to (one SSO config per org)
    org_id UUID NOT NULL UNIQUE REFERENCES organizations(id) ON DELETE CASCADE,
    -- Provider type: 'oidc' or 'saml'
    provider_type sso_provider_type NOT NULL DEFAULT 'oidc',

    -- ==========================================================================
    -- OIDC Configuration (used when provider_type = 'oidc')
    -- ==========================================================================
    -- OIDC issuer URL (e.g., "https://accounts.google.com")
    -- Required for OIDC, NULL for SAML
    issuer VARCHAR(512),
    -- OIDC discovery URL (defaults to issuer/.well-known/openid-configuration)
    discovery_url VARCHAR(512),
    -- OAuth2 client ID (required for OIDC, NULL for SAML)
    client_id VARCHAR(256),
    -- Client secret stored in secret manager, this is the key reference
    -- Required for OIDC, NULL for SAML
    client_secret_key VARCHAR(512),
    -- Redirect URI (optional - can use global default)
    redirect_uri VARCHAR(512),
    -- Scopes as space-separated string (e.g., 'openid email profile groups')
    scopes VARCHAR(512) NOT NULL DEFAULT 'openid email profile',
    -- Claims configuration (OIDC-specific)
    identity_claim VARCHAR(64),
    org_claim VARCHAR(64),
    groups_claim VARCHAR(64),

    -- ==========================================================================
    -- SAML 2.0 Configuration (used when provider_type = 'saml')
    -- ==========================================================================
    -- IdP metadata URL for auto-configuration (alternative to manual config)
    saml_metadata_url VARCHAR(512),
    -- IdP entity identifier (e.g., "https://idp.example.com/metadata")
    saml_idp_entity_id VARCHAR(512),
    -- IdP Single Sign-On service URL (HTTP-Redirect or HTTP-POST binding)
    saml_idp_sso_url VARCHAR(512),
    -- IdP Single Logout service URL (optional)
    saml_idp_slo_url VARCHAR(512),
    -- IdP X.509 certificate for signature validation (PEM format)
    saml_idp_certificate TEXT,
    -- Service Provider entity ID (Hadrian's identifier to the IdP)
    saml_sp_entity_id VARCHAR(512),
    -- NameID format to request (e.g., 'urn:oasis:names:tc:SAML:1.1:nameid-format:emailAddress')
    saml_name_id_format VARCHAR(256),
    -- Whether to sign AuthnRequests
    saml_sign_requests BOOLEAN NOT NULL DEFAULT FALSE,
    -- SP private key reference in secret manager (used for signing requests)
    saml_sp_private_key_ref VARCHAR(512),
    -- SP X.509 certificate for metadata (PEM format, not a secret)
    saml_sp_certificate TEXT,
    -- Whether to force re-authentication at IdP
    saml_force_authn BOOLEAN NOT NULL DEFAULT FALSE,
    -- Requested authentication context class (e.g., 'urn:oasis:names:tc:SAML:2.0:ac:classes:PasswordProtectedTransport')
    saml_authn_context_class_ref VARCHAR(256),
    -- SAML attribute name for user identity (like identity_claim for OIDC)
    saml_identity_attribute VARCHAR(256),
    -- SAML attribute name for email
    saml_email_attribute VARCHAR(256),
    -- SAML attribute name for display name
    saml_name_attribute VARCHAR(256),
    -- SAML attribute name for groups
    saml_groups_attribute VARCHAR(256),

    -- ==========================================================================
    -- JIT Provisioning (shared by OIDC and SAML)
    -- ==========================================================================
    -- JIT provisioning settings (mirrors ProvisioningConfig)
    provisioning_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    create_users BOOLEAN NOT NULL DEFAULT TRUE,
    default_team_id UUID REFERENCES teams(id) ON DELETE SET NULL,
    default_org_role VARCHAR(32) NOT NULL DEFAULT 'member',
    default_team_role VARCHAR(32) NOT NULL DEFAULT 'member',
    -- JSON array of allowed email domains (e.g., '["acme.com", "acme.io"]')
    allowed_email_domains JSONB,
    sync_attributes_on_login BOOLEAN NOT NULL DEFAULT FALSE,
    sync_memberships_on_login BOOLEAN NOT NULL DEFAULT TRUE,
    -- SSO enforcement mode: 'optional' (allow other auth), 'required' (SSO only), 'test' (shadow mode)
    enforcement_mode sso_enforcement_mode NOT NULL DEFAULT 'optional',
    -- Whether this SSO config is active
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Index for looking up SSO config by org_id (also covered by UNIQUE constraint)
CREATE INDEX IF NOT EXISTS idx_org_sso_configs_org_id ON org_sso_configs(org_id);
-- Index for enabled SSO configs (for IdP discovery)
CREATE INDEX IF NOT EXISTS idx_org_sso_configs_enabled ON org_sso_configs(enabled) WHERE enabled = TRUE;

-- Domain Verifications for SSO (verify ownership of email domains)
DO $$ BEGIN
    CREATE TYPE domain_verification_status AS ENUM ('pending', 'verified', 'failed');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

CREATE TABLE IF NOT EXISTS domain_verifications (
    id UUID PRIMARY KEY NOT NULL,
    -- SSO config this verification belongs to
    org_sso_config_id UUID NOT NULL REFERENCES org_sso_configs(id) ON DELETE CASCADE,
    -- The domain being verified (e.g., "acme.com")
    domain VARCHAR(255) NOT NULL,
    -- Random token for DNS TXT record verification
    verification_token VARCHAR(64) NOT NULL,
    -- Verification status
    status domain_verification_status NOT NULL DEFAULT 'pending',
    -- The actual DNS TXT record found during verification (for audit)
    dns_txt_record VARCHAR(512),
    -- Number of verification attempts
    verification_attempts INTEGER NOT NULL DEFAULT 0,
    -- Last verification attempt timestamp
    last_attempt_at TIMESTAMPTZ,
    -- When the domain was successfully verified
    verified_at TIMESTAMPTZ,
    -- Optional: require re-verification after this date
    expires_at TIMESTAMPTZ,
    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    -- Each domain can only be verified once per SSO config
    UNIQUE(org_sso_config_id, domain)
);

-- Index for looking up verifications by SSO config
CREATE INDEX IF NOT EXISTS idx_domain_verifications_config_id ON domain_verifications(org_sso_config_id);
-- Index for looking up verifications by domain (for discovery)
CREATE INDEX IF NOT EXISTS idx_domain_verifications_domain ON domain_verifications(domain);
-- Index for verified domains (for SSO discovery)
CREATE INDEX IF NOT EXISTS idx_domain_verifications_verified ON domain_verifications(domain, status) WHERE status = 'verified';
-- Index for config+status queries (list_verified_by_config, has_verified_domain)
CREATE INDEX IF NOT EXISTS idx_domain_verifications_config_status ON domain_verifications(org_sso_config_id, status);

-- =============================================================================
-- SCIM 2.0 Provisioning Tables
-- =============================================================================

-- Per-organization SCIM configuration
-- Enables automatic user provisioning/deprovisioning from IdPs (Okta, Azure AD, etc.)
CREATE TABLE IF NOT EXISTS org_scim_configs (
    id UUID PRIMARY KEY NOT NULL,
    -- Organization this SCIM config belongs to (one SCIM config per org)
    org_id UUID NOT NULL UNIQUE REFERENCES organizations(id) ON DELETE CASCADE,
    -- Whether SCIM provisioning is enabled
    enabled BOOLEAN NOT NULL DEFAULT true,
    -- Bearer token hash for SCIM API authentication
    token_hash VARCHAR(64) NOT NULL,
    -- Token prefix for identification (first 8 chars, like 'scim_xxxx')
    token_prefix VARCHAR(16) NOT NULL,
    -- Last time the SCIM token was used
    token_last_used_at TIMESTAMPTZ,
    -- Provisioning settings
    create_users BOOLEAN NOT NULL DEFAULT true,
    default_team_id UUID REFERENCES teams(id) ON DELETE SET NULL,
    default_org_role VARCHAR(32) NOT NULL DEFAULT 'member',
    default_team_role VARCHAR(32) NOT NULL DEFAULT 'member',
    -- Whether to sync display name from SCIM
    sync_display_name BOOLEAN NOT NULL DEFAULT true,
    -- Deprovisioning behavior: delete user entirely (false = just deactivate)
    deactivate_deletes_user BOOLEAN NOT NULL DEFAULT false,
    -- Whether to revoke all API keys when user is deactivated via SCIM
    revoke_api_keys_on_deactivate BOOLEAN NOT NULL DEFAULT true,
    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_org_scim_configs_org_id ON org_scim_configs(org_id);
CREATE INDEX IF NOT EXISTS idx_org_scim_configs_enabled ON org_scim_configs(enabled) WHERE enabled = true;
-- Index for token authentication lookups
CREATE INDEX IF NOT EXISTS idx_org_scim_configs_token_prefix ON org_scim_configs(token_prefix);

-- Map SCIM external IDs to Hadrian user IDs (per-org)
-- This allows the same user to have different SCIM IDs in different orgs
-- and tracks the SCIM-specific "active" state separately from user deletion
CREATE TABLE IF NOT EXISTS scim_user_mappings (
    id UUID PRIMARY KEY NOT NULL,
    org_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    -- SCIM external ID from IdP (e.g., Okta user ID like '00u1a2b3c4d5e6f7g8h9')
    scim_external_id VARCHAR(255) NOT NULL,
    -- Hadrian user this maps to
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    -- SCIM "active" status (separate from user existence)
    active BOOLEAN NOT NULL DEFAULT true,
    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    -- Each SCIM external ID can only map to one user per org
    UNIQUE(org_id, scim_external_id)
);

CREATE INDEX IF NOT EXISTS idx_scim_user_mappings_org_id ON scim_user_mappings(org_id);
CREATE INDEX IF NOT EXISTS idx_scim_user_mappings_user_id ON scim_user_mappings(user_id);
CREATE INDEX IF NOT EXISTS idx_scim_user_mappings_scim_external_id ON scim_user_mappings(org_id, scim_external_id);

-- Map SCIM groups to Hadrian teams (per-org)
-- When a SCIM group is pushed from the IdP, it maps to a Hadrian team
CREATE TABLE IF NOT EXISTS scim_group_mappings (
    id UUID PRIMARY KEY NOT NULL,
    org_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    -- SCIM group ID from IdP
    scim_group_id VARCHAR(255) NOT NULL,
    -- Hadrian team this maps to
    team_id UUID NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    -- Display name from SCIM (for reference)
    display_name VARCHAR(255),
    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    -- Each SCIM group can only map to one team per org
    UNIQUE(org_id, scim_group_id)
);

CREATE INDEX IF NOT EXISTS idx_scim_group_mappings_org_id ON scim_group_mappings(org_id);
CREATE INDEX IF NOT EXISTS idx_scim_group_mappings_team_id ON scim_group_mappings(team_id);
CREATE INDEX IF NOT EXISTS idx_scim_group_mappings_scim_group_id ON scim_group_mappings(org_id, scim_group_id);

-- =============================================================================
-- Per-organization RBAC policies
-- =============================================================================

-- Policy effect type
DO $$ BEGIN
    CREATE TYPE rbac_policy_effect AS ENUM ('allow', 'deny');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

-- Per-organization RBAC policies for runtime policy management
-- Organizations can define their own CEL-based authorization policies
-- effect: 'allow' or 'deny' (explicit allow/deny semantic)
-- priority: Higher priority policies are evaluated first (descending order)
CREATE TABLE IF NOT EXISTS org_rbac_policies (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    name VARCHAR(128) NOT NULL,
    description TEXT,
    -- Resource pattern (e.g., 'projects/*', 'teams/engineering/*', '*')
    resource VARCHAR(128) NOT NULL DEFAULT '*',
    -- Action pattern (e.g., 'read', 'write', 'delete', '*')
    action VARCHAR(64) NOT NULL DEFAULT '*',
    -- CEL expression for additional conditions
    condition TEXT NOT NULL,
    -- Policy effect: 'allow' or 'deny'
    effect rbac_policy_effect NOT NULL DEFAULT 'deny',
    -- Higher priority = evaluated first (descending order)
    priority INTEGER NOT NULL DEFAULT 0,
    -- Whether this policy is active
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    -- Version number (incremented on each update for optimistic locking)
    version INTEGER NOT NULL DEFAULT 1,
    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    -- Soft delete timestamp (NULL = active, set = deleted)
    deleted_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_org_rbac_policies_org_id ON org_rbac_policies(org_id);
-- Partial index for enabled policies (most queries filter by enabled = true and not deleted)
CREATE INDEX IF NOT EXISTS idx_org_rbac_policies_enabled ON org_rbac_policies(org_id, enabled) WHERE enabled = TRUE AND deleted_at IS NULL;
-- Index for priority-ordered evaluation
CREATE INDEX IF NOT EXISTS idx_org_rbac_policies_priority ON org_rbac_policies(org_id, priority DESC);
-- Partial unique index: policy names must be unique within an org among non-deleted policies
CREATE UNIQUE INDEX IF NOT EXISTS idx_org_rbac_policies_org_name_active ON org_rbac_policies(org_id, name) WHERE deleted_at IS NULL;
-- Partial index for non-deleted policies (query optimization)
CREATE INDEX IF NOT EXISTS idx_org_rbac_policies_org_active ON org_rbac_policies(org_id) WHERE deleted_at IS NULL;

-- Version history for org RBAC policies (for audit and rollback)
-- Every update to a policy creates a new version record
CREATE TABLE IF NOT EXISTS org_rbac_policy_versions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    policy_id UUID NOT NULL REFERENCES org_rbac_policies(id) ON DELETE CASCADE,
    -- Version number (matches the policy's version at time of creation)
    version INTEGER NOT NULL,
    -- Snapshot of policy fields at this version
    name VARCHAR(128) NOT NULL,
    description TEXT,
    resource VARCHAR(128) NOT NULL,
    action VARCHAR(64) NOT NULL,
    condition TEXT NOT NULL,
    effect rbac_policy_effect NOT NULL,
    priority INTEGER NOT NULL,
    enabled BOOLEAN NOT NULL,
    -- Who created this version (null if system/migration)
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    -- Reason for the change (e.g., "Updated condition to include new team")
    reason TEXT,
    -- When this version was created
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    -- Each version number must be unique per policy
    UNIQUE(policy_id, version)
);

CREATE INDEX IF NOT EXISTS idx_org_rbac_policy_versions_policy_id ON org_rbac_policy_versions(policy_id);
CREATE INDEX IF NOT EXISTS idx_org_rbac_policy_versions_created_by ON org_rbac_policy_versions(created_by);
-- Index for fetching latest version efficiently
CREATE INDEX IF NOT EXISTS idx_org_rbac_policy_versions_latest ON org_rbac_policy_versions(policy_id, version DESC);
-- Index for cleanup jobs finding old versions by creation date
CREATE INDEX IF NOT EXISTS idx_org_rbac_policy_versions_cleanup ON org_rbac_policy_versions(policy_id, created_at);

-- =============================================================================

-- API Keys (can belong to org, team, project, or user)
DO $$ BEGIN
    CREATE TYPE api_key_owner_type AS ENUM ('organization', 'team', 'project', 'user', 'service_account');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
    CREATE TYPE budget_period AS ENUM ('daily', 'monthly');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

CREATE TABLE IF NOT EXISTS api_keys (
    id UUID PRIMARY KEY NOT NULL,
    name VARCHAR(255) NOT NULL,
    key_hash VARCHAR(64) NOT NULL UNIQUE,
    key_prefix VARCHAR(16) NOT NULL,
    owner_type api_key_owner_type NOT NULL,
    owner_id UUID NOT NULL,
    budget_amount BIGINT,
    budget_period budget_period,
    revoked_at TIMESTAMPTZ,
    expires_at TIMESTAMPTZ,
    last_used_at TIMESTAMPTZ,
    -- API Key scoping fields (Phase 1 of API Key Scoping and Lifecycle)
    scopes JSONB,                 -- Permission scopes
    allowed_models JSONB,         -- Model patterns
    ip_allowlist JSONB,           -- CIDR blocks
    rate_limit_rpm INTEGER,
    rate_limit_tpm INTEGER,
    rotated_from_key_id UUID REFERENCES api_keys(id) ON DELETE SET NULL,
    rotation_grace_until TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_api_keys_key_hash ON api_keys(key_hash);
CREATE INDEX IF NOT EXISTS idx_api_keys_owner ON api_keys(owner_type, owner_id);
CREATE INDEX IF NOT EXISTS idx_api_keys_prefix ON api_keys(key_prefix);
-- Partial index for active (non-revoked) keys - used in authentication hot path
CREATE INDEX IF NOT EXISTS idx_api_keys_active ON api_keys(key_hash) WHERE revoked_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_api_keys_owner_active ON api_keys(owner_type, owner_id) WHERE revoked_at IS NULL;
-- Partial index for service account-owned API keys (used when deleting service accounts)
CREATE INDEX IF NOT EXISTS idx_api_keys_service_account_owner ON api_keys(owner_id) WHERE owner_type = 'service_account';

-- Dynamic Providers (org, team, project, or user can define custom providers)
DO $$ BEGIN
    CREATE TYPE dynamic_provider_owner_type AS ENUM ('organization', 'team', 'project', 'user');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

CREATE TABLE IF NOT EXISTS dynamic_providers (
    id UUID PRIMARY KEY NOT NULL,
    owner_type dynamic_provider_owner_type NOT NULL,
    owner_id UUID NOT NULL,
    name VARCHAR(64) NOT NULL,
    provider_type VARCHAR(64) NOT NULL,
    base_url TEXT NOT NULL DEFAULT '',
    api_key_secret_ref VARCHAR(255),
    config JSONB,
    models JSONB NOT NULL DEFAULT '[]',
    is_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(owner_type, owner_id, name)
);

CREATE INDEX IF NOT EXISTS idx_dynamic_providers_owner ON dynamic_providers(owner_type, owner_id);

-- Usage records (for tracking request usage with principal-based attribution)
CREATE TABLE IF NOT EXISTS usage_records (
    id UUID PRIMARY KEY NOT NULL,
    -- Unique request identifier for idempotency (prevents duplicate charges)
    request_id TEXT NOT NULL UNIQUE,
    -- Attribution context: nullable to support session-based users without API keys
    api_key_id UUID REFERENCES api_keys(id) ON DELETE SET NULL,
    -- Principal-based attribution fields (all nullable, no FKs to avoid feature-gated table issues)
    user_id UUID,
    org_id UUID,
    project_id UUID,
    team_id UUID,
    service_account_id UUID,
    model VARCHAR(128) NOT NULL,
    provider VARCHAR(64) NOT NULL,
    input_tokens INTEGER NOT NULL DEFAULT 0,
    output_tokens INTEGER NOT NULL DEFAULT 0,
    total_tokens INTEGER NOT NULL DEFAULT 0,
    -- Cost in microcents (1/1,000,000 of a dollar) for sub-cent precision
    cost_microcents BIGINT NOT NULL DEFAULT 0,
    http_referer TEXT,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    -- Additional request metadata
    streamed BOOLEAN NOT NULL DEFAULT FALSE,
    cached_tokens INTEGER NOT NULL DEFAULT 0,
    reasoning_tokens INTEGER NOT NULL DEFAULT 0,
    finish_reason VARCHAR(32),
    latency_ms INTEGER,
    cancelled BOOLEAN NOT NULL DEFAULT FALSE,
    status_code SMALLINT,
    pricing_source VARCHAR(20) NOT NULL DEFAULT 'none',
    image_count INTEGER,
    audio_seconds INTEGER,
    character_count INTEGER,
    provider_source VARCHAR(16)
);

-- API key indexes (partial: only index rows with api_key_id)
CREATE INDEX IF NOT EXISTS idx_usage_records_api_key_id ON usage_records(api_key_id) WHERE api_key_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_usage_records_api_key_date ON usage_records(api_key_id, recorded_at) WHERE api_key_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_usage_records_api_key_model ON usage_records(api_key_id, model) WHERE api_key_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_usage_records_api_key_date_desc ON usage_records(api_key_id, recorded_at DESC) WHERE api_key_id IS NOT NULL;
-- Scope-level indexes (partial: only index rows with the relevant scope)
CREATE INDEX IF NOT EXISTS idx_usage_records_org_date ON usage_records(org_id, recorded_at) WHERE org_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_usage_records_user_date ON usage_records(user_id, recorded_at) WHERE user_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_usage_records_project_date ON usage_records(project_id, recorded_at) WHERE project_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_usage_records_team_date ON usage_records(team_id, recorded_at) WHERE team_id IS NOT NULL;
-- General indexes
CREATE INDEX IF NOT EXISTS idx_usage_records_recorded_at ON usage_records(recorded_at);
CREATE INDEX IF NOT EXISTS idx_usage_records_model ON usage_records(model);
CREATE INDEX IF NOT EXISTS idx_usage_records_request_id ON usage_records(request_id);

-- Daily spend aggregates (materialized from usage_records periodically)
CREATE TABLE IF NOT EXISTS daily_spend (
    id UUID PRIMARY KEY NOT NULL,
    api_key_id UUID REFERENCES api_keys(id) ON DELETE SET NULL,
    -- Principal-based attribution (mirrors usage_records)
    user_id UUID,
    org_id UUID,
    project_id UUID,
    team_id UUID,
    service_account_id UUID,
    date DATE NOT NULL,
    model VARCHAR(128) NOT NULL,
    -- Total cost in microcents (1/1,000,000 of a dollar) for sub-cent precision
    total_cost_microcents BIGINT NOT NULL DEFAULT 0,
    total_tokens INTEGER NOT NULL DEFAULT 0,
    request_count INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_daily_spend_date ON daily_spend(date);
CREATE INDEX IF NOT EXISTS idx_daily_spend_api_key_date ON daily_spend(api_key_id, date) WHERE api_key_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_daily_spend_org_date ON daily_spend(org_id, date) WHERE org_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_daily_spend_user_date ON daily_spend(user_id, date) WHERE user_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_daily_spend_project_date ON daily_spend(project_id, date) WHERE project_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_daily_spend_team_date ON daily_spend(team_id, date) WHERE team_id IS NOT NULL;

-- Model pricing configuration
-- Allows users to configure pricing for models at different scopes
-- Pricing is looked up in order: user -> project -> organization -> static config -> defaults
DO $$ BEGIN
    CREATE TYPE model_pricing_owner_type AS ENUM ('organization', 'team', 'project', 'user');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
    CREATE TYPE pricing_source AS ENUM ('manual', 'provider_api', 'default');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

CREATE TABLE IF NOT EXISTS model_pricing (
    id UUID PRIMARY KEY NOT NULL,
    owner_type model_pricing_owner_type,  -- NULL for global/static pricing
    owner_id UUID,
    provider VARCHAR(64) NOT NULL,
    model VARCHAR(128) NOT NULL,
    -- All costs in microcents per 1M tokens (divide by 10000 for cents)
    input_per_1m_tokens BIGINT NOT NULL DEFAULT 0,
    output_per_1m_tokens BIGINT NOT NULL DEFAULT 0,
    per_image BIGINT,
    per_request BIGINT,
    cached_input_per_1m_tokens BIGINT,
    cache_write_per_1m_tokens BIGINT,
    reasoning_per_1m_tokens BIGINT,
    -- Per-second pricing for audio transcription/translation (microcents/sec)
    per_second BIGINT,
    -- Per-character pricing for TTS (microcents per 1M characters)
    per_1m_characters BIGINT,
    source pricing_source NOT NULL DEFAULT 'manual',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    -- Global pricing (owner_type IS NULL) is unique per provider/model
    -- Scoped pricing is unique per owner_type/owner_id/provider/model
    UNIQUE NULLS NOT DISTINCT (owner_type, owner_id, provider, model)
);

CREATE INDEX IF NOT EXISTS idx_model_pricing_owner ON model_pricing(owner_type, owner_id);
CREATE INDEX IF NOT EXISTS idx_model_pricing_provider_model ON model_pricing(provider, model);
CREATE INDEX IF NOT EXISTS idx_model_pricing_owner_provider ON model_pricing(owner_type, owner_id, provider);

-- Updated_at trigger function
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

-- Apply updated_at triggers (using IF NOT EXISTS pattern)
DO $$ BEGIN
    CREATE TRIGGER update_organizations_updated_at BEFORE UPDATE ON organizations FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
EXCEPTION WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
    CREATE TRIGGER update_teams_updated_at BEFORE UPDATE ON teams FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
EXCEPTION WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
    CREATE TRIGGER update_projects_updated_at BEFORE UPDATE ON projects FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
EXCEPTION WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
    CREATE TRIGGER update_users_updated_at BEFORE UPDATE ON users FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
EXCEPTION WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
    CREATE TRIGGER update_api_keys_updated_at BEFORE UPDATE ON api_keys FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
EXCEPTION WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
    CREATE TRIGGER update_sso_group_mappings_updated_at BEFORE UPDATE ON sso_group_mappings FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
EXCEPTION WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
    CREATE TRIGGER update_dynamic_providers_updated_at BEFORE UPDATE ON dynamic_providers FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
EXCEPTION WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
    CREATE TRIGGER update_model_pricing_updated_at BEFORE UPDATE ON model_pricing FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
EXCEPTION WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
    CREATE TRIGGER update_org_sso_configs_updated_at BEFORE UPDATE ON org_sso_configs FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
EXCEPTION WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
    CREATE TRIGGER update_domain_verifications_updated_at BEFORE UPDATE ON domain_verifications FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
EXCEPTION WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
    CREATE TRIGGER update_org_scim_configs_updated_at BEFORE UPDATE ON org_scim_configs FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
EXCEPTION WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
    CREATE TRIGGER update_scim_user_mappings_updated_at BEFORE UPDATE ON scim_user_mappings FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
EXCEPTION WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
    CREATE TRIGGER update_scim_group_mappings_updated_at BEFORE UPDATE ON scim_group_mappings FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
EXCEPTION WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
    CREATE TRIGGER update_org_rbac_policies_updated_at BEFORE UPDATE ON org_rbac_policies FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
EXCEPTION WHEN duplicate_object THEN null;
END $$;

-- Dead-letter queue for failed operations
-- Stores failed operations (e.g., usage logging) for later recovery or inspection
CREATE TABLE IF NOT EXISTS dead_letter_queue (
    id UUID PRIMARY KEY NOT NULL,
    entry_type VARCHAR(64) NOT NULL,
    payload TEXT NOT NULL,
    error TEXT NOT NULL,
    retry_count INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_retry_at TIMESTAMPTZ,
    metadata JSONB NOT NULL DEFAULT '{}'
);

CREATE INDEX IF NOT EXISTS idx_dlq_entry_type ON dead_letter_queue(entry_type);
CREATE INDEX IF NOT EXISTS idx_dlq_created_at ON dead_letter_queue(created_at);
CREATE INDEX IF NOT EXISTS idx_dlq_retry_count ON dead_letter_queue(retry_count);

-- ============================================================================
-- Conversations (for storing chat message history)
-- ============================================================================

DO $$ BEGIN
    CREATE TYPE conversation_owner_type AS ENUM ('project', 'user');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

-- pin_order: NULL = not pinned, 0-N = pinned with order (lower = higher in list)
CREATE TABLE IF NOT EXISTS conversations (
    id UUID PRIMARY KEY NOT NULL,
    owner_type conversation_owner_type NOT NULL,
    owner_id UUID NOT NULL,
    title VARCHAR(255) NOT NULL,
    models JSONB NOT NULL DEFAULT '[]',
    messages JSONB NOT NULL DEFAULT '[]',
    pin_order INTEGER,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_conversations_owner ON conversations(owner_type, owner_id);
CREATE INDEX IF NOT EXISTS idx_conversations_created_at ON conversations(created_at);
-- Partial index for non-deleted conversations (most queries filter by deleted_at IS NULL)
CREATE INDEX IF NOT EXISTS idx_conversations_owner_active ON conversations(owner_type, owner_id) WHERE deleted_at IS NULL;
-- Index for pinned conversations (for efficient pinned queries per owner)
CREATE INDEX IF NOT EXISTS idx_conversations_owner_pinned ON conversations(owner_type, owner_id, pin_order) WHERE pin_order IS NOT NULL AND deleted_at IS NULL;

DO $$ BEGIN
    CREATE TRIGGER update_conversations_updated_at BEFORE UPDATE ON conversations FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
EXCEPTION WHEN duplicate_object THEN null;
END $$;

-- ============================================================================
-- Audit Logs (for tracking admin operations)
-- ============================================================================

DO $$ BEGIN
    CREATE TYPE audit_actor_type AS ENUM ('user', 'api_key', 'system');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

CREATE TABLE IF NOT EXISTS audit_logs (
    id UUID PRIMARY KEY NOT NULL,
    -- When the action occurred
    timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    -- Who performed the action
    actor_type audit_actor_type NOT NULL,
    -- ID of the actor (user_id or api_key_id, NULL for system)
    actor_id UUID,
    -- The action performed (e.g., 'api_key.create', 'user.update')
    action VARCHAR(64) NOT NULL,
    -- Type of resource affected (e.g., 'api_key', 'user', 'organization')
    resource_type VARCHAR(64) NOT NULL,
    -- ID of the affected resource
    resource_id UUID NOT NULL,
    -- Optional organization context
    org_id UUID REFERENCES organizations(id) ON DELETE SET NULL,
    -- Optional project context
    project_id UUID REFERENCES projects(id) ON DELETE SET NULL,
    -- JSON with additional details (request info, before/after values, etc.)
    details JSONB NOT NULL DEFAULT '{}',
    -- Client IP address
    ip_address VARCHAR(45),
    -- Client user agent
    user_agent TEXT
);

CREATE INDEX IF NOT EXISTS idx_audit_logs_timestamp ON audit_logs(timestamp);
CREATE INDEX IF NOT EXISTS idx_audit_logs_actor ON audit_logs(actor_type, actor_id);
CREATE INDEX IF NOT EXISTS idx_audit_logs_action ON audit_logs(action);
CREATE INDEX IF NOT EXISTS idx_audit_logs_resource ON audit_logs(resource_type, resource_id);
CREATE INDEX IF NOT EXISTS idx_audit_logs_org_id ON audit_logs(org_id);
CREATE INDEX IF NOT EXISTS idx_audit_logs_project_id ON audit_logs(project_id);
-- Composite index for common filter pattern: action + resource_type
CREATE INDEX IF NOT EXISTS idx_audit_logs_action_resource ON audit_logs(action, resource_type);
CREATE INDEX IF NOT EXISTS idx_audit_logs_org_action_time ON audit_logs(org_id, action, timestamp DESC);

-- ============================================================================
-- Vector Stores (RAG/Vector Search)
-- ============================================================================

-- Owner type for vector_stores and files (follows existing pattern)
DO $$ BEGIN
    CREATE TYPE vector_store_owner_type AS ENUM ('organization', 'team', 'project', 'user');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

-- File purpose (OpenAI Files API compatible)
DO $$ BEGIN
    CREATE TYPE file_purpose AS ENUM ('assistants', 'batch', 'fine-tune', 'vision');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

-- File status (OpenAI Files API compatible)
DO $$ BEGIN
    CREATE TYPE file_status AS ENUM ('uploaded', 'processed', 'error');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

-- Storage backend type for files
DO $$ BEGIN
    CREATE TYPE file_storage_backend AS ENUM ('database', 'filesystem', 's3');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

-- OpenAI Files API - stores uploaded files before they're added to vector stores
CREATE TABLE IF NOT EXISTS files (
    id UUID PRIMARY KEY NOT NULL,
    -- Ownership (who can access this file)
    owner_type vector_store_owner_type NOT NULL,
    owner_id UUID NOT NULL,
    -- File metadata
    filename VARCHAR(255) NOT NULL,
    purpose file_purpose NOT NULL DEFAULT 'assistants',
    content_type VARCHAR(128),
    size_bytes BIGINT NOT NULL,
    status file_status NOT NULL DEFAULT 'uploaded',
    status_details TEXT,
    -- SHA-256 hash of file content for deduplication (64 hex characters)
    content_hash VARCHAR(64),
    -- Storage
    storage_backend file_storage_backend NOT NULL DEFAULT 'database',
    file_data BYTEA,
    storage_path TEXT,
    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_files_owner ON files(owner_type, owner_id);
CREATE INDEX IF NOT EXISTS idx_files_purpose ON files(purpose);
CREATE INDEX IF NOT EXISTS idx_files_status ON files(status);
-- Index for content hash lookups (deduplication queries)
CREATE INDEX IF NOT EXISTS idx_files_content_hash ON files(content_hash) WHERE content_hash IS NOT NULL;

-- Collection status (OpenAI VectorStore compatible)
DO $$ BEGIN
    CREATE TYPE collection_status AS ENUM ('in_progress', 'completed', 'expired');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

-- File processing status (OpenAI VectorStoreFile compatible)
DO $$ BEGIN
    CREATE TYPE collection_file_status AS ENUM ('in_progress', 'completed', 'cancelled', 'failed');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

-- Vector Stores table (vector stores for RAG)
-- Follows OpenAI VectorStore schema with multi-tenant ownership
CREATE TABLE IF NOT EXISTS vector_stores (
    id UUID PRIMARY KEY NOT NULL,
    -- Ownership (who can access this vector store)
    owner_type vector_store_owner_type NOT NULL,
    owner_id UUID NOT NULL,
    -- Vector Store metadata
    name VARCHAR(255) NOT NULL,
    description TEXT,
    status collection_status NOT NULL DEFAULT 'completed',
    -- Embedding configuration (set at creation, immutable)
    embedding_model VARCHAR(128) NOT NULL DEFAULT 'text-embedding-3-small',
    embedding_dimensions INTEGER NOT NULL DEFAULT 1536,
    -- Usage statistics
    usage_bytes BIGINT NOT NULL DEFAULT 0,
    -- File counts as JSON: {"cancelled":0, "completed":0, "failed":0, "in_progress":0, "total":0}
    file_counts JSONB NOT NULL DEFAULT '{"cancelled":0,"completed":0,"failed":0,"in_progress":0,"total":0}',
    -- Custom metadata (up to 16 key-value pairs, OpenAI-compatible)
    metadata JSONB,
    -- Expiration policy: {"anchor": "last_active_at", "days": N}
    expires_after JSONB,
    expires_at TIMESTAMPTZ,
    last_active_at TIMESTAMPTZ,
    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    -- Unique name per owner
    UNIQUE(owner_type, owner_id, name)
);

CREATE INDEX IF NOT EXISTS idx_vector_stores_owner ON vector_stores(owner_type, owner_id);
-- Partial index for non-deleted vector_stores (most queries filter by deleted_at IS NULL)
CREATE INDEX IF NOT EXISTS idx_vector_stores_owner_active ON vector_stores(owner_type, owner_id) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_vector_stores_status ON vector_stores(status);
CREATE INDEX IF NOT EXISTS idx_vector_stores_expires_at ON vector_stores(expires_at) WHERE expires_at IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_vector_stores_embedding_model ON vector_stores(embedding_model);

DO $$ BEGIN
    CREATE TRIGGER update_collections_updated_at BEFORE UPDATE ON vector_stores FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
EXCEPTION WHEN duplicate_object THEN null;
END $$;

-- Vector store files table (links files to vector stores)
-- Follows OpenAI VectorStoreFile schema
CREATE TABLE IF NOT EXISTS vector_store_files (
    id UUID PRIMARY KEY NOT NULL,
    vector_store_id UUID NOT NULL REFERENCES vector_stores(id) ON DELETE CASCADE,
    file_id UUID NOT NULL REFERENCES files(id),
    -- Processing status
    status collection_file_status NOT NULL DEFAULT 'in_progress',
    -- Processing statistics
    usage_bytes BIGINT NOT NULL DEFAULT 0,
    -- Error information (if status = failed): {"code": "string", "message": "string"}
    last_error JSONB,
    -- Chunking strategy: {"type": "auto"|"static", "static": {"max_chunk_size_tokens": N, "chunk_overlap_tokens": N}}
    chunking_strategy JSONB,
    -- Custom attributes for filtering (up to 16 key-value pairs)
    attributes JSONB,
    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    -- Soft delete timestamp (NULL = not deleted)
    deleted_at TIMESTAMPTZ,
    -- A file can only be in a vector store once (among non-deleted entries)
    UNIQUE(vector_store_id, file_id)
);

CREATE INDEX IF NOT EXISTS idx_vector_store_files_vector_store ON vector_store_files(vector_store_id);
CREATE INDEX IF NOT EXISTS idx_vector_store_files_file ON vector_store_files(file_id);
CREATE INDEX IF NOT EXISTS idx_vector_store_files_status ON vector_store_files(status);
CREATE INDEX IF NOT EXISTS idx_vector_store_files_deleted_at ON vector_store_files(deleted_at) WHERE deleted_at IS NOT NULL;

DO $$ BEGIN
    CREATE TRIGGER update_vector_store_files_updated_at BEFORE UPDATE ON vector_store_files FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
EXCEPTION WHEN duplicate_object THEN null;
END $$;

-- Note: Document chunks are stored in the vector database (pgvector or Qdrant),
-- not in the relational database. This enables efficient similarity search
-- without cross-database joins. See VectorStore trait for chunk operations.

-- ============================================================================
-- Prompts (reusable system prompt templates)
-- ============================================================================

-- Owner type for prompts (reuses vector_store_owner_type pattern)
DO $$ BEGIN
    CREATE TYPE prompt_owner_type AS ENUM ('organization', 'team', 'project', 'user');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

-- Prompts table for saving and reusing system prompts
CREATE TABLE IF NOT EXISTS prompts (
    id UUID PRIMARY KEY NOT NULL,
    -- Ownership (who can access this prompt)
    owner_type prompt_owner_type NOT NULL,
    owner_id UUID NOT NULL,
    -- Prompt metadata
    name VARCHAR(255) NOT NULL,
    description TEXT,
    -- The actual prompt content (system message template)
    content TEXT NOT NULL,
    -- Optional metadata (temperature, max_tokens, etc.)
    metadata JSONB,
    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    -- Unique name per owner
    UNIQUE(owner_type, owner_id, name)
);

CREATE INDEX IF NOT EXISTS idx_prompts_owner ON prompts(owner_type, owner_id);
-- Partial index for non-deleted prompts (most queries filter by deleted_at IS NULL)
CREATE INDEX IF NOT EXISTS idx_prompts_owner_active ON prompts(owner_type, owner_id) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_prompts_name ON prompts(name);

DO $$ BEGIN
    CREATE TRIGGER update_prompts_updated_at BEFORE UPDATE ON prompts FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
EXCEPTION WHEN duplicate_object THEN null;
END $$;

-- ============================================================================
-- Service Accounts (machine identities for API key authentication with roles)
-- ============================================================================

-- Service accounts are first-class machine identities that can own API keys
-- and carry roles for RBAC evaluation. This enables unified authorization
-- across human users and machine identities.
CREATE TABLE IF NOT EXISTS service_accounts (
    id UUID PRIMARY KEY NOT NULL,
    org_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    slug VARCHAR(64) NOT NULL,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    -- JSON array of role strings (e.g., '["admin", "developer"]')
    -- These roles flow into the RBAC Subject when authenticating via API key
    roles JSONB NOT NULL DEFAULT '[]',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    UNIQUE(org_id, slug)
);

CREATE INDEX IF NOT EXISTS idx_service_accounts_org_id ON service_accounts(org_id);
CREATE INDEX IF NOT EXISTS idx_service_accounts_slug ON service_accounts(slug);
-- Partial indexes for non-deleted service accounts (most queries filter by deleted_at IS NULL)
CREATE INDEX IF NOT EXISTS idx_service_accounts_org_active ON service_accounts(org_id) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_service_accounts_org_slug_active ON service_accounts(org_id, slug) WHERE deleted_at IS NULL;

DO $$ BEGIN
    CREATE TRIGGER update_service_accounts_updated_at BEFORE UPDATE ON service_accounts FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
EXCEPTION WHEN duplicate_object THEN null;
END $$;
