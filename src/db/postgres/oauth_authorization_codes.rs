use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::{
    db::{
        error::{DbError, DbResult},
        repos::{NewAuthorizationCode, OAuthAuthorizationCodeRepo, truncate_to_millis},
    },
    models::{OAuthAuthorizationCode, OAuthKeyOptions, PkceCodeChallengeMethod},
};

pub struct PostgresOAuthAuthorizationCodeRepo {
    write_pool: PgPool,
}

impl PostgresOAuthAuthorizationCodeRepo {
    pub fn new(write_pool: PgPool, _read_pool: Option<PgPool>) -> Self {
        // All operations on this repo (insert, consume = UPDATE, delete_stale)
        // are writes, so we ignore the read replica pool here.
        Self { write_pool }
    }
}

fn parse_method(s: &str) -> DbResult<PkceCodeChallengeMethod> {
    s.parse().map_err(DbError::Internal)
}

fn row_to_code(row: &sqlx::postgres::PgRow) -> DbResult<OAuthAuthorizationCode> {
    let key_options_value: serde_json::Value = row.get("key_options");
    let key_options: OAuthKeyOptions = serde_json::from_value(key_options_value)?;
    Ok(OAuthAuthorizationCode {
        id: row.get("id"),
        code: row.get("code"),
        code_challenge: row.get("code_challenge"),
        code_challenge_method: parse_method(&row.get::<String, _>("code_challenge_method"))?,
        callback_url: row.get("callback_url"),
        user_id: row.get("user_id"),
        app_name: row.get("app_name"),
        key_options,
        expires_at: row.get("expires_at"),
        used_at: row.get("used_at"),
        created_at: row.get("created_at"),
    })
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl OAuthAuthorizationCodeRepo for PostgresOAuthAuthorizationCodeRepo {
    async fn insert(&self, input: NewAuthorizationCode) -> DbResult<OAuthAuthorizationCode> {
        let id = Uuid::new_v4();
        let now = truncate_to_millis(Utc::now());
        let key_options_json = serde_json::to_value(&input.key_options)?;

        sqlx::query(
            r#"
            INSERT INTO oauth_authorization_codes (
                id, code, code_challenge, code_challenge_method,
                callback_url, user_id, app_name, key_options,
                expires_at, created_at
            )
            VALUES ($1, $2, $3, $4::oauth_pkce_method, $5, $6, $7, $8, $9, $10)
            "#,
        )
        .bind(id)
        .bind(&input.code)
        .bind(&input.code_challenge)
        .bind(input.code_challenge_method.as_str())
        .bind(&input.callback_url)
        .bind(input.user_id)
        .bind(&input.app_name)
        .bind(&key_options_json)
        .bind(input.expires_at)
        .bind(now)
        .execute(&self.write_pool)
        .await?;

        Ok(OAuthAuthorizationCode {
            id,
            code: input.code,
            code_challenge: input.code_challenge,
            code_challenge_method: input.code_challenge_method,
            callback_url: input.callback_url,
            user_id: input.user_id,
            app_name: input.app_name,
            key_options: input.key_options,
            expires_at: input.expires_at,
            used_at: None,
            created_at: now,
        })
    }

    async fn lookup_active(&self, code: &str) -> DbResult<Option<OAuthAuthorizationCode>> {
        let now = truncate_to_millis(Utc::now());
        let result = sqlx::query(
            r#"
            SELECT id, code, code_challenge,
                   code_challenge_method::text AS code_challenge_method,
                   callback_url, user_id, app_name, key_options,
                   expires_at, used_at, created_at
            FROM oauth_authorization_codes
            WHERE code = $1 AND used_at IS NULL AND expires_at > $2
            "#,
        )
        .bind(code)
        .bind(now)
        .fetch_optional(&self.write_pool)
        .await?;

        match result {
            Some(row) => Ok(Some(row_to_code(&row)?)),
            None => Ok(None),
        }
    }

    async fn consume(&self, code: &str) -> DbResult<Option<OAuthAuthorizationCode>> {
        let now = truncate_to_millis(Utc::now());

        // Atomic claim: only succeeds if the code is unused and unexpired.
        let result = sqlx::query(
            r#"
            UPDATE oauth_authorization_codes
            SET used_at = $1
            WHERE code = $2 AND used_at IS NULL AND expires_at > $3
            RETURNING id, code, code_challenge,
                      code_challenge_method::text AS code_challenge_method,
                      callback_url, user_id, app_name, key_options,
                      expires_at, used_at, created_at
            "#,
        )
        .bind(now)
        .bind(code)
        .bind(now)
        .fetch_optional(&self.write_pool)
        .await?;

        match result {
            Some(row) => Ok(Some(row_to_code(&row)?)),
            None => Ok(None),
        }
    }

    async fn delete_stale(&self, before: DateTime<Utc>) -> DbResult<u64> {
        let result = sqlx::query(
            r#"
            DELETE FROM oauth_authorization_codes
            WHERE expires_at < $1 OR used_at IS NOT NULL
            "#,
        )
        .bind(before)
        .execute(&self.write_pool)
        .await?;
        Ok(result.rows_affected())
    }
}
