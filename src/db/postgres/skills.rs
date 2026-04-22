use std::collections::HashMap;

use async_trait::async_trait;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::{
    db::{
        error::{DbError, DbResult},
        repos::{
            CursorDirection, ListParams, ListResult, PageCursors, SkillRepo, cursor_from_row,
        },
    },
    models::{
        CreateSkill, Skill, SkillFile, SkillFileInput, SkillFileManifest, SkillOwnerType,
        UpdateSkill,
    },
};

const SKILL_COLUMNS: &str = "id, owner_type::TEXT, owner_id, name, description, user_invocable, \
     disable_model_invocation, allowed_tools, argument_hint, source_url, source_ref, \
     frontmatter_extra, total_bytes, created_at, updated_at";

/// Same columns as [`SKILL_COLUMNS`] but prefixed with `s.` for aliased queries.
const SKILL_COLUMNS_ALIASED: &str = "s.id, s.owner_type::TEXT, s.owner_id, s.name, s.description, \
     s.user_invocable, s.disable_model_invocation, s.allowed_tools, s.argument_hint, \
     s.source_url, s.source_ref, s.frontmatter_extra, s.total_bytes, s.created_at, s.updated_at";

pub struct PostgresSkillRepo {
    write_pool: PgPool,
    read_pool: PgPool,
}

impl PostgresSkillRepo {
    pub fn new(write_pool: PgPool, read_pool: Option<PgPool>) -> Self {
        let read_pool = read_pool.unwrap_or_else(|| write_pool.clone());
        Self {
            write_pool,
            read_pool,
        }
    }

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

    fn parse_skill(row: &sqlx::postgres::PgRow) -> DbResult<Skill> {
        let owner_type_str: String = row.get("owner_type");
        let owner_type: SkillOwnerType = owner_type_str.parse().map_err(DbError::Internal)?;

        let allowed_tools: Option<serde_json::Value> = row.get("allowed_tools");
        let allowed_tools: Option<Vec<String>> = allowed_tools
            .map(serde_json::from_value)
            .transpose()
            .map_err(|e| DbError::Internal(format!("Failed to parse allowed_tools: {}", e)))?;

        let frontmatter_extra: Option<serde_json::Value> = row.get("frontmatter_extra");
        let frontmatter_extra: Option<HashMap<String, serde_json::Value>> = frontmatter_extra
            .map(serde_json::from_value)
            .transpose()
            .map_err(|e| {
                DbError::Internal(format!("Failed to parse frontmatter_extra: {}", e))
            })?;

        Ok(Skill {
            id: row.get("id"),
            owner_type,
            owner_id: row.get("owner_id"),
            name: row.get("name"),
            description: row.get("description"),
            user_invocable: row.get("user_invocable"),
            disable_model_invocation: row.get("disable_model_invocation"),
            allowed_tools,
            argument_hint: row.get("argument_hint"),
            source_url: row.get("source_url"),
            source_ref: row.get("source_ref"),
            frontmatter_extra,
            total_bytes: row.get("total_bytes"),
            files: Vec::new(),
            files_manifest: Vec::new(),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }

    fn parse_file(row: &sqlx::postgres::PgRow) -> SkillFile {
        SkillFile {
            path: row.get("path"),
            content: row.get("content"),
            byte_size: row.get("byte_size"),
            content_type: row.get("content_type"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        }
    }

    fn parse_manifest(row: &sqlx::postgres::PgRow) -> SkillFileManifest {
        SkillFileManifest {
            path: row.get("path"),
            byte_size: row.get("byte_size"),
            content_type: row.get("content_type"),
        }
    }

    async fn load_files(&self, skill_id: Uuid) -> DbResult<Vec<SkillFile>> {
        let rows = sqlx::query(
            r#"
            SELECT path, content, byte_size, content_type, created_at, updated_at
            FROM skill_files
            WHERE skill_id = $1
            ORDER BY path ASC
            "#,
        )
        .bind(skill_id)
        .fetch_all(&self.read_pool)
        .await?;

        Ok(rows.iter().map(Self::parse_file).collect())
    }

    async fn attach_manifests(&self, skills: &mut [Skill]) -> DbResult<()> {
        if skills.is_empty() {
            return Ok(());
        }

        let ids: Vec<Uuid> = skills.iter().map(|s| s.id).collect();
        let rows = sqlx::query(
            r#"
            SELECT skill_id, path, byte_size, content_type
            FROM skill_files
            WHERE skill_id = ANY($1)
            ORDER BY path ASC
            "#,
        )
        .bind(&ids)
        .fetch_all(&self.read_pool)
        .await?;

        let mut by_skill: HashMap<Uuid, Vec<SkillFileManifest>> = HashMap::new();
        for row in rows.iter() {
            let skill_id: Uuid = row.get("skill_id");
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
}

/// Shared org-scope filter for list_by_org and get_by_id_and_org.
/// Uses $1 for the org_id (referenced four times).
const ORG_SCOPE_FILTER: &str = r#"
    AND (
        (s.owner_type = 'organization' AND s.owner_id = $1)
        OR (s.owner_type = 'team' AND EXISTS (
            SELECT 1 FROM teams t WHERE t.id = s.owner_id AND t.org_id = $1
        ))
        OR (s.owner_type = 'project' AND EXISTS (
            SELECT 1 FROM projects pr WHERE pr.id = s.owner_id AND pr.org_id = $1
        ))
        OR (s.owner_type = 'user' AND EXISTS (
            SELECT 1 FROM org_memberships om WHERE om.user_id = s.owner_id AND om.org_id = $1
        ))
    )
"#;

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl SkillRepo for PostgresSkillRepo {
    async fn create(&self, input: CreateSkill) -> DbResult<Skill> {
        let id = Uuid::new_v4();
        let owner_type = input.owner.owner_type();
        let owner_id = input.owner.owner_id();

        let allowed_tools_json: Option<serde_json::Value> = input
            .allowed_tools
            .as_ref()
            .map(serde_json::to_value)
            .transpose()
            .map_err(|e| DbError::Internal(format!("Failed to serialize allowed_tools: {}", e)))?;
        let frontmatter_extra_json: Option<serde_json::Value> = input
            .frontmatter_extra
            .as_ref()
            .map(serde_json::to_value)
            .transpose()
            .map_err(|e| {
                DbError::Internal(format!("Failed to serialize frontmatter_extra: {}", e))
            })?;

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

        let mut tx = self.write_pool.begin().await?;

        sqlx::query(
            r#"
            INSERT INTO skills (
                id, owner_type, owner_id, name, description,
                user_invocable, disable_model_invocation, allowed_tools,
                argument_hint, source_url, source_ref, frontmatter_extra,
                total_bytes
            )
            VALUES ($1, $2::skill_owner_type, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            "#,
        )
        .bind(id)
        .bind(owner_type.as_str())
        .bind(owner_id)
        .bind(&input.name)
        .bind(&input.description)
        .bind(input.user_invocable)
        .bind(input.disable_model_invocation)
        .bind(&allowed_tools_json)
        .bind(&input.argument_hint)
        .bind(&input.source_url)
        .bind(&input.source_ref)
        .bind(&frontmatter_extra_json)
        .bind(total_bytes)
        .execute(&mut *tx)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(db_err) if db_err.is_unique_violation() => DbError::Conflict(
                format!("Skill with name '{}' already exists for this owner", input.name),
            ),
            _ => DbError::from(e),
        })?;

        for (file, size, content_type) in files_with_size.iter() {
            sqlx::query(
                r#"
                INSERT INTO skill_files (
                    skill_id, path, content, byte_size, content_type
                )
                VALUES ($1, $2, $3, $4, $5)
                "#,
            )
            .bind(id)
            .bind(&file.path)
            .bind(&file.content)
            .bind(*size)
            .bind(content_type)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;

        self.get_by_id(id)
            .await?
            .ok_or_else(|| DbError::Internal("Skill vanished after create".into()))
    }

    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<Skill>> {
        let sql = format!(
            "SELECT {cols} FROM skills WHERE id = $1 AND deleted_at IS NULL",
            cols = SKILL_COLUMNS
        );
        let result = sqlx::query(&sql)
            .bind(id)
            .fetch_optional(&self.read_pool)
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
        // Org id referenced as $1 (4x via filter); skill id as $2.
        let sql = format!(
            "SELECT {cols} FROM skills s WHERE s.id = $2 AND s.deleted_at IS NULL {scope}",
            cols = SKILL_COLUMNS_ALIASED,
            scope = ORG_SCOPE_FILTER,
        );
        let result = sqlx::query(&sql)
            .bind(org_id)
            .bind(id)
            .fetch_optional(&self.read_pool)
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
            let (comparison, order, should_reverse) =
                params.sort_order.cursor_query_params(params.direction);

            let deleted_filter = if params.include_deleted {
                ""
            } else {
                "AND deleted_at IS NULL"
            };

            let sql = format!(
                "SELECT {cols} FROM skills \
                 WHERE owner_type = $1::skill_owner_type AND owner_id = $2 \
                 AND ROW(created_at, id) {cmp} ROW($3, $4) {deleted_filter} \
                 ORDER BY created_at {order}, id {order} LIMIT $5",
                cols = SKILL_COLUMNS,
                cmp = comparison,
                deleted_filter = deleted_filter,
                order = order,
            );

            let rows = sqlx::query(&sql)
                .bind(owner_type.as_str())
                .bind(owner_id)
                .bind(cursor.created_at)
                .bind(cursor.id)
                .bind(fetch_limit)
                .fetch_all(&self.read_pool)
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

        let deleted_filter = if params.include_deleted {
            ""
        } else {
            "AND deleted_at IS NULL"
        };
        let sql = format!(
            "SELECT {cols} FROM skills \
             WHERE owner_type = $1::skill_owner_type AND owner_id = $2 {deleted_filter} \
             ORDER BY created_at DESC, id DESC LIMIT $3",
            cols = SKILL_COLUMNS,
            deleted_filter = deleted_filter
        );

        let rows = sqlx::query(&sql)
            .bind(owner_type.as_str())
            .bind(owner_id)
            .bind(fetch_limit)
            .fetch_all(&self.read_pool)
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

        if let Some(ref cursor) = params.cursor {
            let (comparison, order, should_reverse) =
                params.sort_order.cursor_query_params(params.direction);

            // $1 = org_id (referenced 4x in scope filter), $2/$3 = cursor, $4 = limit
            let sql = format!(
                "SELECT {cols} FROM skills s \
                 WHERE s.deleted_at IS NULL AND ROW(s.created_at, s.id) {cmp} ROW($2, $3) \
                 {scope} \
                 ORDER BY s.created_at {order}, s.id {order} LIMIT $4",
                cols = SKILL_COLUMNS_ALIASED,
                cmp = comparison,
                scope = ORG_SCOPE_FILTER,
                order = order,
            );

            let rows = sqlx::query(&sql)
                .bind(org_id)
                .bind(cursor.created_at)
                .bind(cursor.id)
                .bind(fetch_limit)
                .fetch_all(&self.read_pool)
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
             ORDER BY s.created_at DESC, s.id DESC LIMIT $2",
            cols = SKILL_COLUMNS_ALIASED,
            scope = ORG_SCOPE_FILTER,
        );

        let rows = sqlx::query(&sql)
            .bind(org_id)
            .bind(fetch_limit)
            .fetch_all(&self.read_pool)
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
            "SELECT COUNT(*) AS count FROM skills \
             WHERE owner_type = $1::skill_owner_type AND owner_id = $2"
        } else {
            "SELECT COUNT(*) AS count FROM skills \
             WHERE owner_type = $1::skill_owner_type AND owner_id = $2 AND deleted_at IS NULL"
        };

        let row = sqlx::query(sql)
            .bind(owner_type.as_str())
            .bind(owner_id)
            .fetch_one(&self.read_pool)
            .await?;

        Ok(row.get::<i64, _>("count"))
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

        let allowed_tools_json: Option<serde_json::Value> = allowed_tools
            .as_ref()
            .map(serde_json::to_value)
            .transpose()
            .map_err(|e| DbError::Internal(format!("Failed to serialize allowed_tools: {}", e)))?;
        let frontmatter_extra_json: Option<serde_json::Value> = frontmatter_extra
            .as_ref()
            .map(serde_json::to_value)
            .transpose()
            .map_err(|e| {
                DbError::Internal(format!("Failed to serialize frontmatter_extra: {}", e))
            })?;

        let mut set_clauses: Vec<String> = vec!["updated_at = NOW()".into()];
        let mut param_idx: i32 = 1;
        let mut push = |col: &str, idx: &mut i32| {
            set_clauses.push(format!("{} = ${}", col, idx));
            *idx += 1;
        };

        if name.is_some() {
            push("name", &mut param_idx);
        }
        if description.is_some() {
            push("description", &mut param_idx);
        }
        if user_invocable.is_some() {
            push("user_invocable", &mut param_idx);
        }
        if disable_model_invocation.is_some() {
            push("disable_model_invocation", &mut param_idx);
        }
        if allowed_tools.is_some() {
            push("allowed_tools", &mut param_idx);
        }
        if argument_hint.is_some() {
            push("argument_hint", &mut param_idx);
        }
        if source_url.is_some() {
            push("source_url", &mut param_idx);
        }
        if source_ref.is_some() {
            push("source_ref", &mut param_idx);
        }
        if frontmatter_extra.is_some() {
            push("frontmatter_extra", &mut param_idx);
        }
        if new_total_bytes.is_some() {
            push("total_bytes", &mut param_idx);
        }

        let sql = format!(
            "UPDATE skills SET {} WHERE id = ${} AND deleted_at IS NULL",
            set_clauses.join(", "),
            param_idx
        );

        let mut tx = self.write_pool.begin().await?;

        let mut q = sqlx::query(&sql);
        if let Some(ref v) = name {
            q = q.bind(v);
        }
        if let Some(ref v) = description {
            q = q.bind(v);
        }
        if let Some(v) = user_invocable {
            q = q.bind(v);
        }
        if let Some(v) = disable_model_invocation {
            q = q.bind(v);
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
        q = q.bind(id);

        let result = q
            .execute(&mut *tx)
            .await
            .map_err(|e| match e {
                sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
                    DbError::Conflict("Skill with this name already exists for this owner".into())
                }
                _ => DbError::from(e),
            })?;

        if result.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }

        if let Some(new_files) = files_with_size {
            sqlx::query("DELETE FROM skill_files WHERE skill_id = $1")
                .bind(id)
                .execute(&mut *tx)
                .await?;

            for (file, size, content_type) in new_files.iter() {
                sqlx::query(
                    r#"
                    INSERT INTO skill_files (
                        skill_id, path, content, byte_size, content_type
                    )
                    VALUES ($1, $2, $3, $4, $5)
                    "#,
                )
                .bind(id)
                .bind(&file.path)
                .bind(&file.content)
                .bind(*size)
                .bind(content_type)
                .execute(&mut *tx)
                .await?;
            }
        }

        tx.commit().await?;

        self.get_by_id(id).await?.ok_or(DbError::NotFound)
    }

    async fn delete(&self, id: Uuid) -> DbResult<()> {
        let result = sqlx::query(
            r#"
            UPDATE skills SET deleted_at = NOW()
            WHERE id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(id)
        .execute(&self.write_pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }

        Ok(())
    }
}
