//! SQLite implementation of [`ResponsesRepo`].

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;
use uuid::Uuid;

use super::{
    backend::{Pool, RowExt, query},
    common::parse_uuid,
};
use crate::db::{
    error::DbResult,
    repos::{
        NewResponse, ResponseCompletion, ResponseRecord, ResponseStatus, ResponsesRepo,
        truncate_to_millis,
    },
};

pub struct SqliteResponsesRepo {
    pool: Pool,
}

impl SqliteResponsesRepo {
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }
}

fn parse_status(s: &str) -> DbResult<ResponseStatus> {
    ResponseStatus::parse(s)
        .ok_or_else(|| crate::db::error::DbError::Internal(format!("unknown response status: {s}")))
}

fn parse_json(s: Option<String>) -> DbResult<Option<Value>> {
    match s {
        Some(s) => Ok(Some(serde_json::from_str(&s)?)),
        None => Ok(None),
    }
}

fn parse_optional_uuid(s: Option<String>) -> DbResult<Option<Uuid>> {
    s.map(|s| parse_uuid(&s)).transpose()
}

fn row_to_record(row: &super::backend::Row) -> DbResult<ResponseRecord> {
    let request_payload: String = row.col("request_payload");
    Ok(ResponseRecord {
        id: row.col("id"),
        org_id: parse_optional_uuid(row.col("org_id"))?,
        project_id: parse_optional_uuid(row.col("project_id"))?,
        user_id: parse_optional_uuid(row.col("user_id"))?,
        api_key_id: parse_optional_uuid(row.col("api_key_id"))?,
        service_account_id: parse_optional_uuid(row.col("service_account_id"))?,
        status: parse_status(&row.col::<String>("status"))?,
        background: row.col::<i64>("background") != 0,
        model: row.col("model"),
        provider: row.col("provider"),
        created_at: row.col("created_at"),
        started_at: row.col("started_at"),
        completed_at: row.col("completed_at"),
        request_payload: serde_json::from_str(&request_payload)?,
        output: parse_json(row.col("output"))?,
        usage: parse_json(row.col("usage"))?,
        error: parse_json(row.col("error"))?,
        retention_expires_at: row.col("retention_expires_at"),
        last_sequence_number: row.col("last_sequence_number"),
    })
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl ResponsesRepo for SqliteResponsesRepo {
    async fn insert(&self, input: NewResponse) -> DbResult<ResponseRecord> {
        let created_at = truncate_to_millis(input.created_at);
        let retention_expires_at = truncate_to_millis(input.retention_expires_at);
        let request_payload_json = serde_json::to_string(&input.request_payload)?;

        query(
            r#"
            INSERT INTO responses (
                id, org_id, project_id, user_id, api_key_id, service_account_id,
                status, background, model, provider,
                created_at, request_payload, retention_expires_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&input.id)
        .bind(input.org_id.map(|id| id.to_string()))
        .bind(input.project_id.map(|id| id.to_string()))
        .bind(input.user_id.map(|id| id.to_string()))
        .bind(input.api_key_id.map(|id| id.to_string()))
        .bind(input.service_account_id.map(|id| id.to_string()))
        .bind(input.status.as_str())
        .bind(input.background as i64)
        .bind(&input.model)
        .bind(&input.provider)
        .bind(created_at)
        .bind(&request_payload_json)
        .bind(retention_expires_at)
        .execute(&self.pool)
        .await?;

        Ok(ResponseRecord {
            id: input.id,
            org_id: input.org_id,
            project_id: input.project_id,
            user_id: input.user_id,
            api_key_id: input.api_key_id,
            service_account_id: input.service_account_id,
            status: input.status,
            background: input.background,
            model: input.model,
            provider: input.provider,
            created_at,
            started_at: None,
            completed_at: None,
            request_payload: input.request_payload,
            output: None,
            usage: None,
            error: None,
            retention_expires_at,
            last_sequence_number: 0,
        })
    }

    async fn get_by_id_and_org(
        &self,
        id: &str,
        org_id: Option<Uuid>,
    ) -> DbResult<Option<ResponseRecord>> {
        let result = query(
            r#"
            SELECT id, org_id, project_id, user_id, api_key_id, service_account_id,
                   status, background, model, provider,
                   created_at, started_at, completed_at,
                   request_payload, output, usage, error,
                   retention_expires_at, last_sequence_number
            FROM responses
            WHERE id = ? AND (org_id IS ? OR org_id = ?)
            "#,
        )
        .bind(id)
        .bind(org_id.map(|id| id.to_string()))
        .bind(org_id.map(|id| id.to_string()))
        .fetch_optional(&self.pool)
        .await?;
        match result {
            Some(row) => Ok(Some(row_to_record(&row)?)),
            None => Ok(None),
        }
    }

    async fn update(
        &self,
        id: &str,
        patch: ResponseCompletion,
    ) -> DbResult<Option<ResponseRecord>> {
        // Build the SET clause dynamically. SQLite handles this fine
        // with one bind per Some field.
        let mut setters: Vec<&str> = Vec::new();
        if patch.status.is_some() {
            setters.push("status = ?");
        }
        if patch.started_at.is_some() {
            setters.push("started_at = ?");
        }
        if patch.completed_at.is_some() {
            setters.push("completed_at = ?");
        }
        if patch.output.is_some() {
            setters.push("output = ?");
        }
        if patch.usage.is_some() {
            setters.push("usage = ?");
        }
        if patch.error.is_some() {
            setters.push("error = ?");
        }
        if patch.retention_expires_at.is_some() {
            setters.push("retention_expires_at = ?");
        }
        if setters.is_empty() {
            return self.get_by_id_and_org(id, None).await;
        }

        let sql = format!(
            "UPDATE responses SET {} WHERE id = ? RETURNING \
             id, org_id, project_id, user_id, api_key_id, service_account_id, \
             status, background, model, provider, \
             created_at, started_at, completed_at, \
             request_payload, output, usage, error, retention_expires_at, last_sequence_number",
            setters.join(", ")
        );
        let mut q = query(&sql);
        if let Some(status) = patch.status {
            q = q.bind(status.as_str().to_string());
        }
        if let Some(ts) = patch.started_at {
            q = q.bind(truncate_to_millis(ts));
        }
        if let Some(ts) = patch.completed_at {
            q = q.bind(truncate_to_millis(ts));
        }
        if let Some(output) = patch.output {
            q = q.bind(serde_json::to_string(&output)?);
        }
        if let Some(usage) = patch.usage {
            q = q.bind(serde_json::to_string(&usage)?);
        }
        if let Some(error) = patch.error {
            q = q.bind(serde_json::to_string(&error)?);
        }
        if let Some(ts) = patch.retention_expires_at {
            q = q.bind(truncate_to_millis(ts));
        }
        q = q.bind(id);

        let result = q.fetch_optional(&self.pool).await?;
        match result {
            Some(row) => Ok(Some(row_to_record(&row)?)),
            None => Ok(None),
        }
    }

    async fn delete_by_id_and_org(&self, id: &str, org_id: Option<Uuid>) -> DbResult<bool> {
        let result = query(
            r#"
            DELETE FROM responses
            WHERE id = ? AND (org_id IS ? OR org_id = ?)
            "#,
        )
        .bind(id)
        .bind(org_id.map(|id| id.to_string()))
        .bind(org_id.map(|id| id.to_string()))
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn claim_queued(&self, now: DateTime<Utc>) -> DbResult<Option<ResponseRecord>> {
        let now = truncate_to_millis(now);
        // SQLite serialises writes, so a plain UPDATE...RETURNING with
        // a subselect of one row gives atomic claim semantics: the
        // first transaction wins, the rest see status != 'queued' and
        // get no rows back.
        let result = query(
            r#"
            UPDATE responses
            SET status = 'in_progress', started_at = ?
            WHERE id = (
                SELECT id FROM responses
                WHERE status = 'queued'
                ORDER BY created_at ASC
                LIMIT 1
            )
            RETURNING
                id, org_id, project_id, user_id, api_key_id, service_account_id,
                status, background, model, provider,
                created_at, started_at, completed_at,
                request_payload, output, usage, error,
                retention_expires_at, last_sequence_number
            "#,
        )
        .bind(now)
        .fetch_optional(&self.pool)
        .await?;
        match result {
            Some(row) => Ok(Some(row_to_record(&row)?)),
            None => Ok(None),
        }
    }

    async fn delete_expired(&self, before: DateTime<Utc>) -> DbResult<u64> {
        let before = truncate_to_millis(before);
        let result = query(
            r#"
            DELETE FROM responses
            WHERE retention_expires_at < ?
              AND status IN ('completed', 'failed', 'cancelled', 'incomplete')
            "#,
        )
        .bind(before)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }
}
