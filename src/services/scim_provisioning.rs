//! SCIM 2.0 User and Group Provisioning Service
//!
//! This service orchestrates user and group provisioning and deprovisioning operations
//! from identity providers via SCIM 2.0 protocol.

use std::{collections::HashSet, sync::Arc};

use serde_json::Value;
use tracing::{debug, warn};
use uuid::Uuid;

use crate::{
    db::{DbPool, repos::ListParams},
    models::{
        AddTeamMember, CreateScimGroupMapping, CreateScimUserMapping, CreateTeam, CreateUser,
        OrgScimConfig, ScimGroupMapping, ScimUserMapping, Team, UpdateTeam, UpdateUser, User,
    },
    scim::{
        PatchError, PatchOp, PatchPath, PatchRequest, ScimEmail, ScimErrorResponse, ScimGroup,
        ScimGroupMember, ScimListParams, ScimListResponse, ScimMeta, ScimName, ScimResult,
        ScimUser,
        filter_to_sql::{ScimResourceType, filter_to_sql},
        matches_filter, parse_filter, parse_path,
    },
};

/// SCIM provisioning error types
#[derive(Debug)]
pub enum ScimProvisioningError {
    /// Database error
    Database(crate::db::DbError),
    /// User creation is disabled for this organization
    UserCreationDisabled,
    /// User with this userName already exists
    DuplicateUserName(String),
    /// User not found
    UserNotFound(String),
    /// Invalid SCIM request
    InvalidRequest(String),
    /// PATCH operation error
    PatchError(PatchError),
    /// Group with this ID already exists
    DuplicateGroupId(String),
    /// Group not found
    GroupNotFound(String),
}

impl From<crate::db::DbError> for ScimProvisioningError {
    fn from(e: crate::db::DbError) -> Self {
        ScimProvisioningError::Database(e)
    }
}

impl From<ScimProvisioningError> for ScimErrorResponse {
    fn from(e: ScimProvisioningError) -> Self {
        match e {
            ScimProvisioningError::Database(db_err) => {
                ScimErrorResponse::internal(format!("Database error: {}", db_err))
            }
            ScimProvisioningError::UserCreationDisabled => ScimErrorResponse::forbidden(
                "User creation is disabled for this organization's SCIM configuration",
            ),
            ScimProvisioningError::DuplicateUserName(username) => ScimErrorResponse::uniqueness(
                format!("User with userName '{}' already exists", username),
            ),
            ScimProvisioningError::UserNotFound(id) => {
                ScimErrorResponse::not_found(format!("User '{}' not found", id))
            }
            ScimProvisioningError::InvalidRequest(msg) => ScimErrorResponse::invalid_value(msg),
            ScimProvisioningError::PatchError(e) => match e {
                PatchError::NoTarget => ScimErrorResponse::no_target(e.to_string()),
                PatchError::Immutable(attr) => ScimErrorResponse::mutability(format!(
                    "Attribute '{}' is immutable and cannot be modified",
                    attr
                )),
                PatchError::InvalidPath(msg) => ScimErrorResponse::invalid_value(msg),
                _ => ScimErrorResponse::invalid_value(e.to_string()),
            },
            ScimProvisioningError::DuplicateGroupId(group_id) => ScimErrorResponse::uniqueness(
                format!("Group with id '{}' already exists", group_id),
            ),
            ScimProvisioningError::GroupNotFound(id) => {
                ScimErrorResponse::not_found(format!("Group '{}' not found", id))
            }
        }
    }
}

/// Result type for SCIM provisioning operations
pub type ProvisioningResult<T> = Result<T, ScimProvisioningError>;

/// SCIM User Provisioning Service
///
/// Handles user provisioning, deprovisioning, and lifecycle management
/// for SCIM 2.0 protocol operations.
#[derive(Clone)]
pub struct ScimProvisioningService {
    db: Arc<DbPool>,
}

impl ScimProvisioningService {
    pub fn new(db: Arc<DbPool>) -> Self {
        Self { db }
    }

    /// Create a new user via SCIM provisioning.
    ///
    /// This creates both a Hadrian user and a SCIM user mapping.
    pub async fn create_user(
        &self,
        org_id: Uuid,
        config: &OrgScimConfig,
        scim_user: &ScimUser,
        base_url: &str,
    ) -> ProvisioningResult<ScimUser> {
        // Check if user creation is enabled
        if !config.create_users {
            return Err(ScimProvisioningError::UserCreationDisabled);
        }

        // Validate required fields
        if scim_user.user_name.is_empty() {
            return Err(ScimProvisioningError::InvalidRequest(
                "userName is required".to_string(),
            ));
        }

        // Check for duplicate userName within the organization
        let existing = self
            .db
            .scim_user_mappings()
            .get_by_scim_external_id(org_id, &scim_user.user_name)
            .await?;

        if existing.is_some() {
            return Err(ScimProvisioningError::DuplicateUserName(
                scim_user.user_name.clone(),
            ));
        }

        // Also check by externalId if provided
        if let Some(ref external_id) = scim_user.external_id {
            let existing = self
                .db
                .scim_user_mappings()
                .get_by_scim_external_id(org_id, external_id)
                .await?;

            if existing.is_some() {
                return Err(ScimProvisioningError::DuplicateUserName(
                    external_id.clone(),
                ));
            }
        }

        // Create the Hadrian user
        let create_user = CreateUser {
            external_id: scim_user.user_name.clone(),
            email: scim_user.primary_email().map(String::from),
            name: self.extract_display_name(scim_user),
        };

        let user = self.db.users().create(create_user).await?;

        // Add user to the organization with default role
        self.db
            .users()
            .add_to_org(
                user.id,
                org_id,
                &config.default_org_role,
                crate::models::MembershipSource::Scim,
            )
            .await?;

        // Add user to default team if configured
        if let Some(team_id) = config.default_team_id {
            use crate::models::AddTeamMember;
            if let Err(e) = self
                .db
                .teams()
                .add_member(
                    team_id,
                    AddTeamMember {
                        user_id: user.id,
                        role: config.default_team_role.clone(),
                        source: crate::models::MembershipSource::Scim,
                    },
                )
                .await
            {
                warn!(
                    "Failed to add user {} to default team {}: {}",
                    user.id, team_id, e
                );
            }
        }

        // Create the SCIM user mapping
        // Use externalId if provided, otherwise use userName
        let scim_external_id = scim_user
            .external_id
            .clone()
            .unwrap_or_else(|| scim_user.user_name.clone());

        let mapping = self
            .db
            .scim_user_mappings()
            .create(
                org_id,
                CreateScimUserMapping {
                    scim_external_id,
                    user_id: user.id,
                    active: scim_user.active,
                },
            )
            .await?;

        debug!(
            user_id = %user.id,
            mapping_id = %mapping.id,
            user_name = %scim_user.user_name,
            "SCIM user created"
        );

        // Return the created user as SCIM format
        Ok(self.hadrian_to_scim(&user, &mapping, base_url))
    }

    /// Get a user by SCIM mapping ID.
    pub async fn get_user(
        &self,
        org_id: Uuid,
        mapping_id: Uuid,
        base_url: &str,
    ) -> ProvisioningResult<ScimUser> {
        let mapping = self
            .db
            .scim_user_mappings()
            .get_by_id(mapping_id)
            .await?
            .filter(|m| m.org_id == org_id)
            .ok_or_else(|| ScimProvisioningError::UserNotFound(mapping_id.to_string()))?;

        let user = self
            .db
            .users()
            .get_by_id(mapping.user_id)
            .await?
            .ok_or_else(|| ScimProvisioningError::UserNotFound(mapping_id.to_string()))?;

        Ok(self.hadrian_to_scim(&user, &mapping, base_url))
    }

    /// Update a user (full replace via PUT).
    pub async fn replace_user(
        &self,
        org_id: Uuid,
        config: &OrgScimConfig,
        mapping_id: Uuid,
        scim_user: &ScimUser,
        base_url: &str,
    ) -> ProvisioningResult<ScimUser> {
        // Get existing mapping
        let mapping = self
            .db
            .scim_user_mappings()
            .get_by_id(mapping_id)
            .await?
            .filter(|m| m.org_id == org_id)
            .ok_or_else(|| ScimProvisioningError::UserNotFound(mapping_id.to_string()))?;

        // Validate required fields
        if scim_user.user_name.is_empty() {
            return Err(ScimProvisioningError::InvalidRequest(
                "userName is required".to_string(),
            ));
        }

        // Update user attributes if sync_display_name is enabled
        if config.sync_display_name {
            let update = UpdateUser {
                email: scim_user.primary_email().map(String::from),
                name: self.extract_display_name(scim_user),
            };
            self.db.users().update(mapping.user_id, update).await?;
        }

        // Handle active status change
        if scim_user.active != mapping.active {
            self.db
                .scim_user_mappings()
                .set_active(mapping.id, scim_user.active)
                .await?;

            // Revoke API keys if deactivating and configured to do so
            if !scim_user.active && config.revoke_api_keys_on_deactivate {
                let revoked_count = self.db.api_keys().revoke_by_user(mapping.user_id).await?;
                debug!(
                    user_id = %mapping.user_id,
                    revoked_count,
                    "Revoked API keys due to SCIM deactivation"
                );
            }
        }

        // Get updated mapping
        let updated_mapping = self
            .db
            .scim_user_mappings()
            .get_by_id(mapping_id)
            .await?
            .ok_or_else(|| ScimProvisioningError::UserNotFound(mapping_id.to_string()))?;

        let user = self
            .db
            .users()
            .get_by_id(mapping.user_id)
            .await?
            .ok_or_else(|| ScimProvisioningError::UserNotFound(mapping_id.to_string()))?;

        Ok(self.hadrian_to_scim(&user, &updated_mapping, base_url))
    }

    /// Apply PATCH operations to a user.
    pub async fn patch_user(
        &self,
        org_id: Uuid,
        config: &OrgScimConfig,
        mapping_id: Uuid,
        patch_request: &PatchRequest,
        base_url: &str,
    ) -> ProvisioningResult<ScimUser> {
        // Validate patch request
        patch_request
            .validate()
            .map_err(ScimProvisioningError::PatchError)?;

        // Get existing mapping and user
        let mapping = self
            .db
            .scim_user_mappings()
            .get_by_id(mapping_id)
            .await?
            .filter(|m| m.org_id == org_id)
            .ok_or_else(|| ScimProvisioningError::UserNotFound(mapping_id.to_string()))?;

        let user = self
            .db
            .users()
            .get_by_id(mapping.user_id)
            .await?
            .ok_or_else(|| ScimProvisioningError::UserNotFound(mapping_id.to_string()))?;

        // Convert current user to JSON for patch application
        let mut scim_json = serde_json::to_value(self.hadrian_to_scim(&user, &mapping, base_url))
            .map_err(|e| ScimProvisioningError::InvalidRequest(e.to_string()))?;

        // Apply each operation
        for op in &patch_request.operations {
            self.apply_patch_op(op, &mut scim_json)?;
        }

        // Convert back to ScimUser
        let patched_user: ScimUser = serde_json::from_value(scim_json)
            .map_err(|e| ScimProvisioningError::InvalidRequest(e.to_string()))?;

        // Apply changes via replace
        self.replace_user(org_id, config, mapping_id, &patched_user, base_url)
            .await
    }

    /// Delete a user via SCIM.
    ///
    /// Behavior depends on `deactivate_deletes_user` config:
    /// - If true: Hard deletes the user
    /// - If false: Just deactivates (sets active=false)
    pub async fn delete_user(
        &self,
        org_id: Uuid,
        config: &OrgScimConfig,
        mapping_id: Uuid,
    ) -> ProvisioningResult<()> {
        // Get existing mapping
        let mapping = self
            .db
            .scim_user_mappings()
            .get_by_id(mapping_id)
            .await?
            .filter(|m| m.org_id == org_id)
            .ok_or_else(|| ScimProvisioningError::UserNotFound(mapping_id.to_string()))?;

        // Revoke API keys if configured
        if config.revoke_api_keys_on_deactivate {
            let revoked_count = self.db.api_keys().revoke_by_user(mapping.user_id).await?;
            debug!(
                user_id = %mapping.user_id,
                revoked_count,
                "Revoked API keys due to SCIM deletion"
            );
        }

        if config.deactivate_deletes_user {
            // Hard delete: Remove mapping and delete user
            self.db.scim_user_mappings().delete(mapping.id).await?;
            self.db.users().hard_delete(mapping.user_id).await?;

            debug!(
                user_id = %mapping.user_id,
                mapping_id = %mapping.id,
                "SCIM user hard deleted"
            );
        } else {
            // Soft delete: Just deactivate
            self.db
                .scim_user_mappings()
                .set_active(mapping.id, false)
                .await?;

            debug!(
                user_id = %mapping.user_id,
                mapping_id = %mapping.id,
                "SCIM user deactivated"
            );
        }

        Ok(())
    }

    /// List users with optional filter and pagination.
    ///
    /// Filtering is performed at the database level for efficiency. Supported filter
    /// attributes: `userName`, `externalId`, `active`, `displayName`, `name.formatted`,
    /// `emails.value`. Unsupported patterns (value filters like `emails[type eq "work"].value`,
    /// unknown attributes) return an error.
    pub async fn list_users(
        &self,
        org_id: Uuid,
        params: &ScimListParams,
        base_url: &str,
    ) -> ScimResult<ScimListResponse<ScimUser>> {
        // Parse filter if provided
        let filter = params
            .filter
            .as_ref()
            .map(|f| parse_filter(f))
            .transpose()
            .map_err(|e| ScimErrorResponse::invalid_filter(e.to_string()))?;

        // Convert filter to SQL. Returns error if filter uses unsupported patterns.
        let sql_filter = filter
            .as_ref()
            .map(|f| {
                filter_to_sql(f, ScimResourceType::User).ok_or_else(|| {
                    ScimErrorResponse::invalid_filter(
                        "Filter contains unsupported attributes or operators. \
                         Supported: userName, externalId, active, displayName, name.formatted, emails.value. \
                         Unsupported: value filters (e.g., emails[type eq \"work\"].value), unknown attributes.",
                    )
                })
            })
            .transpose()?;

        // Calculate pagination (SCIM uses 1-based indexing)
        let start_index = params.start_index.max(1);
        let limit = params.count.min(200) as i64; // Max 200 per RFC 7644
        let offset = (start_index - 1) as i64;

        // Query database with filtering
        let (results, total) = self
            .db
            .scim_user_mappings()
            .list_by_org_filtered(org_id, sql_filter.as_ref(), limit, offset)
            .await
            .map_err(|e| ScimErrorResponse::internal(e.to_string()))?;

        // Convert to SCIM users
        let scim_users: Vec<ScimUser> = results
            .iter()
            .map(|r| self.hadrian_to_scim(&r.user, &r.mapping, base_url))
            .collect();

        Ok(ScimListResponse::new(scim_users, total as u32, start_index))
    }

    // =========================================================================
    // Group provisioning methods
    // =========================================================================

    /// Create a new group via SCIM provisioning.
    ///
    /// This creates both a Hadrian team and a SCIM group mapping.
    pub async fn create_group(
        &self,
        org_id: Uuid,
        config: &OrgScimConfig,
        scim_group: &ScimGroup,
        base_url: &str,
    ) -> ProvisioningResult<ScimGroup> {
        // Validate required fields
        if scim_group.display_name.is_empty() {
            return Err(ScimProvisioningError::InvalidRequest(
                "displayName is required".to_string(),
            ));
        }

        // Check for duplicate group ID (by externalId if provided)
        let scim_group_id = scim_group
            .external_id
            .clone()
            .unwrap_or_else(|| scim_group.display_name.clone());

        let existing = self
            .db
            .scim_group_mappings()
            .get_by_scim_group_id(org_id, &scim_group_id)
            .await?;

        if existing.is_some() {
            return Err(ScimProvisioningError::DuplicateGroupId(scim_group_id));
        }

        // Generate a URL-safe slug from the display name
        let slug = self.generate_team_slug(&scim_group.display_name);

        // Create the Hadrian team
        let create_team = CreateTeam {
            slug,
            name: scim_group.display_name.clone(),
        };

        let team = self.db.teams().create(org_id, create_team).await?;

        // Create the SCIM group mapping
        let mapping = self
            .db
            .scim_group_mappings()
            .create(
                org_id,
                CreateScimGroupMapping {
                    scim_group_id,
                    team_id: team.id,
                    display_name: Some(scim_group.display_name.clone()),
                },
            )
            .await?;

        // Process initial members if any
        if !scim_group.members.is_empty() {
            self.sync_group_members(org_id, team.id, config, &scim_group.members)
                .await?;
        }

        debug!(
            team_id = %team.id,
            mapping_id = %mapping.id,
            display_name = %scim_group.display_name,
            "SCIM group created"
        );

        // Return the created group as SCIM format
        self.get_group(org_id, mapping.id, base_url).await
    }

    /// Get a group by SCIM mapping ID.
    pub async fn get_group(
        &self,
        org_id: Uuid,
        mapping_id: Uuid,
        base_url: &str,
    ) -> ProvisioningResult<ScimGroup> {
        let mapping = self
            .db
            .scim_group_mappings()
            .get_by_id(mapping_id)
            .await?
            .filter(|m| m.org_id == org_id)
            .ok_or_else(|| ScimProvisioningError::GroupNotFound(mapping_id.to_string()))?;

        let team = self
            .db
            .teams()
            .get_by_id(mapping.team_id)
            .await?
            .ok_or_else(|| ScimProvisioningError::GroupNotFound(mapping_id.to_string()))?;

        // Get team members and convert to SCIM format
        let members_result = self
            .db
            .teams()
            .list_members(team.id, ListParams::default())
            .await?;
        let members = self
            .team_members_to_scim(org_id, &members_result.items, base_url)
            .await?;

        Ok(self.hadrian_to_scim_group(&team, &mapping, members, base_url))
    }

    /// Update a group (full replace via PUT).
    pub async fn replace_group(
        &self,
        org_id: Uuid,
        config: &OrgScimConfig,
        mapping_id: Uuid,
        scim_group: &ScimGroup,
        base_url: &str,
    ) -> ProvisioningResult<ScimGroup> {
        // Get existing mapping
        let mapping = self
            .db
            .scim_group_mappings()
            .get_by_id(mapping_id)
            .await?
            .filter(|m| m.org_id == org_id)
            .ok_or_else(|| ScimProvisioningError::GroupNotFound(mapping_id.to_string()))?;

        // Validate required fields
        if scim_group.display_name.is_empty() {
            return Err(ScimProvisioningError::InvalidRequest(
                "displayName is required".to_string(),
            ));
        }

        // Update team name if changed
        let team = self
            .db
            .teams()
            .get_by_id(mapping.team_id)
            .await?
            .ok_or_else(|| ScimProvisioningError::GroupNotFound(mapping_id.to_string()))?;

        if team.name != scim_group.display_name {
            self.db
                .teams()
                .update(
                    mapping.team_id,
                    UpdateTeam {
                        name: Some(scim_group.display_name.clone()),
                    },
                )
                .await?;
        }

        // Sync members (full sync - match incoming list exactly)
        self.sync_group_members(org_id, mapping.team_id, config, &scim_group.members)
            .await?;

        debug!(
            team_id = %mapping.team_id,
            mapping_id = %mapping.id,
            display_name = %scim_group.display_name,
            "SCIM group replaced"
        );

        // Return the updated group
        self.get_group(org_id, mapping.id, base_url).await
    }

    /// Apply PATCH operations to a group.
    pub async fn patch_group(
        &self,
        org_id: Uuid,
        config: &OrgScimConfig,
        mapping_id: Uuid,
        patch_request: &PatchRequest,
        base_url: &str,
    ) -> ProvisioningResult<ScimGroup> {
        // Validate patch request
        patch_request
            .validate()
            .map_err(ScimProvisioningError::PatchError)?;

        // Get existing group
        let current_group = self.get_group(org_id, mapping_id, base_url).await?;

        // Convert current group to JSON for patch application
        let mut scim_json = serde_json::to_value(&current_group)
            .map_err(|e| ScimProvisioningError::InvalidRequest(e.to_string()))?;

        // Apply each operation
        for op in &patch_request.operations {
            self.apply_group_patch_op(op, &mut scim_json)?;
        }

        // Convert back to ScimGroup
        let patched_group: ScimGroup = serde_json::from_value(scim_json)
            .map_err(|e| ScimProvisioningError::InvalidRequest(e.to_string()))?;

        // Apply changes via replace
        self.replace_group(org_id, config, mapping_id, &patched_group, base_url)
            .await
    }

    /// Delete a group via SCIM.
    ///
    /// This removes the SCIM mapping but keeps the team (soft delete semantics).
    pub async fn delete_group(&self, org_id: Uuid, mapping_id: Uuid) -> ProvisioningResult<()> {
        // Get existing mapping
        let mapping = self
            .db
            .scim_group_mappings()
            .get_by_id(mapping_id)
            .await?
            .filter(|m| m.org_id == org_id)
            .ok_or_else(|| ScimProvisioningError::GroupNotFound(mapping_id.to_string()))?;

        // Delete the SCIM mapping (team is preserved)
        self.db.scim_group_mappings().delete(mapping.id).await?;

        debug!(
            team_id = %mapping.team_id,
            mapping_id = %mapping.id,
            "SCIM group mapping deleted (team preserved)"
        );

        Ok(())
    }

    /// List groups with optional filter and pagination.
    ///
    /// Filtering is performed at the database level for efficiency. Supported filter
    /// attributes: `id`, `externalId`, `displayName`. Unsupported patterns (value filters,
    /// `members` attribute, unknown attributes) return an error.
    pub async fn list_groups(
        &self,
        org_id: Uuid,
        params: &ScimListParams,
        base_url: &str,
    ) -> ScimResult<ScimListResponse<ScimGroup>> {
        // Parse filter if provided
        let filter = params
            .filter
            .as_ref()
            .map(|f| parse_filter(f))
            .transpose()
            .map_err(|e| ScimErrorResponse::invalid_filter(e.to_string()))?;

        // Convert filter to SQL. Returns error if filter uses unsupported patterns.
        let sql_filter = filter
            .as_ref()
            .map(|f| {
                filter_to_sql(f, ScimResourceType::Group).ok_or_else(|| {
                    ScimErrorResponse::invalid_filter(
                        "Filter contains unsupported attributes or operators. \
                         Supported: id, externalId, displayName. \
                         Unsupported: members attribute, value filters, unknown attributes.",
                    )
                })
            })
            .transpose()?;

        // Calculate pagination (SCIM uses 1-based indexing)
        let start_index = params.start_index.max(1);
        let limit = params.count.min(200) as i64; // Max 200 per RFC 7644
        let offset = (start_index - 1) as i64;

        // Query database with filtering
        let (results, total) = self
            .db
            .scim_group_mappings()
            .list_by_org_filtered(org_id, sql_filter.as_ref(), limit, offset)
            .await
            .map_err(|e| ScimErrorResponse::internal(e.to_string()))?;

        // Convert to SCIM groups (with members fetched per-group)
        let mut scim_groups = Vec::new();
        for result in &results {
            // Get team members for this group
            let members_result = self
                .db
                .teams()
                .list_members(result.team.id, ListParams::default())
                .await
                .map_err(|e| ScimErrorResponse::internal(e.to_string()))?;

            let members = self
                .team_members_to_scim(org_id, &members_result.items, base_url)
                .await
                .map_err(|e| ScimErrorResponse::internal(format!("{:?}", e)))?;

            let scim_group =
                self.hadrian_to_scim_group(&result.team, &result.mapping, members, base_url);
            scim_groups.push(scim_group);
        }

        Ok(ScimListResponse::new(
            scim_groups,
            total as u32,
            start_index,
        ))
    }

    // =========================================================================
    // User helper methods
    // =========================================================================

    /// Convert a Hadrian user + SCIM mapping to SCIM User format.
    fn hadrian_to_scim(&self, user: &User, mapping: &ScimUserMapping, base_url: &str) -> ScimUser {
        let mut scim_user = ScimUser::new(mapping.id.to_string(), mapping.scim_external_id.clone());

        // Set externalId to the SCIM external ID
        scim_user.external_id = Some(mapping.scim_external_id.clone());

        // Set active status from mapping
        scim_user.active = mapping.active;

        // Set name and display name
        if let Some(ref name) = user.name {
            scim_user.display_name = Some(name.clone());
            scim_user.name = Some(ScimName {
                formatted: Some(name.clone()),
                given_name: None,
                family_name: None,
                middle_name: None,
                honorific_prefix: None,
                honorific_suffix: None,
            });
        }

        // Set email
        if let Some(ref email) = user.email {
            scim_user.emails = vec![ScimEmail::work_primary(email.clone())];
        }

        // Set metadata
        scim_user.meta = Some(
            ScimMeta::user(user.created_at, user.updated_at)
                .with_location(format!("{}/Users/{}", base_url, mapping.id)),
        );

        scim_user
    }

    /// Extract display name from SCIM user.
    fn extract_display_name(&self, scim_user: &ScimUser) -> Option<String> {
        scim_user
            .display_name
            .clone()
            .or_else(|| scim_user.name.as_ref().and_then(|n| n.formatted.clone()))
            .or_else(|| {
                scim_user.name.as_ref().map(|n| {
                    let given = n.given_name.as_deref().unwrap_or("");
                    let family = n.family_name.as_deref().unwrap_or("");
                    format!("{} {}", given, family).trim().to_string()
                })
            })
            .filter(|s| !s.is_empty())
    }

    /// Apply a single PATCH operation to a JSON value.
    fn apply_patch_op(&self, op: &PatchOp, target: &mut Value) -> ProvisioningResult<()> {
        match op {
            PatchOp::Add { path, value } => {
                if let Some(path_str) = path {
                    let parsed_path =
                        parse_path(path_str).map_err(ScimProvisioningError::PatchError)?;
                    self.set_path_value(target, &parsed_path, value.clone())?;
                } else if let (Value::Object(obj), Value::Object(target_obj)) = (value, target) {
                    // No path = merge at root level
                    for (k, v) in obj {
                        target_obj.insert(k.clone(), v.clone());
                    }
                }
            }
            PatchOp::Replace { path, value } => {
                if let Some(path_str) = path {
                    let parsed_path =
                        parse_path(path_str).map_err(ScimProvisioningError::PatchError)?;
                    self.set_path_value(target, &parsed_path, value.clone())?;
                } else if let (Value::Object(obj), Value::Object(target_obj)) = (value, target) {
                    // No path = replace at root level (merge attributes)
                    for (k, v) in obj {
                        target_obj.insert(k.clone(), v.clone());
                    }
                }
            }
            PatchOp::Remove { path } => {
                let parsed_path = parse_path(path).map_err(ScimProvisioningError::PatchError)?;
                self.remove_path_value(target, &parsed_path)?;
            }
        }
        Ok(())
    }

    /// Set a value at a specific path.
    fn set_path_value(
        &self,
        target: &mut Value,
        path: &PatchPath,
        value: Value,
    ) -> ProvisioningResult<()> {
        // Check for immutable attributes
        if path.attr == "id" || path.attr == "schemas" {
            return Err(ScimProvisioningError::PatchError(PatchError::Immutable(
                path.attr.clone(),
            )));
        }

        if let Some(ref sub_attr) = path.sub_attr {
            // Nested path like "name.familyName"
            if let Value::Object(obj) = target {
                let parent = obj
                    .entry(&path.attr)
                    .or_insert_with(|| Value::Object(Default::default()));
                if let Value::Object(parent_obj) = parent {
                    parent_obj.insert(sub_attr.clone(), value);
                }
            }
        } else {
            // Simple path like "displayName"
            if let Value::Object(obj) = target {
                obj.insert(path.attr.clone(), value);
            }
        }

        Ok(())
    }

    /// Remove a value at a specific path.
    fn remove_path_value(&self, target: &mut Value, path: &PatchPath) -> ProvisioningResult<()> {
        // Check for immutable attributes
        if path.attr == "id" || path.attr == "schemas" || path.attr == "userName" {
            return Err(ScimProvisioningError::PatchError(PatchError::Immutable(
                path.attr.clone(),
            )));
        }

        if let Some(ref sub_attr) = path.sub_attr {
            // Nested path
            if let Value::Object(obj) = target
                && let Some(Value::Object(parent)) = obj.get_mut(&path.attr)
            {
                parent.remove(sub_attr);
            }
        } else if let Some(ref filter) = path.value_filter {
            // Path with filter like "emails[type eq \"work\"]"
            if let Value::Object(obj) = target
                && let Some(Value::Array(arr)) = obj.get_mut(&path.attr)
            {
                arr.retain(|item| !matches_filter(filter, item));
            }
        } else if let Value::Object(obj) = target {
            // Simple path
            obj.remove(&path.attr);
        }

        Ok(())
    }

    // =========================================================================
    // Group helper methods
    // =========================================================================

    /// Convert a Hadrian team + SCIM mapping to SCIM Group format.
    fn hadrian_to_scim_group(
        &self,
        team: &Team,
        mapping: &ScimGroupMapping,
        members: Vec<ScimGroupMember>,
        base_url: &str,
    ) -> ScimGroup {
        let mut scim_group = ScimGroup::new(mapping.id.to_string(), team.name.clone());

        // Set externalId to the SCIM group ID
        scim_group.external_id = Some(mapping.scim_group_id.clone());

        // Set members
        scim_group.members = members;

        // Set metadata
        scim_group.meta = Some(
            ScimMeta::group(team.created_at, team.updated_at)
                .with_location(format!("{}/Groups/{}", base_url, mapping.id)),
        );

        scim_group
    }

    /// Convert team members to SCIM group member format.
    ///
    /// This looks up SCIM user mappings to get the SCIM IDs for each member.
    async fn team_members_to_scim(
        &self,
        org_id: Uuid,
        team_members: &[crate::models::TeamMember],
        base_url: &str,
    ) -> ProvisioningResult<Vec<ScimGroupMember>> {
        let mut scim_members = Vec::new();

        for member in team_members {
            // Look up the SCIM user mapping for this user
            let mapping = self
                .db
                .scim_user_mappings()
                .get_by_user_id(org_id, member.user_id)
                .await?;

            if let Some(mapping) = mapping {
                scim_members.push(ScimGroupMember {
                    value: mapping.id.to_string(),
                    ref_uri: Some(format!("{}/Users/{}", base_url, mapping.id)),
                    display: member.name.clone(),
                    member_type: Some("User".to_string()),
                });
            }
            // Skip members without SCIM mappings (manually added users)
        }

        Ok(scim_members)
    }

    /// Generate a URL-safe slug from a display name.
    fn generate_team_slug(&self, display_name: &str) -> String {
        generate_team_slug(display_name)
    }
}

/// Generate a URL-safe slug from a display name.
///
/// If the display name is empty or collapses to an empty slug (e.g., "---"),
/// a UUID-based fallback slug is generated.
fn generate_team_slug(display_name: &str) -> String {
    let slug: String = display_name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        // Collapse multiple hyphens
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");

    // Fallback to UUID-based slug if result is empty
    if slug.is_empty() {
        format!("group-{}", &Uuid::new_v4().to_string()[..8])
    } else {
        slug
    }
}

impl ScimProvisioningService {
    /// Sync group members to match the SCIM member list.
    ///
    /// This is a full sync: members not in the SCIM list are removed,
    /// and members in the SCIM list but not in the team are added.
    async fn sync_group_members(
        &self,
        org_id: Uuid,
        team_id: Uuid,
        config: &OrgScimConfig,
        scim_members: &[ScimGroupMember],
    ) -> ProvisioningResult<()> {
        // Get current team members
        let current_members = self
            .db
            .teams()
            .list_members(team_id, ListParams::default())
            .await?;

        let current_user_ids: HashSet<Uuid> =
            current_members.items.iter().map(|m| m.user_id).collect();

        // Resolve SCIM members to user IDs
        let mut target_user_ids: HashSet<Uuid> = HashSet::new();
        for scim_member in scim_members {
            // Try to parse member.value as a UUID (mapping ID)
            if let Ok(mapping_id) = scim_member.value.parse::<Uuid>() {
                let mapping = self.db.scim_user_mappings().get_by_id(mapping_id).await?;
                if let Some(m) = mapping
                    && m.org_id == org_id
                {
                    target_user_ids.insert(m.user_id);
                }
            } else {
                // Try as external SCIM ID
                let mapping = self
                    .db
                    .scim_user_mappings()
                    .get_by_scim_external_id(org_id, &scim_member.value)
                    .await?;
                if let Some(m) = mapping {
                    target_user_ids.insert(m.user_id);
                }
            }
        }

        // Remove members not in target list
        for user_id in current_user_ids.difference(&target_user_ids) {
            if let Err(e) = self.db.teams().remove_member(team_id, *user_id).await {
                warn!(
                    team_id = %team_id,
                    user_id = %user_id,
                    error = %e,
                    "Failed to remove member from team during SCIM sync"
                );
            }
        }

        // Add members not in current list
        for user_id in target_user_ids.difference(&current_user_ids) {
            if let Err(e) = self
                .db
                .teams()
                .add_member(
                    team_id,
                    AddTeamMember {
                        user_id: *user_id,
                        role: config.default_team_role.clone(),
                        source: crate::models::MembershipSource::Scim,
                    },
                )
                .await
            {
                warn!(
                    team_id = %team_id,
                    user_id = %user_id,
                    error = %e,
                    "Failed to add member to team during SCIM sync"
                );
            }
        }

        debug!(
            team_id = %team_id,
            members_added = target_user_ids.difference(&current_user_ids).count(),
            members_removed = current_user_ids.difference(&target_user_ids).count(),
            "SCIM group membership synced"
        );

        Ok(())
    }

    /// Apply a single PATCH operation to a group JSON value.
    fn apply_group_patch_op(&self, op: &PatchOp, target: &mut Value) -> ProvisioningResult<()> {
        match op {
            PatchOp::Add { path, value } => {
                if let Some(path_str) = path {
                    // Special handling for "members" path
                    if path_str == "members" || path_str.starts_with("members[") {
                        self.add_members_patch(target, value)?;
                    } else {
                        let parsed_path =
                            parse_path(path_str).map_err(ScimProvisioningError::PatchError)?;
                        self.set_path_value(target, &parsed_path, value.clone())?;
                    }
                } else if let (Value::Object(obj), Value::Object(target_obj)) = (value, target) {
                    for (k, v) in obj {
                        target_obj.insert(k.clone(), v.clone());
                    }
                }
            }
            PatchOp::Replace { path, value } => {
                if let Some(path_str) = path {
                    let parsed_path =
                        parse_path(path_str).map_err(ScimProvisioningError::PatchError)?;
                    self.set_path_value(target, &parsed_path, value.clone())?;
                } else if let (Value::Object(obj), Value::Object(target_obj)) = (value, target) {
                    for (k, v) in obj {
                        target_obj.insert(k.clone(), v.clone());
                    }
                }
            }
            PatchOp::Remove { path } => {
                // Special handling for members removal with filter
                if path.starts_with("members[") {
                    self.remove_members_patch(target, path)?;
                } else {
                    let parsed_path =
                        parse_path(path).map_err(ScimProvisioningError::PatchError)?;
                    self.remove_path_value(target, &parsed_path)?;
                }
            }
        }
        Ok(())
    }

    /// Add members via PATCH operation.
    fn add_members_patch(&self, target: &mut Value, value: &Value) -> ProvisioningResult<()> {
        if let Value::Object(obj) = target {
            let members = obj
                .entry("members")
                .or_insert_with(|| Value::Array(Vec::new()));
            if let Value::Array(arr) = members {
                // Value can be a single member or array of members
                match value {
                    Value::Array(new_members) => {
                        for m in new_members {
                            // Avoid duplicates by value
                            if let Value::Object(member_obj) = m
                                && let Some(Value::String(member_value)) = member_obj.get("value")
                            {
                                let already_exists = arr.iter().any(|existing| {
                                    if let Value::Object(existing_obj) = existing
                                        && let Some(Value::String(existing_value)) =
                                            existing_obj.get("value")
                                    {
                                        existing_value == member_value
                                    } else {
                                        false
                                    }
                                });
                                if !already_exists {
                                    arr.push(m.clone());
                                }
                            } else {
                                arr.push(m.clone());
                            }
                        }
                    }
                    Value::Object(_) => {
                        arr.push(value.clone());
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }

    /// Remove members via PATCH operation with filter.
    fn remove_members_patch(&self, target: &mut Value, path: &str) -> ProvisioningResult<()> {
        // Parse the filter from path like "members[value eq \"user-id\"]"
        let parsed_path = parse_path(path).map_err(ScimProvisioningError::PatchError)?;

        if let Value::Object(obj) = target
            && let Some(Value::Array(arr)) = obj.get_mut("members")
            && let Some(ref filter) = parsed_path.value_filter
        {
            arr.retain(|item| !matches_filter(filter, item));
        }
        Ok(())
    }
}

impl From<PatchError> for ScimProvisioningError {
    fn from(e: PatchError) -> Self {
        ScimProvisioningError::PatchError(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_conversion() {
        let err = ScimProvisioningError::UserCreationDisabled;
        let scim_err: ScimErrorResponse = err.into();
        assert_eq!(scim_err.status, "403");

        let err = ScimProvisioningError::DuplicateUserName("test@example.com".to_string());
        let scim_err: ScimErrorResponse = err.into();
        assert_eq!(scim_err.status, "409");

        let err = ScimProvisioningError::UserNotFound("123".to_string());
        let scim_err: ScimErrorResponse = err.into();
        assert_eq!(scim_err.status, "404");
    }

    #[test]
    fn test_generate_team_slug() {
        // Normal display name
        assert_eq!(generate_team_slug("Engineering"), "engineering");

        // Display name with spaces
        assert_eq!(generate_team_slug("Product Team"), "product-team");

        // Display name with special characters
        assert_eq!(generate_team_slug("Dev & Ops"), "dev-ops");

        // Multiple consecutive special characters
        assert_eq!(generate_team_slug("Dev --- Ops"), "dev-ops");

        // Leading/trailing special characters
        assert_eq!(generate_team_slug("---Engineering---"), "engineering");

        // Mixed case
        assert_eq!(generate_team_slug("DevOps Team"), "devops-team");

        // Numbers
        assert_eq!(generate_team_slug("Team 42"), "team-42");
    }

    #[test]
    fn test_generate_team_slug_empty_fallback() {
        // Empty display name should produce UUID-based fallback
        let slug = generate_team_slug("");
        assert!(slug.starts_with("group-"));
        assert_eq!(slug.len(), 14); // "group-" (6) + 8 hex chars

        // Whitespace only should produce fallback
        let slug = generate_team_slug("   ");
        assert!(slug.starts_with("group-"));

        // Special chars only should produce fallback
        let slug = generate_team_slug("---");
        assert!(slug.starts_with("group-"));

        // Non-ASCII chars only should produce fallback
        let slug = generate_team_slug("日本語");
        assert!(slug.starts_with("group-"));
    }
}
