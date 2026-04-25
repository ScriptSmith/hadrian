use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::{
    backend::{Pool, RowExt, query},
    common::parse_uuid,
};
use crate::{
    db::{
        error::{DbError, DbResult},
        repos::{NewAuthorizationCode, OAuthAuthorizationCodeRepo, truncate_to_millis},
    },
    models::{OAuthAuthorizationCode, OAuthKeyOptions, PkceCodeChallengeMethod},
};

pub struct SqliteOAuthAuthorizationCodeRepo {
    pool: Pool,
}

impl SqliteOAuthAuthorizationCodeRepo {
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }
}

fn parse_method(s: &str) -> DbResult<PkceCodeChallengeMethod> {
    s.parse().map_err(DbError::Internal)
}

fn row_to_code(row: &super::backend::Row) -> DbResult<OAuthAuthorizationCode> {
    let key_options_str: String = row.col("key_options");
    let key_options: OAuthKeyOptions = serde_json::from_str(&key_options_str)?;
    Ok(OAuthAuthorizationCode {
        id: parse_uuid(&row.col::<String>("id"))?,
        code: row.col("code"),
        code_challenge: row.col("code_challenge"),
        code_challenge_method: parse_method(&row.col::<String>("code_challenge_method"))?,
        callback_url: row.col("callback_url"),
        user_id: parse_uuid(&row.col::<String>("user_id"))?,
        app_name: row.col("app_name"),
        key_options,
        expires_at: row.col("expires_at"),
        used_at: row.col("used_at"),
        created_at: row.col("created_at"),
    })
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl OAuthAuthorizationCodeRepo for SqliteOAuthAuthorizationCodeRepo {
    async fn insert(&self, input: NewAuthorizationCode) -> DbResult<OAuthAuthorizationCode> {
        let id = Uuid::new_v4();
        let now = truncate_to_millis(Utc::now());
        let key_options_json = serde_json::to_string(&input.key_options)?;

        query(
            r#"
            INSERT INTO oauth_authorization_codes (
                id, code, code_challenge, code_challenge_method,
                callback_url, user_id, app_name, key_options,
                expires_at, created_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(id.to_string())
        .bind(&input.code)
        .bind(&input.code_challenge)
        .bind(input.code_challenge_method.as_str())
        .bind(&input.callback_url)
        .bind(input.user_id.to_string())
        .bind(&input.app_name)
        .bind(&key_options_json)
        .bind(input.expires_at)
        .bind(now)
        .execute(&self.pool)
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
        let result = query(
            r#"
            SELECT id, code, code_challenge, code_challenge_method,
                   callback_url, user_id, app_name, key_options,
                   expires_at, used_at, created_at
            FROM oauth_authorization_codes
            WHERE code = ? AND used_at IS NULL AND expires_at > ?
            "#,
        )
        .bind(code)
        .bind(now)
        .fetch_optional(&self.pool)
        .await?;

        match result {
            Some(row) => Ok(Some(row_to_code(&row)?)),
            None => Ok(None),
        }
    }

    async fn consume(&self, code: &str) -> DbResult<Option<OAuthAuthorizationCode>> {
        let now = truncate_to_millis(Utc::now());

        // Atomic claim: only succeeds if the code is unused and unexpired.
        let result = query(
            r#"
            UPDATE oauth_authorization_codes
            SET used_at = ?
            WHERE code = ? AND used_at IS NULL AND expires_at > ?
            RETURNING id, code, code_challenge, code_challenge_method,
                      callback_url, user_id, app_name, key_options,
                      expires_at, used_at, created_at
            "#,
        )
        .bind(now)
        .bind(code)
        .bind(now)
        .fetch_optional(&self.pool)
        .await?;

        match result {
            Some(row) => Ok(Some(row_to_code(&row)?)),
            None => Ok(None),
        }
    }

    async fn delete_stale(&self, before: DateTime<Utc>) -> DbResult<u64> {
        let result = query(
            r#"
            DELETE FROM oauth_authorization_codes
            WHERE expires_at < ? OR used_at IS NOT NULL
            "#,
        )
        .bind(before)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }
}
