use async_trait::async_trait;
use sqlx::{PgPool, Row};
use uuid::Uuid;

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

pub struct PostgresDomainVerificationRepo {
    write_pool: PgPool,
    read_pool: PgPool,
}

impl PostgresDomainVerificationRepo {
    pub fn new(write_pool: PgPool, read_pool: Option<PgPool>) -> Self {
        let read_pool = read_pool.unwrap_or_else(|| write_pool.clone());
        Self {
            write_pool,
            read_pool,
        }
    }

    /// Parse a DomainVerification from a database row.
    fn parse_verification(row: &sqlx::postgres::PgRow) -> DomainVerification {
        let status_str: String = row.get("status");
        let status = status_str
            .parse::<DomainVerificationStatus>()
            .unwrap_or_default();

        DomainVerification {
            id: row.get("id"),
            org_sso_config_id: row.get("org_sso_config_id"),
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
        }
    }
}

#[async_trait]
impl DomainVerificationRepo for PostgresDomainVerificationRepo {
    async fn create(
        &self,
        org_sso_config_id: Uuid,
        input: CreateDomainVerification,
        verification_token: &str,
    ) -> DbResult<DomainVerification> {
        let row = sqlx::query(
            r#"
            INSERT INTO domain_verifications (
                id, org_sso_config_id, domain, verification_token, status
            )
            VALUES ($1, $2, $3, $4, 'pending'::domain_verification_status)
            RETURNING id, org_sso_config_id, domain, verification_token, status::text,
                      dns_txt_record, verification_attempts, last_attempt_at, verified_at,
                      expires_at, created_at, updated_at
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(org_sso_config_id)
        .bind(&input.domain)
        .bind(verification_token)
        .fetch_one(&self.write_pool)
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

        Ok(Self::parse_verification(&row))
    }

    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<DomainVerification>> {
        let result = sqlx::query(
            r#"
            SELECT id, org_sso_config_id, domain, verification_token, status::text,
                   dns_txt_record, verification_attempts, last_attempt_at, verified_at,
                   expires_at, created_at, updated_at
            FROM domain_verifications
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.read_pool)
        .await?;

        Ok(result.map(|row| Self::parse_verification(&row)))
    }

    async fn get_by_config_and_domain(
        &self,
        org_sso_config_id: Uuid,
        domain: &str,
    ) -> DbResult<Option<DomainVerification>> {
        let result = sqlx::query(
            r#"
            SELECT id, org_sso_config_id, domain, verification_token, status::text,
                   dns_txt_record, verification_attempts, last_attempt_at, verified_at,
                   expires_at, created_at, updated_at
            FROM domain_verifications
            WHERE org_sso_config_id = $1 AND domain = $2
            "#,
        )
        .bind(org_sso_config_id)
        .bind(domain)
        .fetch_optional(&self.read_pool)
        .await?;

        Ok(result.map(|row| Self::parse_verification(&row)))
    }

    async fn list_by_config(
        &self,
        org_sso_config_id: Uuid,
        params: ListParams,
    ) -> DbResult<Vec<DomainVerification>> {
        let limit = params.limit.unwrap_or(100);

        let rows = sqlx::query(
            r#"
            SELECT id, org_sso_config_id, domain, verification_token, status::text,
                   dns_txt_record, verification_attempts, last_attempt_at, verified_at,
                   expires_at, created_at, updated_at
            FROM domain_verifications
            WHERE org_sso_config_id = $1
            ORDER BY created_at DESC, id DESC
            LIMIT $2
            "#,
        )
        .bind(org_sso_config_id)
        .bind(limit)
        .fetch_all(&self.read_pool)
        .await?;

        Ok(rows.iter().map(Self::parse_verification).collect())
    }

    async fn count_by_config(&self, org_sso_config_id: Uuid) -> DbResult<i64> {
        let row = sqlx::query(
            "SELECT COUNT(*) as count FROM domain_verifications WHERE org_sso_config_id = $1",
        )
        .bind(org_sso_config_id)
        .fetch_one(&self.read_pool)
        .await?;

        Ok(row.get::<i64, _>("count"))
    }

    async fn update(
        &self,
        id: Uuid,
        input: UpdateDomainVerification,
    ) -> DbResult<DomainVerification> {
        // Fetch existing record
        let existing = self.get_by_id(id).await?.ok_or(DbError::NotFound)?;

        let row = sqlx::query(
            r#"
            UPDATE domain_verifications SET
                status = $1::domain_verification_status,
                dns_txt_record = $2,
                verification_attempts = $3,
                last_attempt_at = $4,
                verified_at = $5,
                expires_at = $6,
                updated_at = NOW()
            WHERE id = $7
            RETURNING id, org_sso_config_id, domain, verification_token, status::text,
                      dns_txt_record, verification_attempts, last_attempt_at, verified_at,
                      expires_at, created_at, updated_at
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
        .bind(id)
        .fetch_one(&self.write_pool)
        .await?;

        Ok(Self::parse_verification(&row))
    }

    async fn delete(&self, id: Uuid) -> DbResult<()> {
        let result = sqlx::query("DELETE FROM domain_verifications WHERE id = $1")
            .bind(id)
            .execute(&self.write_pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }

        Ok(())
    }

    async fn find_verified_by_domain(&self, domain: &str) -> DbResult<Option<DomainVerification>> {
        let result = sqlx::query(
            r#"
            SELECT dv.id, dv.org_sso_config_id, dv.domain, dv.verification_token, dv.status::text,
                   dv.dns_txt_record, dv.verification_attempts, dv.last_attempt_at, dv.verified_at,
                   dv.expires_at, dv.created_at, dv.updated_at
            FROM domain_verifications dv
            JOIN org_sso_configs osc ON dv.org_sso_config_id = osc.id
            WHERE dv.domain = $1
              AND dv.status = 'verified'
              AND osc.enabled = TRUE
              AND (dv.expires_at IS NULL OR dv.expires_at > NOW())
            LIMIT 1
            "#,
        )
        .bind(domain)
        .fetch_optional(&self.read_pool)
        .await?;

        Ok(result.map(|row| Self::parse_verification(&row)))
    }

    async fn list_verified_by_config(
        &self,
        org_sso_config_id: Uuid,
    ) -> DbResult<Vec<DomainVerification>> {
        let rows = sqlx::query(
            r#"
            SELECT id, org_sso_config_id, domain, verification_token, status::text,
                   dns_txt_record, verification_attempts, last_attempt_at, verified_at,
                   expires_at, created_at, updated_at
            FROM domain_verifications
            WHERE org_sso_config_id = $1
              AND status = 'verified'
              AND (expires_at IS NULL OR expires_at > NOW())
            ORDER BY domain ASC
            "#,
        )
        .bind(org_sso_config_id)
        .fetch_all(&self.read_pool)
        .await?;

        Ok(rows.iter().map(Self::parse_verification).collect())
    }

    async fn has_verified_domain(&self, org_sso_config_id: Uuid) -> DbResult<bool> {
        let row = sqlx::query(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM domain_verifications
                WHERE org_sso_config_id = $1
                  AND status = 'verified'
                  AND (expires_at IS NULL OR expires_at > NOW())
            ) as has_verified
            "#,
        )
        .bind(org_sso_config_id)
        .fetch_one(&self.read_pool)
        .await?;

        Ok(row.get::<bool, _>("has_verified"))
    }

    async fn create_auto_verified(
        &self,
        org_sso_config_id: Uuid,
        input: CreateDomainVerification,
        verification_token: &str,
    ) -> DbResult<DomainVerification> {
        let row = sqlx::query(
            r#"
            INSERT INTO domain_verifications (
                id, org_sso_config_id, domain, verification_token, status,
                dns_txt_record, verified_at
            )
            VALUES ($1, $2, $3, $4, 'verified'::domain_verification_status,
                    'AUTO_VERIFIED_BY_BOOTSTRAP', NOW())
            RETURNING id, org_sso_config_id, domain, verification_token, status::text,
                      dns_txt_record, verification_attempts, last_attempt_at, verified_at,
                      expires_at, created_at, updated_at
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(org_sso_config_id)
        .bind(&input.domain)
        .bind(verification_token)
        .fetch_one(&self.write_pool)
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

        Ok(Self::parse_verification(&row))
    }
}
