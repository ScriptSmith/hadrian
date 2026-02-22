use async_trait::async_trait;
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use super::common::parse_uuid;
use crate::{
    db::{
        error::{DbError, DbResult},
        repos::{DomainVerificationRepo, ListParams},
    },
    models::{
        CreateDomainVerification, DomainVerification, DomainVerificationStatus,
        UpdateDomainVerification,
    },
};

pub struct SqliteDomainVerificationRepo {
    pool: SqlitePool,
}

impl SqliteDomainVerificationRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Parse a DomainVerification from a database row.
    fn parse_verification(row: &sqlx::sqlite::SqliteRow) -> DbResult<DomainVerification> {
        let status_str: String = row.get("status");
        let status = status_str
            .parse::<DomainVerificationStatus>()
            .unwrap_or_default();

        Ok(DomainVerification {
            id: parse_uuid(&row.get::<String, _>("id"))?,
            org_sso_config_id: parse_uuid(&row.get::<String, _>("org_sso_config_id"))?,
            domain: row.get("domain"),
            verification_token: row.get("verification_token"),
            status,
            dns_txt_record: row.get("dns_txt_record"),
            verification_attempts: row.get("verification_attempts"),
            last_attempt_at: row.get("last_attempt_at"),
            verified_at: row.get("verified_at"),
            expires_at: row.get("expires_at"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }
}

#[async_trait]
impl DomainVerificationRepo for SqliteDomainVerificationRepo {
    async fn create(
        &self,
        org_sso_config_id: Uuid,
        input: CreateDomainVerification,
        verification_token: &str,
    ) -> DbResult<DomainVerification> {
        let id = Uuid::new_v4();
        let now = chrono::Utc::now();

        sqlx::query(
            r#"
            INSERT INTO domain_verifications (
                id, org_sso_config_id, domain, verification_token, status,
                verification_attempts, created_at, updated_at
            )
            VALUES (?, ?, ?, ?, 'pending', 0, ?, ?)
            "#,
        )
        .bind(id.to_string())
        .bind(org_sso_config_id.to_string())
        .bind(&input.domain)
        .bind(verification_token)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
                DbError::Conflict(format!(
                    "Domain '{}' already exists for this SSO configuration",
                    input.domain
                ))
            }
            sqlx::Error::Database(db_err) if db_err.is_foreign_key_violation() => DbError::NotFound,
            _ => DbError::from(e),
        })?;

        Ok(DomainVerification {
            id,
            org_sso_config_id,
            domain: input.domain,
            verification_token: verification_token.to_string(),
            status: DomainVerificationStatus::Pending,
            dns_txt_record: None,
            verification_attempts: 0,
            last_attempt_at: None,
            verified_at: None,
            expires_at: None,
            created_at: now,
            updated_at: now,
        })
    }

    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<DomainVerification>> {
        let result = sqlx::query(
            r#"
            SELECT id, org_sso_config_id, domain, verification_token, status,
                   dns_txt_record, verification_attempts, last_attempt_at, verified_at,
                   expires_at, created_at, updated_at
            FROM domain_verifications
            WHERE id = ?
            "#,
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        match result {
            Some(row) => Ok(Some(Self::parse_verification(&row)?)),
            None => Ok(None),
        }
    }

    async fn get_by_config_and_domain(
        &self,
        org_sso_config_id: Uuid,
        domain: &str,
    ) -> DbResult<Option<DomainVerification>> {
        let result = sqlx::query(
            r#"
            SELECT id, org_sso_config_id, domain, verification_token, status,
                   dns_txt_record, verification_attempts, last_attempt_at, verified_at,
                   expires_at, created_at, updated_at
            FROM domain_verifications
            WHERE org_sso_config_id = ? AND domain = ?
            "#,
        )
        .bind(org_sso_config_id.to_string())
        .bind(domain)
        .fetch_optional(&self.pool)
        .await?;

        match result {
            Some(row) => Ok(Some(Self::parse_verification(&row)?)),
            None => Ok(None),
        }
    }

    async fn list_by_config(
        &self,
        org_sso_config_id: Uuid,
        params: ListParams,
    ) -> DbResult<Vec<DomainVerification>> {
        let limit = params.limit.unwrap_or(100);

        let rows = sqlx::query(
            r#"
            SELECT id, org_sso_config_id, domain, verification_token, status,
                   dns_txt_record, verification_attempts, last_attempt_at, verified_at,
                   expires_at, created_at, updated_at
            FROM domain_verifications
            WHERE org_sso_config_id = ?
            ORDER BY created_at DESC, id DESC
            LIMIT ?
            "#,
        )
        .bind(org_sso_config_id.to_string())
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        rows.iter()
            .map(Self::parse_verification)
            .collect::<DbResult<Vec<_>>>()
    }

    async fn count_by_config(&self, org_sso_config_id: Uuid) -> DbResult<i64> {
        let row = sqlx::query(
            "SELECT COUNT(*) as count FROM domain_verifications WHERE org_sso_config_id = ?",
        )
        .bind(org_sso_config_id.to_string())
        .fetch_one(&self.pool)
        .await?;

        Ok(row.get::<i64, _>("count"))
    }

    async fn update(
        &self,
        id: Uuid,
        input: UpdateDomainVerification,
    ) -> DbResult<DomainVerification> {
        let now = chrono::Utc::now();

        // Fetch existing record
        let existing = self.get_by_id(id).await?.ok_or(DbError::NotFound)?;

        sqlx::query(
            r#"
            UPDATE domain_verifications SET
                status = ?,
                dns_txt_record = ?,
                verification_attempts = ?,
                last_attempt_at = ?,
                verified_at = ?,
                expires_at = ?,
                updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(input.status.unwrap_or(existing.status).to_string())
        .bind(input.dns_txt_record.unwrap_or(existing.dns_txt_record))
        .bind(
            input
                .verification_attempts
                .unwrap_or(existing.verification_attempts),
        )
        .bind(input.last_attempt_at.unwrap_or(existing.last_attempt_at))
        .bind(input.verified_at.unwrap_or(existing.verified_at))
        .bind(input.expires_at.unwrap_or(existing.expires_at))
        .bind(now)
        .bind(id.to_string())
        .execute(&self.pool)
        .await?;

        self.get_by_id(id).await?.ok_or(DbError::NotFound)
    }

    async fn delete(&self, id: Uuid) -> DbResult<()> {
        let result = sqlx::query("DELETE FROM domain_verifications WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }

        Ok(())
    }

    async fn find_verified_by_domain(&self, domain: &str) -> DbResult<Option<DomainVerification>> {
        let result = sqlx::query(
            r#"
            SELECT dv.id, dv.org_sso_config_id, dv.domain, dv.verification_token, dv.status,
                   dv.dns_txt_record, dv.verification_attempts, dv.last_attempt_at, dv.verified_at,
                   dv.expires_at, dv.created_at, dv.updated_at
            FROM domain_verifications dv
            JOIN org_sso_configs osc ON dv.org_sso_config_id = osc.id
            WHERE dv.domain = ?
              AND dv.status = 'verified'
              AND osc.enabled = 1
              AND (dv.expires_at IS NULL OR dv.expires_at > datetime('now'))
            LIMIT 1
            "#,
        )
        .bind(domain)
        .fetch_optional(&self.pool)
        .await?;

        match result {
            Some(row) => Ok(Some(Self::parse_verification(&row)?)),
            None => Ok(None),
        }
    }

    async fn list_verified_by_config(
        &self,
        org_sso_config_id: Uuid,
    ) -> DbResult<Vec<DomainVerification>> {
        let rows = sqlx::query(
            r#"
            SELECT id, org_sso_config_id, domain, verification_token, status,
                   dns_txt_record, verification_attempts, last_attempt_at, verified_at,
                   expires_at, created_at, updated_at
            FROM domain_verifications
            WHERE org_sso_config_id = ?
              AND status = 'verified'
              AND (expires_at IS NULL OR expires_at > datetime('now'))
            ORDER BY domain ASC
            "#,
        )
        .bind(org_sso_config_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        rows.iter()
            .map(Self::parse_verification)
            .collect::<DbResult<Vec<_>>>()
    }

    async fn has_verified_domain(&self, org_sso_config_id: Uuid) -> DbResult<bool> {
        let row = sqlx::query(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM domain_verifications
                WHERE org_sso_config_id = ?
                  AND status = 'verified'
                  AND (expires_at IS NULL OR expires_at > datetime('now'))
            ) as has_verified
            "#,
        )
        .bind(org_sso_config_id.to_string())
        .fetch_one(&self.pool)
        .await?;

        // SQLite returns 0 or 1 for EXISTS
        Ok(row.get::<i32, _>("has_verified") != 0)
    }

    async fn create_auto_verified(
        &self,
        org_sso_config_id: Uuid,
        input: CreateDomainVerification,
        verification_token: &str,
    ) -> DbResult<DomainVerification> {
        let id = Uuid::new_v4();
        let now = chrono::Utc::now();

        sqlx::query(
            r#"
            INSERT INTO domain_verifications (
                id, org_sso_config_id, domain, verification_token, status,
                dns_txt_record, verification_attempts, verified_at, created_at, updated_at
            )
            VALUES (?, ?, ?, ?, 'verified', 'AUTO_VERIFIED_BY_BOOTSTRAP', 0, ?, ?, ?)
            "#,
        )
        .bind(id.to_string())
        .bind(org_sso_config_id.to_string())
        .bind(&input.domain)
        .bind(verification_token)
        .bind(now) // verified_at
        .bind(now) // created_at
        .bind(now) // updated_at
        .execute(&self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
                DbError::Conflict(format!(
                    "Domain '{}' already exists for this SSO configuration",
                    input.domain
                ))
            }
            sqlx::Error::Database(db_err) if db_err.is_foreign_key_violation() => DbError::NotFound,
            _ => DbError::from(e),
        })?;

        Ok(DomainVerification {
            id,
            org_sso_config_id,
            domain: input.domain,
            verification_token: verification_token.to_string(),
            status: DomainVerificationStatus::Verified,
            dns_txt_record: Some("AUTO_VERIFIED_BY_BOOTSTRAP".to_string()),
            verification_attempts: 0,
            last_attempt_at: None,
            verified_at: Some(now),
            expires_at: None,
            created_at: now,
            updated_at: now,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn create_test_pool() -> SqlitePool {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("Failed to create in-memory SQLite pool");

        // Create organizations table (required for FK)
        sqlx::query(
            r#"
            CREATE TABLE organizations (
                id TEXT PRIMARY KEY NOT NULL,
                slug TEXT NOT NULL UNIQUE,
                name TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                deleted_at TEXT
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to create organizations table");

        // Create teams table (required for FK)
        sqlx::query(
            r#"
            CREATE TABLE teams (
                id TEXT PRIMARY KEY NOT NULL,
                org_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
                slug TEXT NOT NULL,
                name TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                deleted_at TEXT,
                UNIQUE(org_id, slug)
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to create teams table");

        // Create org_sso_configs table
        sqlx::query(
            r#"
            CREATE TABLE org_sso_configs (
                id TEXT PRIMARY KEY NOT NULL,
                org_id TEXT NOT NULL UNIQUE REFERENCES organizations(id) ON DELETE CASCADE,
                provider_type TEXT NOT NULL DEFAULT 'oidc',
                issuer TEXT NOT NULL,
                discovery_url TEXT,
                client_id TEXT NOT NULL,
                client_secret_key TEXT NOT NULL,
                redirect_uri TEXT,
                scopes TEXT NOT NULL DEFAULT 'openid email profile',
                identity_claim TEXT NOT NULL DEFAULT 'sub',
                org_claim TEXT,
                groups_claim TEXT,
                provisioning_enabled INTEGER NOT NULL DEFAULT 1,
                create_users INTEGER NOT NULL DEFAULT 1,
                default_team_id TEXT REFERENCES teams(id) ON DELETE SET NULL,
                default_org_role TEXT NOT NULL DEFAULT 'member',
                default_team_role TEXT NOT NULL DEFAULT 'member',
                allowed_email_domains TEXT,
                sync_attributes_on_login INTEGER NOT NULL DEFAULT 0,
                sync_memberships_on_login INTEGER NOT NULL DEFAULT 1,
                enforcement_mode TEXT NOT NULL DEFAULT 'optional',
                enabled INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to create org_sso_configs table");

        // Create domain_verifications table
        sqlx::query(
            r#"
            CREATE TABLE domain_verifications (
                id TEXT PRIMARY KEY NOT NULL,
                org_sso_config_id TEXT NOT NULL REFERENCES org_sso_configs(id) ON DELETE CASCADE,
                domain TEXT NOT NULL,
                verification_token TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'verified', 'failed')),
                dns_txt_record TEXT,
                verification_attempts INTEGER NOT NULL DEFAULT 0,
                last_attempt_at TEXT,
                verified_at TEXT,
                expires_at TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                UNIQUE(org_sso_config_id, domain)
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to create domain_verifications table");

        pool
    }

    async fn create_test_org(pool: &SqlitePool, slug: &str) -> Uuid {
        let id = Uuid::new_v4();
        let now = chrono::Utc::now();
        sqlx::query(
            "INSERT INTO organizations (id, slug, name, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(id.to_string())
        .bind(slug)
        .bind(format!("Org {}", slug))
        .bind(now)
        .bind(now)
        .execute(pool)
        .await
        .expect("Failed to create test org");
        id
    }

    async fn create_test_sso_config(pool: &SqlitePool, org_id: Uuid) -> Uuid {
        let id = Uuid::new_v4();
        let now = chrono::Utc::now();
        sqlx::query(
            r#"
            INSERT INTO org_sso_configs (
                id, org_id, issuer, client_id, client_secret_key, enabled, created_at, updated_at
            )
            VALUES (?, ?, 'https://idp.example.com', 'client-id', 'secret-key', 1, ?, ?)
            "#,
        )
        .bind(id.to_string())
        .bind(org_id.to_string())
        .bind(now)
        .bind(now)
        .execute(pool)
        .await
        .expect("Failed to create test SSO config");
        id
    }

    #[tokio::test]
    async fn test_create_domain_verification() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let sso_config_id = create_test_sso_config(&pool, org_id).await;
        let repo = SqliteDomainVerificationRepo::new(pool);

        let input = CreateDomainVerification {
            domain: "acme.com".to_string(),
        };
        let verification = repo
            .create(sso_config_id, input, "test-token-123")
            .await
            .expect("Failed to create verification");

        assert_eq!(verification.org_sso_config_id, sso_config_id);
        assert_eq!(verification.domain, "acme.com");
        assert_eq!(verification.verification_token, "test-token-123");
        assert_eq!(verification.status, DomainVerificationStatus::Pending);
        assert_eq!(verification.verification_attempts, 0);
    }

    #[tokio::test]
    async fn test_create_duplicate_domain_fails() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let sso_config_id = create_test_sso_config(&pool, org_id).await;
        let repo = SqliteDomainVerificationRepo::new(pool);

        let input = CreateDomainVerification {
            domain: "acme.com".to_string(),
        };
        repo.create(sso_config_id, input.clone(), "token1")
            .await
            .expect("First create should succeed");

        let result = repo.create(sso_config_id, input, "token2").await;
        assert!(matches!(result, Err(DbError::Conflict(_))));
    }

    #[tokio::test]
    async fn test_get_by_id() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let sso_config_id = create_test_sso_config(&pool, org_id).await;
        let repo = SqliteDomainVerificationRepo::new(pool);

        let input = CreateDomainVerification {
            domain: "acme.com".to_string(),
        };
        let created = repo
            .create(sso_config_id, input, "token")
            .await
            .expect("Failed to create");

        let fetched = repo
            .get_by_id(created.id)
            .await
            .expect("Failed to get")
            .expect("Should exist");

        assert_eq!(fetched.id, created.id);
        assert_eq!(fetched.domain, "acme.com");
    }

    #[tokio::test]
    async fn test_get_by_config_and_domain() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let sso_config_id = create_test_sso_config(&pool, org_id).await;
        let repo = SqliteDomainVerificationRepo::new(pool);

        let input = CreateDomainVerification {
            domain: "acme.com".to_string(),
        };
        let created = repo
            .create(sso_config_id, input, "token")
            .await
            .expect("Failed to create");

        let fetched = repo
            .get_by_config_and_domain(sso_config_id, "acme.com")
            .await
            .expect("Failed to get")
            .expect("Should exist");

        assert_eq!(fetched.id, created.id);

        // Not found for different domain
        let not_found = repo
            .get_by_config_and_domain(sso_config_id, "other.com")
            .await
            .expect("Failed to get");
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_list_by_config() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let sso_config_id = create_test_sso_config(&pool, org_id).await;
        let repo = SqliteDomainVerificationRepo::new(pool);

        repo.create(
            sso_config_id,
            CreateDomainVerification {
                domain: "acme.com".to_string(),
            },
            "token1",
        )
        .await
        .expect("Failed to create");

        repo.create(
            sso_config_id,
            CreateDomainVerification {
                domain: "acme.io".to_string(),
            },
            "token2",
        )
        .await
        .expect("Failed to create");

        let list = repo
            .list_by_config(sso_config_id, ListParams::default())
            .await
            .expect("Failed to list");

        assert_eq!(list.len(), 2);
    }

    #[tokio::test]
    async fn test_update_verification() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let sso_config_id = create_test_sso_config(&pool, org_id).await;
        let repo = SqliteDomainVerificationRepo::new(pool);

        let input = CreateDomainVerification {
            domain: "acme.com".to_string(),
        };
        let created = repo
            .create(sso_config_id, input, "token")
            .await
            .expect("Failed to create");

        let now = chrono::Utc::now();
        let update = UpdateDomainVerification {
            status: Some(DomainVerificationStatus::Verified),
            dns_txt_record: Some(Some("hadrian-verify=token".to_string())),
            verification_attempts: Some(1),
            last_attempt_at: Some(Some(now)),
            verified_at: Some(Some(now)),
            expires_at: None,
        };

        let updated = repo
            .update(created.id, update)
            .await
            .expect("Failed to update");

        assert_eq!(updated.status, DomainVerificationStatus::Verified);
        assert_eq!(
            updated.dns_txt_record,
            Some("hadrian-verify=token".to_string())
        );
        assert_eq!(updated.verification_attempts, 1);
        assert!(updated.verified_at.is_some());
    }

    #[tokio::test]
    async fn test_delete_verification() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let sso_config_id = create_test_sso_config(&pool, org_id).await;
        let repo = SqliteDomainVerificationRepo::new(pool);

        let input = CreateDomainVerification {
            domain: "acme.com".to_string(),
        };
        let created = repo
            .create(sso_config_id, input, "token")
            .await
            .expect("Failed to create");

        repo.delete(created.id).await.expect("Failed to delete");

        let result = repo
            .get_by_id(created.id)
            .await
            .expect("Query should succeed");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_find_verified_by_domain() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let sso_config_id = create_test_sso_config(&pool, org_id).await;
        let repo = SqliteDomainVerificationRepo::new(pool);

        let input = CreateDomainVerification {
            domain: "acme.com".to_string(),
        };
        let created = repo
            .create(sso_config_id, input, "token")
            .await
            .expect("Failed to create");

        // Not verified yet
        let not_found = repo
            .find_verified_by_domain("acme.com")
            .await
            .expect("Failed to search");
        assert!(not_found.is_none());

        // Mark as verified
        let update = UpdateDomainVerification {
            status: Some(DomainVerificationStatus::Verified),
            verified_at: Some(Some(chrono::Utc::now())),
            ..Default::default()
        };
        repo.update(created.id, update)
            .await
            .expect("Failed to update");

        // Now should be found
        let found = repo
            .find_verified_by_domain("acme.com")
            .await
            .expect("Failed to search")
            .expect("Should find");
        assert_eq!(found.id, created.id);
    }

    #[tokio::test]
    async fn test_has_verified_domain() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let sso_config_id = create_test_sso_config(&pool, org_id).await;
        let repo = SqliteDomainVerificationRepo::new(pool);

        // No verifications yet
        let has_verified = repo
            .has_verified_domain(sso_config_id)
            .await
            .expect("Failed to check");
        assert!(!has_verified);

        let input = CreateDomainVerification {
            domain: "acme.com".to_string(),
        };
        let created = repo
            .create(sso_config_id, input, "token")
            .await
            .expect("Failed to create");

        // Still pending
        let has_verified = repo
            .has_verified_domain(sso_config_id)
            .await
            .expect("Failed to check");
        assert!(!has_verified);

        // Mark as verified
        let update = UpdateDomainVerification {
            status: Some(DomainVerificationStatus::Verified),
            verified_at: Some(Some(chrono::Utc::now())),
            ..Default::default()
        };
        repo.update(created.id, update)
            .await
            .expect("Failed to update");

        // Now has verified
        let has_verified = repo
            .has_verified_domain(sso_config_id)
            .await
            .expect("Failed to check");
        assert!(has_verified);
    }
}
