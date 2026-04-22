use std::collections::HashMap;

use async_trait::async_trait;
use uuid::Uuid;

use super::{
    backend::{Pool, Row, RowExt, begin, map_unique_violation, query},
    common::parse_uuid,
};
use crate::{
    db::{
        error::{DbError, DbResult},
        repos::{
            Cursor, CursorDirection, ListParams, ListResult, PageCursors, SkillRepo,
            cursor_from_row, truncate_to_millis,
        },
    },
    models::{
        CreateSkill, Skill, SkillFile, SkillFileInput, SkillFileManifest, SkillOwnerType,
        UpdateSkill,
    },
};

const SKILL_COLUMNS: &str = "id, owner_type, owner_id, name, description, user_invocable, \
     disable_model_invocation, allowed_tools, argument_hint, source_url, source_ref, \
     frontmatter_extra, total_bytes, created_at, updated_at";

pub struct SqliteSkillRepo {
    pool: Pool,
}

impl SqliteSkillRepo {
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// Sniff a MIME type from a file path's extension. Falls back to
    /// `text/plain` for unknown extensions.
    fn sniff_content_type(path: &str) -> &'static str {
        let lower = path.to_ascii_lowercase();
        match lower.rsplit_once('.').map(|(_, ext)| ext) {
            Some("md") | Some("markdown") => "text/markdown",
            Some("py") => "text/x-python",
            Some("js") | Some("mjs") | Some("cjs") => "text/javascript",
            Some("ts") => "text/typescript",
            Some("sh") | Some("bash") => "text/x-shellscript",
            Some("json") => "application/json",
            Some("yaml") | Some("yml") => "application/yaml",
            Some("toml") => "application/toml",
            Some("html") | Some("htm") => "text/html",
            Some("css") => "text/css",
            Some("csv") => "text/csv",
            Some("txt") | None => "text/plain",
            _ => "text/plain",
        }
    }

    /// Parse a skill row (no files attached).
    fn parse_skill(row: &Row) -> DbResult<Skill> {
        let owner_type_str: String = row.col("owner_type");
        let owner_type: SkillOwnerType = owner_type_str.parse().map_err(DbError::Internal)?;

        let allowed_tools: Option<String> = row.col("allowed_tools");
        let allowed_tools: Option<Vec<String>> = allowed_tools
            .map(|s| serde_json::from_str(&s))
            .transpose()
            .map_err(|e| DbError::Internal(format!("Failed to parse allowed_tools: {}", e)))?;

        let frontmatter_extra: Option<String> = row.col("frontmatter_extra");
        let frontmatter_extra: Option<HashMap<String, serde_json::Value>> = frontmatter_extra
            .map(|s| serde_json::from_str(&s))
            .transpose()
            .map_err(|e| {
                DbError::Internal(format!("Failed to parse frontmatter_extra: {}", e))
            })?;

        let user_invocable: Option<i64> = row.col("user_invocable");
        let disable_model_invocation: Option<i64> = row.col("disable_model_invocation");

        Ok(Skill {
            id: parse_uuid(&row.col::<String>("id"))?,
            owner_type,
            owner_id: parse_uuid(&row.col::<String>("owner_id"))?,
            name: row.col("name"),
            description: row.col("description"),
            user_invocable: user_invocable.map(|n| n != 0),
            disable_model_invocation: disable_model_invocation.map(|n| n != 0),
            allowed_tools,
            argument_hint: row.col("argument_hint"),
            source_url: row.col("source_url"),
            source_ref: row.col("source_ref"),
            frontmatter_extra,
            total_bytes: row.col("total_bytes"),
            files: Vec::new(),
            files_manifest: Vec::new(),
            created_at: row.col("created_at"),
            updated_at: row.col("updated_at"),
        })
    }

    fn parse_file(row: &Row) -> SkillFile {
        SkillFile {
            path: row.col("path"),
            content: row.col("content"),
            byte_size: row.col("byte_size"),
            content_type: row.col("content_type"),
            created_at: row.col("created_at"),
            updated_at: row.col("updated_at"),
        }
    }

    fn parse_manifest(row: &Row) -> SkillFileManifest {
        SkillFileManifest {
            path: row.col("path"),
            byte_size: row.col("byte_size"),
            content_type: row.col("content_type"),
        }
    }

    /// Load all files for a single skill, sorted by path.
    async fn load_files(&self, skill_id: Uuid) -> DbResult<Vec<SkillFile>> {
        let rows = query(
            r#"
            SELECT path, content, byte_size, content_type, created_at, updated_at
            FROM skill_files
            WHERE skill_id = ?
            ORDER BY path ASC
            "#,
        )
        .bind(skill_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.iter().map(Self::parse_file).collect())
    }

    /// Load file manifests for many skills and attach them. Used by list
    /// endpoints so each returned skill has `files_manifest` populated.
    async fn attach_manifests(&self, skills: &mut [Skill]) -> DbResult<()> {
        if skills.is_empty() {
            return Ok(());
        }

        // Build the IN-clause with placeholders matching the skill count.
        let placeholders = std::iter::repeat_n("?", skills.len())
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT skill_id, path, byte_size, content_type FROM skill_files \
             WHERE skill_id IN ({}) ORDER BY path ASC",
            placeholders
        );

        let mut q = query(&sql);
        for skill in skills.iter() {
            q = q.bind(skill.id.to_string());
        }
        let rows = q.fetch_all(&self.pool).await?;

        let mut by_skill: HashMap<Uuid, Vec<SkillFileManifest>> = HashMap::new();
        for row in rows.iter() {
            let skill_id = parse_uuid(&row.col::<String>("skill_id"))?;
            by_skill
                .entry(skill_id)
                .or_default()
                .push(Self::parse_manifest(row));
        }

        for skill in skills.iter_mut() {
            if let Some(manifest) = by_skill.remove(&skill.id) {
                skill.files_manifest = manifest;
            }
        }
        Ok(())
    }

    /// Org-scoped WHERE clause for skills reachable within an organization.
    const ORG_SCOPE_FILTER: &'static str = r#"
        AND (
            (s.owner_type = 'organization' AND s.owner_id = ?)
            OR
            (s.owner_type = 'team' AND EXISTS (
                SELECT 1 FROM teams t WHERE t.id = s.owner_id AND t.org_id = ?
            ))
            OR
            (s.owner_type = 'project' AND EXISTS (
                SELECT 1 FROM projects pr WHERE pr.id = s.owner_id AND pr.org_id = ?
            ))
            OR
            (s.owner_type = 'user' AND EXISTS (
                SELECT 1 FROM org_memberships om WHERE om.user_id = s.owner_id AND om.org_id = ?
            ))
        )
    "#;

    async fn list_by_owner_with_cursor(
        &self,
        owner_type: SkillOwnerType,
        owner_id: Uuid,
        params: &ListParams,
        cursor: &Cursor,
        fetch_limit: i64,
        limit: i64,
    ) -> DbResult<ListResult<Skill>> {
        let (comparison, order, should_reverse) =
            params.sort_order.cursor_query_params(params.direction);

        let deleted_filter = if params.include_deleted {
            ""
        } else {
            "AND deleted_at IS NULL"
        };

        let sql = format!(
            "SELECT {cols} FROM skills \
             WHERE owner_type = ? AND owner_id = ? AND (created_at, id) {cmp} (?, ?) \
             {deleted_filter} \
             ORDER BY created_at {order}, id {order} LIMIT ?",
            cols = SKILL_COLUMNS,
            cmp = comparison,
            deleted_filter = deleted_filter,
            order = order,
        );

        let rows = query(&sql)
            .bind(owner_type.as_str())
            .bind(owner_id.to_string())
            .bind(cursor.created_at)
            .bind(cursor.id.to_string())
            .bind(fetch_limit)
            .fetch_all(&self.pool)
            .await?;

        let has_more = rows.len() as i64 > limit;
        let mut items: Vec<Skill> = rows
            .iter()
            .take(limit as usize)
            .map(Self::parse_skill)
            .collect::<DbResult<Vec<_>>>()?;

        if should_reverse {
            items.reverse();
        }
        self.attach_manifests(&mut items).await?;

        let cursors =
            PageCursors::from_items(&items, has_more, params.direction, Some(cursor), |s| {
                cursor_from_row(s.created_at, s.id)
            });

        Ok(ListResult::new(items, has_more, cursors))
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl SkillRepo for SqliteSkillRepo {
    async fn create(&self, input: CreateSkill) -> DbResult<Skill> {
        let id = Uuid::new_v4();
        let now = truncate_to_millis(chrono::Utc::now());
        let owner_type = input.owner.owner_type();
        let owner_id = input.owner.owner_id();

        let allowed_tools_json = input
            .allowed_tools
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|e| DbError::Internal(format!("Failed to serialize allowed_tools: {}", e)))?;
        let frontmatter_extra_json = input
            .frontmatter_extra
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|e| {
                DbError::Internal(format!("Failed to serialize frontmatter_extra: {}", e))
            })?;

        // Compute byte sizes up front so total_bytes matches the file rows.
        let files_with_size: Vec<(SkillFileInput, i64, String)> = input
            .files
            .iter()
            .map(|f| {
                let size = f.content.len() as i64;
                let ct = f
                    .content_type
                    .clone()
                    .unwrap_or_else(|| Self::sniff_content_type(&f.path).to_string());
                (f.clone(), size, ct)
            })
            .collect();
        let total_bytes: i64 = files_with_size.iter().map(|(_, s, _)| *s).sum();

        let mut tx = begin(&self.pool).await?;

        query(
            r#"
            INSERT INTO skills (
                id, owner_type, owner_id, name, description,
                user_invocable, disable_model_invocation, allowed_tools,
                argument_hint, source_url, source_ref, frontmatter_extra,
                total_bytes, created_at, updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(id.to_string())
        .bind(owner_type.as_str())
        .bind(owner_id.to_string())
        .bind(&input.name)
        .bind(&input.description)
        .bind(input.user_invocable.map(|b| if b { 1i64 } else { 0i64 }))
        .bind(
            input
                .disable_model_invocation
                .map(|b| if b { 1i64 } else { 0i64 }),
        )
        .bind(&allowed_tools_json)
        .bind(&input.argument_hint)
        .bind(&input.source_url)
        .bind(&input.source_ref)
        .bind(&frontmatter_extra_json)
        .bind(total_bytes)
        .bind(now)
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(map_unique_violation(format!(
            "Skill with name '{}' already exists for this owner",
            input.name
        )))?;

        for (file, size, content_type) in files_with_size.iter() {
            query(
                r#"
                INSERT INTO skill_files (
                    skill_id, path, content, byte_size, content_type,
                    created_at, updated_at
                )
                VALUES (?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(id.to_string())
            .bind(&file.path)
            .bind(&file.content)
            .bind(*size)
            .bind(content_type)
            .bind(now)
            .bind(now)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;

        let mut skill = self
            .get_by_id(id)
            .await?
            .ok_or_else(|| DbError::Internal("Skill vanished after create".into()))?;
        // get_by_id already populates files; belt and braces:
        if skill.files.is_empty() {
            skill.files = self.load_files(id).await?;
        }
        Ok(skill)
    }

    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<Skill>> {
        let sql = format!(
            "SELECT {cols} FROM skills WHERE id = ? AND deleted_at IS NULL",
            cols = SKILL_COLUMNS
        );
        let result = query(&sql)
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await?;

        match result {
            Some(row) => {
                let mut skill = Self::parse_skill(&row)?;
                skill.files = self.load_files(id).await?;
                Ok(Some(skill))
            }
            None => Ok(None),
        }
    }

    async fn get_by_id_and_org(&self, id: Uuid, org_id: Uuid) -> DbResult<Option<Skill>> {
        let sql = format!(
            "SELECT {cols} FROM skills s \
             WHERE s.id = ? AND s.deleted_at IS NULL {scope}",
            cols = SKILL_COLUMNS
                .split(", ")
                .map(|c| format!("s.{}", c))
                .collect::<Vec<_>>()
                .join(", "),
            scope = Self::ORG_SCOPE_FILTER,
        );
        let result = query(&sql)
            .bind(id.to_string())
            .bind(org_id.to_string())
            .bind(org_id.to_string())
            .bind(org_id.to_string())
            .bind(org_id.to_string())
            .fetch_optional(&self.pool)
            .await?;

        match result {
            Some(row) => {
                let mut skill = Self::parse_skill(&row)?;
                skill.files = self.load_files(id).await?;
                Ok(Some(skill))
            }
            None => Ok(None),
        }
    }

    async fn list_by_owner(
        &self,
        owner_type: SkillOwnerType,
        owner_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<Skill>> {
        let limit = params.limit.unwrap_or(100);
        let fetch_limit = limit + 1;

        if let Some(ref cursor) = params.cursor {
            return self
                .list_by_owner_with_cursor(owner_type, owner_id, &params, cursor, fetch_limit, limit)
                .await;
        }

        let deleted_filter = if params.include_deleted {
            ""
        } else {
            "AND deleted_at IS NULL"
        };
        let sql = format!(
            "SELECT {cols} FROM skills \
             WHERE owner_type = ? AND owner_id = ? {deleted_filter} \
             ORDER BY created_at DESC, id DESC LIMIT ?",
            cols = SKILL_COLUMNS,
            deleted_filter = deleted_filter
        );

        let rows = query(&sql)
            .bind(owner_type.as_str())
            .bind(owner_id.to_string())
            .bind(fetch_limit)
            .fetch_all(&self.pool)
            .await?;

        let has_more = rows.len() as i64 > limit;
        let mut items: Vec<Skill> = rows
            .iter()
            .take(limit as usize)
            .map(Self::parse_skill)
            .collect::<DbResult<Vec<_>>>()?;
        self.attach_manifests(&mut items).await?;

        let cursors =
            PageCursors::from_items(&items, has_more, CursorDirection::Forward, None, |s| {
                cursor_from_row(s.created_at, s.id)
            });

        Ok(ListResult::new(items, has_more, cursors))
    }

    async fn list_by_org(
        &self,
        org_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<Skill>> {
        let limit = params.limit.unwrap_or(100);
        let fetch_limit = limit + 1;
        let org_str = org_id.to_string();

        // Prefix every core column with `s.` for the aliased query.
        let cols: String = SKILL_COLUMNS
            .split(", ")
            .map(|c| format!("s.{}", c))
            .collect::<Vec<_>>()
            .join(", ");

        if let Some(ref cursor) = params.cursor {
            let (comparison, order, should_reverse) =
                params.sort_order.cursor_query_params(params.direction);

            let sql = format!(
                "SELECT {cols} FROM skills s \
                 WHERE s.deleted_at IS NULL AND (s.created_at, s.id) {cmp} (?, ?) \
                 {scope} \
                 ORDER BY s.created_at {order}, s.id {order} LIMIT ?",
                cols = cols,
                cmp = comparison,
                scope = Self::ORG_SCOPE_FILTER,
                order = order,
            );

            let rows = query(&sql)
                .bind(cursor.created_at)
                .bind(cursor.id.to_string())
                .bind(&org_str)
                .bind(&org_str)
                .bind(&org_str)
                .bind(&org_str)
                .bind(fetch_limit)
                .fetch_all(&self.pool)
                .await?;

            let has_more = rows.len() as i64 > limit;
            let mut items: Vec<Skill> = rows
                .iter()
                .take(limit as usize)
                .map(Self::parse_skill)
                .collect::<DbResult<Vec<_>>>()?;

            if should_reverse {
                items.reverse();
            }
            self.attach_manifests(&mut items).await?;

            let cursors =
                PageCursors::from_items(&items, has_more, params.direction, Some(cursor), |s| {
                    cursor_from_row(s.created_at, s.id)
                });

            return Ok(ListResult::new(items, has_more, cursors));
        }

        let sql = format!(
            "SELECT {cols} FROM skills s \
             WHERE s.deleted_at IS NULL {scope} \
             ORDER BY s.created_at DESC, s.id DESC LIMIT ?",
            cols = cols,
            scope = Self::ORG_SCOPE_FILTER,
        );

        let rows = query(&sql)
            .bind(&org_str)
            .bind(&org_str)
            .bind(&org_str)
            .bind(&org_str)
            .bind(fetch_limit)
            .fetch_all(&self.pool)
            .await?;

        let has_more = rows.len() as i64 > limit;
        let mut items: Vec<Skill> = rows
            .iter()
            .take(limit as usize)
            .map(Self::parse_skill)
            .collect::<DbResult<Vec<_>>>()?;
        self.attach_manifests(&mut items).await?;

        let cursors =
            PageCursors::from_items(&items, has_more, CursorDirection::Forward, None, |s| {
                cursor_from_row(s.created_at, s.id)
            });

        Ok(ListResult::new(items, has_more, cursors))
    }

    async fn count_by_owner(
        &self,
        owner_type: SkillOwnerType,
        owner_id: Uuid,
        include_deleted: bool,
    ) -> DbResult<i64> {
        let sql = if include_deleted {
            "SELECT COUNT(*) AS count FROM skills WHERE owner_type = ? AND owner_id = ?"
        } else {
            "SELECT COUNT(*) AS count FROM skills WHERE owner_type = ? AND owner_id = ? \
             AND deleted_at IS NULL"
        };

        let row = query(sql)
            .bind(owner_type.as_str())
            .bind(owner_id.to_string())
            .fetch_one(&self.pool)
            .await?;

        Ok(row.col::<i64>("count"))
    }

    async fn update(&self, id: Uuid, input: UpdateSkill) -> DbResult<Skill> {
        let UpdateSkill {
            name,
            description,
            files,
            user_invocable,
            disable_model_invocation,
            allowed_tools,
            argument_hint,
            source_url,
            source_ref,
            frontmatter_extra,
        } = input;

        let has_changes = name.is_some()
            || description.is_some()
            || files.is_some()
            || user_invocable.is_some()
            || disable_model_invocation.is_some()
            || allowed_tools.is_some()
            || argument_hint.is_some()
            || source_url.is_some()
            || source_ref.is_some()
            || frontmatter_extra.is_some();

        if !has_changes {
            return self.get_by_id(id).await?.ok_or(DbError::NotFound);
        }

        let now = truncate_to_millis(chrono::Utc::now());

        let files_with_size: Option<Vec<(SkillFileInput, i64, String)>> = files.as_ref().map(|fs| {
            fs.iter()
                .map(|f| {
                    let size = f.content.len() as i64;
                    let ct = f
                        .content_type
                        .clone()
                        .unwrap_or_else(|| Self::sniff_content_type(&f.path).to_string());
                    (f.clone(), size, ct)
                })
                .collect()
        });
        let new_total_bytes: Option<i64> = files_with_size
            .as_ref()
            .map(|v| v.iter().map(|(_, s, _)| *s).sum());

        let allowed_tools_json = allowed_tools
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|e| DbError::Internal(format!("Failed to serialize allowed_tools: {}", e)))?;
        let frontmatter_extra_json = frontmatter_extra
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|e| {
                DbError::Internal(format!("Failed to serialize frontmatter_extra: {}", e))
            })?;

        let mut set_clauses: Vec<&str> = vec!["updated_at = ?"];
        if name.is_some() {
            set_clauses.push("name = ?");
        }
        if description.is_some() {
            set_clauses.push("description = ?");
        }
        if user_invocable.is_some() {
            set_clauses.push("user_invocable = ?");
        }
        if disable_model_invocation.is_some() {
            set_clauses.push("disable_model_invocation = ?");
        }
        if allowed_tools.is_some() {
            set_clauses.push("allowed_tools = ?");
        }
        if argument_hint.is_some() {
            set_clauses.push("argument_hint = ?");
        }
        if source_url.is_some() {
            set_clauses.push("source_url = ?");
        }
        if source_ref.is_some() {
            set_clauses.push("source_ref = ?");
        }
        if frontmatter_extra.is_some() {
            set_clauses.push("frontmatter_extra = ?");
        }
        if new_total_bytes.is_some() {
            set_clauses.push("total_bytes = ?");
        }

        let sql = format!(
            "UPDATE skills SET {} WHERE id = ? AND deleted_at IS NULL",
            set_clauses.join(", ")
        );

        let mut tx = begin(&self.pool).await?;

        let mut q = query(&sql).bind(now);
        if let Some(ref v) = name {
            q = q.bind(v);
        }
        if let Some(ref v) = description {
            q = q.bind(v);
        }
        if let Some(v) = user_invocable {
            q = q.bind(if v { 1i64 } else { 0i64 });
        }
        if let Some(v) = disable_model_invocation {
            q = q.bind(if v { 1i64 } else { 0i64 });
        }
        if allowed_tools.is_some() {
            q = q.bind(allowed_tools_json.clone());
        }
        if let Some(ref v) = argument_hint {
            q = q.bind(v);
        }
        if let Some(ref v) = source_url {
            q = q.bind(v);
        }
        if let Some(ref v) = source_ref {
            q = q.bind(v);
        }
        if frontmatter_extra.is_some() {
            q = q.bind(frontmatter_extra_json.clone());
        }
        if let Some(total) = new_total_bytes {
            q = q.bind(total);
        }
        q = q.bind(id.to_string());

        let result = q
            .execute(&mut *tx)
            .await
            .map_err(map_unique_violation(
                "Skill with this name already exists for this owner",
            ))?;

        if result.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }

        // Replace the file set if provided.
        if let Some(new_files) = files_with_size {
            query(r#"DELETE FROM skill_files WHERE skill_id = ?"#)
                .bind(id.to_string())
                .execute(&mut *tx)
                .await?;

            for (file, size, content_type) in new_files.iter() {
                query(
                    r#"
                    INSERT INTO skill_files (
                        skill_id, path, content, byte_size, content_type,
                        created_at, updated_at
                    )
                    VALUES (?, ?, ?, ?, ?, ?, ?)
                    "#,
                )
                .bind(id.to_string())
                .bind(&file.path)
                .bind(&file.content)
                .bind(*size)
                .bind(content_type)
                .bind(now)
                .bind(now)
                .execute(&mut *tx)
                .await?;
            }
        }

        tx.commit().await?;

        self.get_by_id(id).await?.ok_or(DbError::NotFound)
    }

    async fn delete(&self, id: Uuid) -> DbResult<()> {
        let now = truncate_to_millis(chrono::Utc::now());

        let result = query(
            r#"
            UPDATE skills
            SET deleted_at = ?
            WHERE id = ? AND deleted_at IS NULL
            "#,
        )
        .bind(now)
        .bind(id.to_string())
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use sqlx::SqlitePool;

    use super::*;
    use crate::models::{SkillFileInput, SkillOwner};

    async fn create_test_pool() -> SqlitePool {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("Failed to create in-memory SQLite pool");

        sqlx::query(
            r#"
            CREATE TABLE skills (
                id TEXT PRIMARY KEY NOT NULL,
                owner_type TEXT NOT NULL CHECK (owner_type IN ('organization', 'team', 'project', 'user')),
                owner_id TEXT NOT NULL,
                name TEXT NOT NULL,
                description TEXT NOT NULL,
                user_invocable INTEGER,
                disable_model_invocation INTEGER,
                allowed_tools TEXT,
                argument_hint TEXT,
                source_url TEXT,
                source_ref TEXT,
                frontmatter_extra TEXT,
                total_bytes INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                deleted_at TEXT,
                UNIQUE(owner_type, owner_id, name)
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to create skills table");

        sqlx::query(
            r#"
            CREATE TABLE skill_files (
                skill_id TEXT NOT NULL REFERENCES skills(id) ON DELETE CASCADE,
                path TEXT NOT NULL,
                content TEXT NOT NULL,
                byte_size INTEGER NOT NULL,
                content_type TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                PRIMARY KEY(skill_id, path)
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to create skill_files table");

        pool
    }

    fn skill_main_file(body: &str) -> SkillFileInput {
        SkillFileInput {
            path: "SKILL.md".into(),
            content: body.into(),
            content_type: None,
        }
    }

    fn create_skill_input(name: &str, body: &str, user_id: Uuid) -> CreateSkill {
        CreateSkill {
            owner: SkillOwner::User { user_id },
            name: name.into(),
            description: "Test skill description.".into(),
            files: vec![skill_main_file(body)],
            user_invocable: None,
            disable_model_invocation: None,
            allowed_tools: None,
            argument_hint: None,
            source_url: None,
            source_ref: None,
            frontmatter_extra: None,
        }
    }

    #[tokio::test]
    async fn create_skill_stores_main_file_and_total_bytes() {
        let pool = create_test_pool().await;
        let repo = SqliteSkillRepo::new(pool);
        let user_id = Uuid::new_v4();

        let skill = repo
            .create(create_skill_input("code-review", "Review code.", user_id))
            .await
            .expect("create should succeed");

        assert_eq!(skill.name, "code-review");
        assert_eq!(skill.owner_type, SkillOwnerType::User);
        assert_eq!(skill.owner_id, user_id);
        assert_eq!(skill.files.len(), 1);
        assert_eq!(skill.files[0].path, "SKILL.md");
        assert_eq!(skill.files[0].content, "Review code.");
        assert_eq!(skill.files[0].content_type, "text/markdown");
        assert_eq!(skill.files[0].byte_size, "Review code.".len() as i64);
        assert_eq!(skill.total_bytes, "Review code.".len() as i64);
    }

    #[tokio::test]
    async fn create_skill_with_bundled_files_sums_total_bytes() {
        let pool = create_test_pool().await;
        let repo = SqliteSkillRepo::new(pool);
        let user_id = Uuid::new_v4();

        let input = CreateSkill {
            owner: SkillOwner::User { user_id },
            name: "pdf-processing".into(),
            description: "Extract PDF text.".into(),
            files: vec![
                skill_main_file("Use scripts/extract.py."),
                SkillFileInput {
                    path: "scripts/extract.py".into(),
                    content: "print('ok')".into(),
                    content_type: None,
                },
                SkillFileInput {
                    path: "references/REFERENCE.md".into(),
                    content: "# Reference".into(),
                    content_type: None,
                },
            ],
            user_invocable: Some(true),
            disable_model_invocation: Some(false),
            allowed_tools: Some(vec!["Bash(python:*)".into()]),
            argument_hint: Some("[file]".into()),
            source_url: None,
            source_ref: None,
            frontmatter_extra: None,
        };

        let expected_total = ("Use scripts/extract.py.".len()
            + "print('ok')".len()
            + "# Reference".len()) as i64;

        let skill = repo.create(input).await.expect("create should succeed");
        assert_eq!(skill.files.len(), 3);
        assert_eq!(skill.total_bytes, expected_total);

        // File paths sorted alphabetically by load_files.
        assert_eq!(skill.files[0].path, "SKILL.md");
        assert_eq!(skill.files[1].path, "references/REFERENCE.md");
        assert_eq!(skill.files[2].path, "scripts/extract.py");

        // Content types sniffed from extension.
        assert_eq!(skill.files[0].content_type, "text/markdown");
        assert_eq!(skill.files[1].content_type, "text/markdown");
        assert_eq!(skill.files[2].content_type, "text/x-python");

        // Frontmatter fields round-tripped.
        assert_eq!(skill.user_invocable, Some(true));
        assert_eq!(skill.disable_model_invocation, Some(false));
        assert_eq!(skill.allowed_tools.as_deref(), Some(&["Bash(python:*)".to_string()][..]));
        assert_eq!(skill.argument_hint.as_deref(), Some("[file]"));
    }

    #[tokio::test]
    async fn create_duplicate_name_per_owner_fails() {
        let pool = create_test_pool().await;
        let repo = SqliteSkillRepo::new(pool);
        let user_id = Uuid::new_v4();

        repo.create(create_skill_input("dup", "a", user_id))
            .await
            .expect("first create succeeds");

        let result = repo
            .create(create_skill_input("dup", "b", user_id))
            .await;
        assert!(matches!(result, Err(DbError::Conflict(_))));
    }

    #[tokio::test]
    async fn same_name_different_owners_succeeds() {
        let pool = create_test_pool().await;
        let repo = SqliteSkillRepo::new(pool);
        let u1 = Uuid::new_v4();
        let u2 = Uuid::new_v4();

        repo.create(create_skill_input("same", "x", u1))
            .await
            .unwrap();
        repo.create(create_skill_input("same", "y", u2))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn get_by_id_returns_full_file_contents() {
        let pool = create_test_pool().await;
        let repo = SqliteSkillRepo::new(pool);
        let user_id = Uuid::new_v4();

        let created = repo
            .create(create_skill_input("lookup", "Body.", user_id))
            .await
            .unwrap();

        let fetched = repo.get_by_id(created.id).await.unwrap().unwrap();
        assert_eq!(fetched.id, created.id);
        assert_eq!(fetched.files.len(), 1);
        assert_eq!(fetched.files[0].content, "Body.");
    }

    #[tokio::test]
    async fn get_by_id_missing() {
        let pool = create_test_pool().await;
        let repo = SqliteSkillRepo::new(pool);
        assert!(repo.get_by_id(Uuid::new_v4()).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn list_by_owner_populates_files_manifest_but_not_content() {
        let pool = create_test_pool().await;
        let repo = SqliteSkillRepo::new(pool);
        let user_id = Uuid::new_v4();

        let input = CreateSkill {
            owner: SkillOwner::User { user_id },
            name: "s1".into(),
            description: "d".into(),
            files: vec![
                skill_main_file("body"),
                SkillFileInput {
                    path: "notes.txt".into(),
                    content: "hi".into(),
                    content_type: None,
                },
            ],
            user_invocable: None,
            disable_model_invocation: None,
            allowed_tools: None,
            argument_hint: None,
            source_url: None,
            source_ref: None,
            frontmatter_extra: None,
        };
        repo.create(input).await.unwrap();

        let result = repo
            .list_by_owner(SkillOwnerType::User, user_id, ListParams::default())
            .await
            .unwrap();

        assert_eq!(result.items.len(), 1);
        let skill = &result.items[0];
        assert!(skill.files.is_empty(), "list should not include file contents");
        assert_eq!(skill.files_manifest.len(), 2);
        let paths: Vec<&str> = skill.files_manifest.iter().map(|m| m.path.as_str()).collect();
        assert_eq!(paths, vec!["SKILL.md", "notes.txt"]);
    }

    #[tokio::test]
    async fn list_by_owner_filters_by_owner() {
        let pool = create_test_pool().await;
        let repo = SqliteSkillRepo::new(pool);
        let u1 = Uuid::new_v4();
        let u2 = Uuid::new_v4();

        repo.create(create_skill_input("u1-skill", "x", u1))
            .await
            .unwrap();
        repo.create(create_skill_input("u2-skill", "y", u2))
            .await
            .unwrap();

        let r1 = repo
            .list_by_owner(SkillOwnerType::User, u1, ListParams::default())
            .await
            .unwrap();
        let r2 = repo
            .list_by_owner(SkillOwnerType::User, u2, ListParams::default())
            .await
            .unwrap();

        assert_eq!(r1.items.len(), 1);
        assert_eq!(r1.items[0].name, "u1-skill");
        assert_eq!(r2.items.len(), 1);
        assert_eq!(r2.items[0].name, "u2-skill");
    }

    #[tokio::test]
    async fn count_by_owner_excludes_deleted() {
        let pool = create_test_pool().await;
        let repo = SqliteSkillRepo::new(pool);
        let user_id = Uuid::new_v4();

        let a = repo
            .create(create_skill_input("a", "a", user_id))
            .await
            .unwrap();
        repo.create(create_skill_input("b", "b", user_id))
            .await
            .unwrap();
        repo.delete(a.id).await.unwrap();

        let live = repo
            .count_by_owner(SkillOwnerType::User, user_id, false)
            .await
            .unwrap();
        let all = repo
            .count_by_owner(SkillOwnerType::User, user_id, true)
            .await
            .unwrap();
        assert_eq!(live, 1);
        assert_eq!(all, 2);
    }

    #[tokio::test]
    async fn update_replaces_file_set_and_total_bytes() {
        let pool = create_test_pool().await;
        let repo = SqliteSkillRepo::new(pool);
        let user_id = Uuid::new_v4();

        let created = repo
            .create(create_skill_input("rewrite", "original", user_id))
            .await
            .unwrap();
        assert_eq!(created.total_bytes, "original".len() as i64);

        let new_files = vec![
            SkillFileInput {
                path: "SKILL.md".into(),
                content: "replaced".into(),
                content_type: None,
            },
            SkillFileInput {
                path: "extra.txt".into(),
                content: "more".into(),
                content_type: None,
            },
        ];

        let updated = repo
            .update(
                created.id,
                UpdateSkill {
                    files: Some(new_files),
                    description: Some("Updated desc.".into()),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        assert_eq!(updated.description, "Updated desc.");
        assert_eq!(updated.files.len(), 2);
        let expected = ("replaced".len() + "more".len()) as i64;
        assert_eq!(updated.total_bytes, expected);

        // Old file content is gone — no stale paths.
        let paths: Vec<&str> = updated.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, vec!["SKILL.md", "extra.txt"]);
    }

    #[tokio::test]
    async fn update_with_no_changes_returns_existing() {
        let pool = create_test_pool().await;
        let repo = SqliteSkillRepo::new(pool);
        let user_id = Uuid::new_v4();

        let created = repo
            .create(create_skill_input("noop", "body", user_id))
            .await
            .unwrap();

        let same = repo
            .update(created.id, UpdateSkill::default())
            .await
            .unwrap();
        assert_eq!(same.name, "noop");
        assert_eq!(same.total_bytes, created.total_bytes);
    }

    #[tokio::test]
    async fn update_not_found() {
        let pool = create_test_pool().await;
        let repo = SqliteSkillRepo::new(pool);
        let result = repo
            .update(
                Uuid::new_v4(),
                UpdateSkill {
                    description: Some("x".into()),
                    ..Default::default()
                },
            )
            .await;
        assert!(matches!(result, Err(DbError::NotFound)));
    }

    #[tokio::test]
    async fn delete_soft_deletes() {
        let pool = create_test_pool().await;
        let repo = SqliteSkillRepo::new(pool);
        let user_id = Uuid::new_v4();

        let created = repo
            .create(create_skill_input("gone", "body", user_id))
            .await
            .unwrap();

        repo.delete(created.id).await.unwrap();
        assert!(repo.get_by_id(created.id).await.unwrap().is_none());

        let list = repo
            .list_by_owner(SkillOwnerType::User, user_id, ListParams::default())
            .await
            .unwrap();
        assert!(list.items.is_empty());
    }

    #[tokio::test]
    async fn delete_not_found() {
        let pool = create_test_pool().await;
        let repo = SqliteSkillRepo::new(pool);
        let result = repo.delete(Uuid::new_v4()).await;
        assert!(matches!(result, Err(DbError::NotFound)));
    }

    #[tokio::test]
    async fn different_owner_types_are_scoped() {
        let pool = create_test_pool().await;
        let repo = SqliteSkillRepo::new(pool);

        let org_id = Uuid::new_v4();
        let team_id = Uuid::new_v4();
        let project_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        for (name, owner) in [
            ("org-skill", SkillOwner::Organization { organization_id: org_id }),
            ("team-skill", SkillOwner::Team { team_id }),
            ("project-skill", SkillOwner::Project { project_id }),
            ("user-skill", SkillOwner::User { user_id }),
        ] {
            repo.create(CreateSkill {
                owner,
                name: name.into(),
                description: "d".into(),
                files: vec![skill_main_file("b")],
                user_invocable: None,
                disable_model_invocation: None,
                allowed_tools: None,
                argument_hint: None,
                source_url: None,
                source_ref: None,
                frontmatter_extra: None,
            })
            .await
            .unwrap();
        }

        let check = |ot, id| {
            let repo = &repo;
            async move {
                repo.list_by_owner(ot, id, ListParams::default())
                    .await
                    .unwrap()
                    .items
            }
        };

        assert_eq!(check(SkillOwnerType::Organization, org_id).await.len(), 1);
        assert_eq!(check(SkillOwnerType::Team, team_id).await.len(), 1);
        assert_eq!(check(SkillOwnerType::Project, project_id).await.len(), 1);
        assert_eq!(check(SkillOwnerType::User, user_id).await.len(), 1);
    }
}
