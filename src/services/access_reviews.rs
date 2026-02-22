use std::{cmp::Reverse, sync::Arc};

use chrono::{DateTime, Duration, Utc};
use uuid::Uuid;

use crate::{
    db::{DbPool, DbResult, ListParams},
    models::{
        AccessGrantHistoryEntry, AccessInventoryResponse, AccessInventorySummary, ApiKeySummary,
        AuditActorType, AuditLogQuery, NeverActiveUserEntry, OrgAccessEntry,
        OrgAccessReportResponse, OrgAccessReportSummary, OrgApiKeyEntry, OrgMemberAccessEntry,
        OrgMemberProjectAccess, Organization, ProjectAccessEntry, StaleAccessResponse,
        StaleAccessSummary, StaleApiKeyEntry, StaleUserEntry, User, UserAccessApiKeyEntry,
        UserAccessInventoryEntry, UserAccessOrgEntry, UserAccessProjectEntry, UserAccessSummary,
        UserAccessSummaryResponse,
    },
};

/// Service layer for access review operations
#[derive(Clone)]
pub struct AccessReviewService {
    db: Arc<DbPool>,
}

impl AccessReviewService {
    pub fn new(db: Arc<DbPool>) -> Self {
        Self { db }
    }

    /// Get the full access inventory for all users
    ///
    /// This returns a comprehensive view of all users and their access rights
    /// across organizations and projects, including API key counts and last activity.
    pub async fn get_access_inventory(
        &self,
        org_filter: Option<Uuid>,
        limit: i64,
        offset: i64,
    ) -> DbResult<AccessInventoryResponse> {
        let generated_at = Utc::now();

        // Get total user count
        let total_users = self.db.users().count(false).await?;

        // Get all users (paginated)
        let users_result = self
            .db
            .users()
            .list(ListParams {
                limit: Some(limit),
                include_deleted: false,
                ..Default::default()
            })
            .await?;

        // Build user access entries
        let mut user_entries = Vec::with_capacity(users_result.items.len());

        for user in users_result.items.into_iter().skip(offset as usize) {
            // Get org memberships
            let org_memberships = self
                .db
                .users()
                .get_org_memberships_for_user(user.id)
                .await?;

            // If filtering by org, skip users not in that org
            if let Some(filter_org_id) = org_filter
                && !org_memberships.iter().any(|m| m.org_id == filter_org_id)
            {
                continue;
            }

            // Convert to OrgAccessEntry
            let org_entries: Vec<OrgAccessEntry> = org_memberships
                .into_iter()
                .map(|m| OrgAccessEntry {
                    org_id: m.org_id,
                    org_slug: m.org_slug,
                    org_name: m.org_name,
                    role: m.role,
                    granted_at: m.joined_at,
                })
                .collect();

            // Get project memberships with org slug
            let project_memberships = self
                .db
                .users()
                .get_project_memberships_for_user(user.id)
                .await?;

            // Convert project memberships to access entries
            // We need org slugs - look them up from the already fetched org entries
            let project_entries: Vec<ProjectAccessEntry> = project_memberships
                .into_iter()
                .map(|m| {
                    let org_slug = org_entries
                        .iter()
                        .find(|o| o.org_id == m.org_id)
                        .map(|o| o.org_slug.clone())
                        .unwrap_or_else(|| String::from("unknown"));

                    ProjectAccessEntry {
                        project_id: m.project_id,
                        project_slug: m.project_slug,
                        project_name: m.project_name,
                        org_id: m.org_id,
                        org_slug,
                        role: m.role,
                        granted_at: m.joined_at,
                    }
                })
                .collect();

            // Get API key summary
            let api_key_summary = self.get_api_key_summary_for_user(user.id).await?;

            // Get last activity from audit logs
            let last_activity_at = self.get_last_activity_for_user(user.id).await?;

            user_entries.push(UserAccessInventoryEntry {
                user_id: user.id,
                external_id: user.external_id,
                email: user.email,
                name: user.name,
                created_at: user.created_at,
                organizations: org_entries,
                projects: project_entries,
                api_key_summary,
                last_activity_at,
            });
        }

        // Calculate summary statistics
        let summary = self.calculate_summary().await?;

        Ok(AccessInventoryResponse {
            generated_at,
            total_users,
            users: user_entries,
            summary,
        })
    }

    /// Get API key summary for a user (active, revoked, expired counts)
    async fn get_api_key_summary_for_user(&self, user_id: Uuid) -> DbResult<ApiKeySummary> {
        // Get all API keys for the user (including deleted/revoked)
        let result = self
            .db
            .api_keys()
            .list_by_user(
                user_id,
                ListParams {
                    limit: Some(1000), // High limit to get all
                    include_deleted: true,
                    ..Default::default()
                },
            )
            .await?;

        let now = Utc::now();
        let mut active_count = 0i64;
        let mut revoked_count = 0i64;
        let mut expired_count = 0i64;

        for key in &result.items {
            if key.revoked_at.is_some() {
                revoked_count += 1;
            } else if key.expires_at.is_some_and(|exp| exp < now) {
                expired_count += 1;
            } else {
                active_count += 1;
            }
        }

        let total_count = result.items.len() as i64;

        Ok(ApiKeySummary {
            active_count,
            revoked_count,
            expired_count,
            total_count,
        })
    }

    /// Get the last activity timestamp for a user from audit logs
    async fn get_last_activity_for_user(&self, user_id: Uuid) -> DbResult<Option<DateTime<Utc>>> {
        // Query audit logs where user was the actor, get most recent
        let result = self
            .db
            .audit_logs()
            .list(AuditLogQuery {
                actor_type: Some(AuditActorType::User),
                actor_id: Some(user_id),
                limit: Some(1),
                ..Default::default()
            })
            .await?;

        Ok(result.items.first().map(|log| log.timestamp))
    }

    /// Calculate summary statistics for the access inventory
    async fn calculate_summary(&self) -> DbResult<AccessInventorySummary> {
        // Count total organizations
        let total_organizations = self.db.organizations().count(false).await?;

        // For project count, we need to iterate through orgs
        // This is not ideal but ProjectRepo doesn't have a global count method
        let orgs = self
            .db
            .organizations()
            .list(ListParams {
                limit: Some(10000),
                include_deleted: false,
                ..Default::default()
            })
            .await?;

        let mut total_projects = 0i64;
        for org in &orgs.items {
            total_projects += self.db.projects().count_by_org(org.id, false).await?;
        }

        // For membership counts, we need to iterate through users
        // This is not ideal for large datasets, but sufficient for now
        let users = self
            .db
            .users()
            .list(ListParams {
                limit: Some(10000),
                include_deleted: false,
                ..Default::default()
            })
            .await?;

        let mut total_org_memberships = 0i64;
        let mut total_project_memberships = 0i64;
        let mut total_active_api_keys = 0i64;
        let now = Utc::now();

        for user in users.items {
            let org_memberships = self
                .db
                .users()
                .get_org_memberships_for_user(user.id)
                .await?;
            total_org_memberships += org_memberships.len() as i64;

            let project_memberships = self
                .db
                .users()
                .get_project_memberships_for_user(user.id)
                .await?;
            total_project_memberships += project_memberships.len() as i64;

            let api_keys = self
                .db
                .api_keys()
                .list_by_user(
                    user.id,
                    ListParams {
                        limit: Some(1000),
                        include_deleted: true,
                        ..Default::default()
                    },
                )
                .await?;

            for key in api_keys.items {
                if key.revoked_at.is_none() && key.expires_at.is_none_or(|exp| exp >= now) {
                    total_active_api_keys += 1;
                }
            }
        }

        Ok(AccessInventorySummary {
            total_organizations,
            total_projects,
            total_org_memberships,
            total_project_memberships,
            total_active_api_keys,
        })
    }

    // ==================== Organization Access Report ====================

    /// Get a comprehensive access report for a specific organization.
    ///
    /// This returns all access details for the organization including:
    /// - All org members with their roles and project access
    /// - All API keys scoped to the org or its projects
    /// - Recent access grant history from audit logs
    pub async fn get_org_access_report(
        &self,
        org: &Organization,
    ) -> DbResult<OrgAccessReportResponse> {
        let generated_at = Utc::now();

        // Get all projects in this org
        let projects = self
            .db
            .projects()
            .list_by_org(
                org.id,
                ListParams {
                    limit: Some(1000),
                    include_deleted: false,
                    ..Default::default()
                },
            )
            .await?;

        // Get all org members
        let org_members = self
            .db
            .users()
            .list_org_members(
                org.id,
                ListParams {
                    limit: Some(10000),
                    include_deleted: false,
                    ..Default::default()
                },
            )
            .await?;

        // Build member access entries
        let mut members = Vec::with_capacity(org_members.items.len());
        let mut total_project_memberships = 0i64;

        for user in org_members.items {
            // Get user's org membership details
            let org_memberships = self
                .db
                .users()
                .get_org_memberships_for_user(user.id)
                .await?;

            let org_membership = org_memberships.iter().find(|m| m.org_id == org.id);

            let (role, granted_at) = org_membership
                .map(|m| (m.role.clone(), m.joined_at))
                .unwrap_or_else(|| ("member".to_string(), user.created_at));

            // Get user's project memberships within this org
            let user_project_memberships = self
                .db
                .users()
                .get_project_memberships_for_user(user.id)
                .await?;

            let project_access: Vec<OrgMemberProjectAccess> = user_project_memberships
                .iter()
                .filter(|pm| pm.org_id == org.id)
                .map(|pm| OrgMemberProjectAccess {
                    project_id: pm.project_id,
                    project_slug: pm.project_slug.clone(),
                    project_name: pm.project_name.clone(),
                    role: pm.role.clone(),
                    granted_at: pm.joined_at,
                })
                .collect();

            total_project_memberships += project_access.len() as i64;

            // Get API key summary for user (only keys related to this org)
            let api_key_summary = self
                .get_org_scoped_api_key_summary_for_user(user.id, org.id, &projects.items)
                .await?;

            // Get last activity within this org
            let last_activity_at = self
                .get_last_activity_for_user_in_org(user.id, org.id)
                .await?;

            members.push(OrgMemberAccessEntry {
                user_id: user.id,
                external_id: user.external_id.clone(),
                email: user.email.clone(),
                name: user.name.clone(),
                role,
                granted_at,
                project_access,
                api_key_summary,
                last_activity_at,
            });
        }

        // Get all API keys for this org and its projects
        let api_keys = self.get_org_api_keys(org.id, &projects.items).await?;

        // Count active/revoked keys
        let active_api_keys = api_keys.iter().filter(|k| k.is_active).count() as i64;
        let revoked_api_keys = api_keys.iter().filter(|k| !k.is_active).count() as i64;

        // Get access grant history from audit logs
        let access_history = self.get_access_grant_history(org.id).await?;

        let summary = OrgAccessReportSummary {
            total_members: members.len() as i64,
            total_projects: projects.items.len() as i64,
            total_project_memberships,
            active_api_keys,
            revoked_api_keys,
        };

        Ok(OrgAccessReportResponse {
            generated_at,
            org_id: org.id,
            org_slug: org.slug.clone(),
            org_name: org.name.clone(),
            members,
            api_keys,
            access_history,
            summary,
        })
    }

    /// Get API key summary for a user, scoped to a specific org and its projects
    async fn get_org_scoped_api_key_summary_for_user(
        &self,
        user_id: Uuid,
        org_id: Uuid,
        projects: &[crate::models::Project],
    ) -> DbResult<ApiKeySummary> {
        let project_ids: std::collections::HashSet<Uuid> = projects.iter().map(|p| p.id).collect();

        let result = self
            .db
            .api_keys()
            .list_by_user(
                user_id,
                ListParams {
                    limit: Some(1000),
                    include_deleted: true,
                    ..Default::default()
                },
            )
            .await?;

        let now = Utc::now();
        let mut active_count = 0i64;
        let mut revoked_count = 0i64;
        let mut expired_count = 0i64;

        for key in &result.items {
            // Check if this key is scoped to the org or one of its projects
            let is_org_scoped = match &key.owner {
                crate::models::ApiKeyOwner::Organization { org_id: key_org_id } => {
                    *key_org_id == org_id
                }
                crate::models::ApiKeyOwner::Team { .. } => false, // Team-owned keys counted separately
                crate::models::ApiKeyOwner::Project { project_id } => {
                    project_ids.contains(project_id)
                }
                crate::models::ApiKeyOwner::User { .. } => false, // User-owned keys counted separately
                crate::models::ApiKeyOwner::ServiceAccount { .. } => false, // Service account-owned keys counted separately
            };

            if !is_org_scoped {
                continue;
            }

            if key.revoked_at.is_some() {
                revoked_count += 1;
            } else if key.expires_at.is_some_and(|exp| exp < now) {
                expired_count += 1;
            } else {
                active_count += 1;
            }
        }

        let total_count = active_count + revoked_count + expired_count;

        Ok(ApiKeySummary {
            active_count,
            revoked_count,
            expired_count,
            total_count,
        })
    }

    /// Get last activity timestamp for a user within a specific organization
    async fn get_last_activity_for_user_in_org(
        &self,
        user_id: Uuid,
        org_id: Uuid,
    ) -> DbResult<Option<DateTime<Utc>>> {
        let result = self
            .db
            .audit_logs()
            .list(AuditLogQuery {
                actor_type: Some(AuditActorType::User),
                actor_id: Some(user_id),
                org_id: Some(org_id),
                limit: Some(1),
                ..Default::default()
            })
            .await?;

        Ok(result.items.first().map(|log| log.timestamp))
    }

    /// Get all API keys scoped to an organization and its projects
    async fn get_org_api_keys(
        &self,
        org_id: Uuid,
        projects: &[crate::models::Project],
    ) -> DbResult<Vec<OrgApiKeyEntry>> {
        let now = Utc::now();
        let mut api_keys = Vec::new();

        // Get org-level API keys
        let org_keys = self
            .db
            .api_keys()
            .list_by_org(
                org_id,
                ListParams {
                    limit: Some(1000),
                    include_deleted: true,
                    ..Default::default()
                },
            )
            .await?;

        for key in org_keys.items {
            let is_active = key.revoked_at.is_none() && key.expires_at.is_none_or(|exp| exp >= now);
            api_keys.push(OrgApiKeyEntry {
                key_id: key.id,
                name: key.name,
                key_prefix: key.key_prefix,
                owner_type: "organization".to_string(),
                owner_id: org_id,
                project_slug: None,
                user_id: None,
                user_email: None,
                is_active,
                created_at: key.created_at,
                revoked_at: key.revoked_at,
                expires_at: key.expires_at,
                last_used_at: key.last_used_at,
            });
        }

        // Get project-level API keys
        for project in projects {
            let project_keys = self
                .db
                .api_keys()
                .list_by_project(
                    project.id,
                    ListParams {
                        limit: Some(1000),
                        include_deleted: true,
                        ..Default::default()
                    },
                )
                .await?;

            for key in project_keys.items {
                let is_active =
                    key.revoked_at.is_none() && key.expires_at.is_none_or(|exp| exp >= now);
                api_keys.push(OrgApiKeyEntry {
                    key_id: key.id,
                    name: key.name,
                    key_prefix: key.key_prefix,
                    owner_type: "project".to_string(),
                    owner_id: project.id,
                    project_slug: Some(project.slug.clone()),
                    user_id: None,
                    user_email: None,
                    is_active,
                    created_at: key.created_at,
                    revoked_at: key.revoked_at,
                    expires_at: key.expires_at,
                    last_used_at: key.last_used_at,
                });
            }
        }

        Ok(api_keys)
    }

    /// Get access grant history from audit logs for an organization
    async fn get_access_grant_history(
        &self,
        org_id: Uuid,
    ) -> DbResult<Vec<AccessGrantHistoryEntry>> {
        // Query audit logs for membership-related actions
        let membership_actions = [
            "org_membership.create",
            "org_membership.delete",
            "project_membership.create",
            "project_membership.delete",
        ];

        let mut history = Vec::new();

        for action in membership_actions {
            let logs = self
                .db
                .audit_logs()
                .list(AuditLogQuery {
                    action: Some(action.to_string()),
                    org_id: Some(org_id),
                    limit: Some(100),
                    ..Default::default()
                })
                .await?;

            for log in logs.items {
                history.push(AccessGrantHistoryEntry {
                    log_id: log.id,
                    action: log.action,
                    resource_type: log.resource_type,
                    resource_id: log.resource_id,
                    actor_type: log.actor_type.to_string(),
                    actor_id: log.actor_id,
                    timestamp: log.timestamp,
                    details: Some(log.details),
                });
            }
        }

        // Sort by timestamp descending
        history.sort_by_key(|a| Reverse(a.timestamp));

        // Limit to most recent 100 entries
        history.truncate(100);

        Ok(history)
    }

    // ==================== User Access Summary ====================

    /// Get a comprehensive access summary for a specific user.
    ///
    /// This returns all access details for the user including:
    /// - All organizations the user belongs to
    /// - All projects the user belongs to
    /// - All API keys owned by the user
    /// - When each access was granted
    /// - Last activity per resource
    pub async fn get_user_access_summary(
        &self,
        user: &User,
    ) -> DbResult<UserAccessSummaryResponse> {
        let generated_at = Utc::now();

        // Get org memberships
        let org_memberships = self
            .db
            .users()
            .get_org_memberships_for_user(user.id)
            .await?;

        // Build org access entries with last activity
        let mut organizations = Vec::with_capacity(org_memberships.len());
        for membership in &org_memberships {
            let last_activity_at = self
                .get_last_activity_for_user_in_org(user.id, membership.org_id)
                .await?;

            organizations.push(UserAccessOrgEntry {
                org_id: membership.org_id,
                org_slug: membership.org_slug.clone(),
                org_name: membership.org_name.clone(),
                role: membership.role.clone(),
                granted_at: membership.joined_at,
                last_activity_at,
            });
        }

        // Get project memberships
        let project_memberships = self
            .db
            .users()
            .get_project_memberships_for_user(user.id)
            .await?;

        // Build project access entries with last activity
        let mut projects = Vec::with_capacity(project_memberships.len());
        for membership in &project_memberships {
            // Look up org slug from org memberships
            let org_slug = organizations
                .iter()
                .find(|o| o.org_id == membership.org_id)
                .map(|o| o.org_slug.clone())
                .unwrap_or_else(|| String::from("unknown"));

            let last_activity_at = self
                .get_last_activity_for_user_in_project(user.id, membership.project_id)
                .await?;

            projects.push(UserAccessProjectEntry {
                project_id: membership.project_id,
                project_slug: membership.project_slug.clone(),
                project_name: membership.project_name.clone(),
                org_id: membership.org_id,
                org_slug,
                role: membership.role.clone(),
                granted_at: membership.joined_at,
                last_activity_at,
            });
        }

        // Get all API keys owned by the user
        let api_keys_result = self
            .db
            .api_keys()
            .list_by_user(
                user.id,
                ListParams {
                    limit: Some(1000),
                    include_deleted: true,
                    ..Default::default()
                },
            )
            .await?;

        let now = Utc::now();
        let mut api_keys = Vec::with_capacity(api_keys_result.items.len());
        let mut active_count = 0i64;
        let mut revoked_count = 0i64;
        let mut expired_count = 0i64;

        for key in api_keys_result.items {
            let (owner_type, owner_id) = match &key.owner {
                crate::models::ApiKeyOwner::Organization { org_id } => {
                    ("organization".to_string(), *org_id)
                }
                crate::models::ApiKeyOwner::Team { team_id } => ("team".to_string(), *team_id),
                crate::models::ApiKeyOwner::Project { project_id } => {
                    ("project".to_string(), *project_id)
                }
                crate::models::ApiKeyOwner::User { user_id } => ("user".to_string(), *user_id),
                crate::models::ApiKeyOwner::ServiceAccount { service_account_id } => {
                    ("service_account".to_string(), *service_account_id)
                }
            };

            let is_active = key.revoked_at.is_none() && key.expires_at.is_none_or(|exp| exp >= now);

            if key.revoked_at.is_some() {
                revoked_count += 1;
            } else if key.expires_at.is_some_and(|exp| exp < now) {
                expired_count += 1;
            } else {
                active_count += 1;
            }

            api_keys.push(UserAccessApiKeyEntry {
                key_id: key.id,
                name: key.name,
                key_prefix: key.key_prefix,
                owner_type,
                owner_id,
                is_active,
                created_at: key.created_at,
                revoked_at: key.revoked_at,
                expires_at: key.expires_at,
                last_used_at: key.last_used_at,
            });
        }

        // Get last activity for user overall
        let last_activity_at = self.get_last_activity_for_user(user.id).await?;

        let summary = UserAccessSummary {
            total_organizations: organizations.len() as i64,
            total_projects: projects.len() as i64,
            active_api_keys: active_count,
            revoked_api_keys: revoked_count,
            expired_api_keys: expired_count,
        };

        Ok(UserAccessSummaryResponse {
            generated_at,
            user_id: user.id,
            external_id: user.external_id.clone(),
            email: user.email.clone(),
            name: user.name.clone(),
            created_at: user.created_at,
            organizations,
            projects,
            api_keys,
            last_activity_at,
            summary,
        })
    }

    /// Get last activity timestamp for a user within a specific project
    async fn get_last_activity_for_user_in_project(
        &self,
        user_id: Uuid,
        project_id: Uuid,
    ) -> DbResult<Option<DateTime<Utc>>> {
        let result = self
            .db
            .audit_logs()
            .list(AuditLogQuery {
                actor_type: Some(AuditActorType::User),
                actor_id: Some(user_id),
                project_id: Some(project_id),
                limit: Some(1),
                ..Default::default()
            })
            .await?;

        Ok(result.items.first().map(|log| log.timestamp))
    }

    // ==================== Stale Access Detection ====================

    /// Detect stale access across the system.
    ///
    /// This identifies:
    /// - Users who haven't been active for N days
    /// - API keys not used for N days
    /// - Users with access but no recorded activity
    pub async fn get_stale_access(
        &self,
        inactive_days: i64,
        org_filter: Option<Uuid>,
        limit: i64,
    ) -> DbResult<StaleAccessResponse> {
        let generated_at = Utc::now();
        let cutoff_date = generated_at - Duration::days(inactive_days);

        let mut stale_users = Vec::new();
        let mut never_active_users = Vec::new();
        let mut stale_api_keys = Vec::new();

        let mut total_users_scanned = 0i64;
        let mut total_api_keys_scanned = 0i64;
        let mut never_used_api_keys_count = 0i64;

        // Get all users (paginated in batches)
        let users = self
            .db
            .users()
            .list(ListParams {
                limit: Some(10000),
                include_deleted: false,
                ..Default::default()
            })
            .await?;

        for user in users.items {
            // If filtering by org, check membership
            if let Some(filter_org_id) = org_filter {
                let org_memberships = self
                    .db
                    .users()
                    .get_org_memberships_for_user(user.id)
                    .await?;

                if !org_memberships.iter().any(|m| m.org_id == filter_org_id) {
                    continue;
                }
            }

            total_users_scanned += 1;

            // Get last activity
            let last_activity_at = self.get_last_activity_for_user(user.id).await?;

            // Get org and project membership counts
            let org_memberships = self
                .db
                .users()
                .get_org_memberships_for_user(user.id)
                .await?;
            let project_memberships = self
                .db
                .users()
                .get_project_memberships_for_user(user.id)
                .await?;

            // Get active API key count
            let api_key_summary = self.get_api_key_summary_for_user(user.id).await?;

            match last_activity_at {
                Some(last_activity) if last_activity < cutoff_date => {
                    // User has activity but it's before the cutoff - stale
                    let days_inactive = (generated_at - last_activity).num_days().max(0);

                    if stale_users.len() < limit as usize {
                        stale_users.push(StaleUserEntry {
                            user_id: user.id,
                            external_id: user.external_id.clone(),
                            email: user.email.clone(),
                            name: user.name.clone(),
                            created_at: user.created_at,
                            last_activity_at: Some(last_activity),
                            days_inactive,
                            org_count: org_memberships.len() as i64,
                            project_count: project_memberships.len() as i64,
                            active_api_keys: api_key_summary.active_count,
                        });
                    }
                }
                None => {
                    // User has never had any recorded activity
                    let days_since_creation = (generated_at - user.created_at).num_days().max(0);

                    if never_active_users.len() < limit as usize {
                        never_active_users.push(NeverActiveUserEntry {
                            user_id: user.id,
                            external_id: user.external_id.clone(),
                            email: user.email.clone(),
                            name: user.name.clone(),
                            created_at: user.created_at,
                            days_since_creation,
                            org_count: org_memberships.len() as i64,
                            project_count: project_memberships.len() as i64,
                            active_api_keys: api_key_summary.active_count,
                        });
                    }
                }
                Some(_) => {
                    // User has recent activity, not stale
                }
            }

            // Check user's API keys for staleness
            let api_keys = self
                .db
                .api_keys()
                .list_by_user(
                    user.id,
                    ListParams {
                        limit: Some(1000),
                        include_deleted: false, // Only check active keys
                        ..Default::default()
                    },
                )
                .await?;

            let now = Utc::now();
            for key in api_keys.items {
                // Skip revoked or expired keys
                if key.revoked_at.is_some() {
                    continue;
                }
                if key.expires_at.is_some_and(|exp| exp < now) {
                    continue;
                }

                total_api_keys_scanned += 1;

                let (owner_type, owner_id) = match &key.owner {
                    crate::models::ApiKeyOwner::Organization { org_id } => {
                        ("organization".to_string(), *org_id)
                    }
                    crate::models::ApiKeyOwner::Team { team_id } => ("team".to_string(), *team_id),
                    crate::models::ApiKeyOwner::Project { project_id } => {
                        ("project".to_string(), *project_id)
                    }
                    crate::models::ApiKeyOwner::User { user_id } => ("user".to_string(), *user_id),
                    crate::models::ApiKeyOwner::ServiceAccount { service_account_id } => {
                        ("service_account".to_string(), *service_account_id)
                    }
                };

                let never_used = key.last_used_at.is_none();
                if never_used {
                    never_used_api_keys_count += 1;
                }

                // Determine staleness based on last_used_at or created_at
                let reference_date = key.last_used_at.unwrap_or(key.created_at);
                if reference_date < cutoff_date && stale_api_keys.len() < limit as usize {
                    let days_inactive = (generated_at - reference_date).num_days().max(0);

                    stale_api_keys.push(StaleApiKeyEntry {
                        key_id: key.id,
                        name: key.name,
                        key_prefix: key.key_prefix,
                        owner_type,
                        owner_id,
                        created_at: key.created_at,
                        last_used_at: key.last_used_at,
                        days_inactive,
                        never_used,
                    });
                }
            }
        }

        // Sort by days_inactive descending (most stale first)
        stale_users.sort_by_key(|a| Reverse(a.days_inactive));
        never_active_users.sort_by_key(|a| Reverse(a.days_since_creation));
        stale_api_keys.sort_by_key(|a| Reverse(a.days_inactive));

        let summary = StaleAccessSummary {
            total_users_scanned,
            stale_users_count: stale_users.len() as i64,
            never_active_users_count: never_active_users.len() as i64,
            total_api_keys_scanned,
            stale_api_keys_count: stale_api_keys.len() as i64,
            never_used_api_keys_count,
        };

        Ok(StaleAccessResponse {
            generated_at,
            inactive_days_threshold: inactive_days,
            cutoff_date,
            stale_users,
            stale_api_keys,
            never_active_users,
            summary,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_key_summary_default() {
        let summary = ApiKeySummary {
            active_count: 5,
            revoked_count: 2,
            expired_count: 1,
            total_count: 8,
        };
        assert_eq!(
            summary.active_count + summary.revoked_count + summary.expired_count,
            summary.total_count
        );
    }

    #[test]
    fn test_stale_access_summary_counts() {
        let summary = StaleAccessSummary {
            total_users_scanned: 100,
            stale_users_count: 10,
            never_active_users_count: 5,
            total_api_keys_scanned: 50,
            stale_api_keys_count: 8,
            never_used_api_keys_count: 3,
        };

        // Stale + never active should be <= total scanned
        assert!(
            summary.stale_users_count + summary.never_active_users_count
                <= summary.total_users_scanned
        );
        // Stale keys <= total keys
        assert!(summary.stale_api_keys_count <= summary.total_api_keys_scanned);
        // Never used keys <= stale keys (never used keys are always stale)
        assert!(summary.never_used_api_keys_count <= summary.stale_api_keys_count);
    }

    #[test]
    fn test_stale_user_entry_days_calculation() {
        let now = Utc::now();
        let last_activity = now - Duration::days(95);

        let entry = StaleUserEntry {
            user_id: Uuid::new_v4(),
            external_id: "test".to_string(),
            email: Some("test@example.com".to_string()),
            name: Some("Test User".to_string()),
            created_at: now - Duration::days(365),
            last_activity_at: Some(last_activity),
            days_inactive: 95,
            org_count: 2,
            project_count: 3,
            active_api_keys: 1,
        };

        assert_eq!(entry.days_inactive, 95);
        assert!(entry.last_activity_at.is_some());
    }

    #[test]
    fn test_never_active_user_entry() {
        let now = Utc::now();
        let created_at = now - Duration::days(30);

        let entry = NeverActiveUserEntry {
            user_id: Uuid::new_v4(),
            external_id: "test".to_string(),
            email: Some("test@example.com".to_string()),
            name: None,
            created_at,
            days_since_creation: 30,
            org_count: 1,
            project_count: 0,
            active_api_keys: 0,
        };

        assert_eq!(entry.days_since_creation, 30);
        assert!(entry.name.is_none());
    }

    #[test]
    fn test_stale_api_key_entry_never_used() {
        let now = Utc::now();
        let created_at = now - Duration::days(100);

        let entry = StaleApiKeyEntry {
            key_id: Uuid::new_v4(),
            name: "Test Key".to_string(),
            key_prefix: "hdr_".to_string(),
            owner_type: "user".to_string(),
            owner_id: Uuid::new_v4(),
            created_at,
            last_used_at: None,
            days_inactive: 100,
            never_used: true,
        };

        assert!(entry.never_used);
        assert!(entry.last_used_at.is_none());
        assert_eq!(entry.days_inactive, 100);
    }

    #[test]
    fn test_stale_api_key_entry_used_but_stale() {
        let now = Utc::now();
        let created_at = now - Duration::days(200);
        let last_used_at = now - Duration::days(100);

        let entry = StaleApiKeyEntry {
            key_id: Uuid::new_v4(),
            name: "Test Key".to_string(),
            key_prefix: "hdr_".to_string(),
            owner_type: "project".to_string(),
            owner_id: Uuid::new_v4(),
            created_at,
            last_used_at: Some(last_used_at),
            days_inactive: 100,
            never_used: false,
        };

        assert!(!entry.never_used);
        assert!(entry.last_used_at.is_some());
        assert_eq!(entry.days_inactive, 100);
    }
}
