use std::{collections::HashSet, sync::Arc};

use uuid::Uuid;

use crate::{
    db::{DbError, DbPool, DbResult, ListParams, repos::ListResult},
    models::{CreateSkill, SKILL_MAIN_FILE, Skill, SkillFileInput, SkillOwnerType, UpdateSkill},
};

/// Service layer for skill operations. Enforces spec invariants on top of the
/// raw repo:
/// - Exactly one file must have path == "SKILL.md".
/// - No duplicate paths within a skill.
/// - Total file size must not exceed the configured `max_skill_bytes` limit.
#[derive(Clone)]
pub struct SkillService {
    db: Arc<DbPool>,
    max_skill_bytes: u32,
}

impl SkillService {
    pub fn new(db: Arc<DbPool>, max_skill_bytes: u32) -> Self {
        Self {
            db,
            max_skill_bytes,
        }
    }

    fn validate_files(&self, files: &[SkillFileInput]) -> DbResult<()> {
        if files.is_empty() {
            return Err(DbError::Validation(
                "Skill must contain at least one file".into(),
            ));
        }

        let main_count = files.iter().filter(|f| f.path == SKILL_MAIN_FILE).count();
        if main_count == 0 {
            return Err(DbError::Validation(format!(
                "Skill must contain a `{}` file",
                SKILL_MAIN_FILE
            )));
        }
        if main_count > 1 {
            return Err(DbError::Validation(format!(
                "Skill must not contain more than one `{}` file",
                SKILL_MAIN_FILE
            )));
        }

        let mut seen: HashSet<&str> = HashSet::with_capacity(files.len());
        for file in files {
            if !seen.insert(file.path.as_str()) {
                return Err(DbError::Validation(format!(
                    "Duplicate file path in skill: {}",
                    file.path
                )));
            }
        }

        // Byte-size limit is configured at runtime; skip the check when set to 0
        // (meaning "unlimited", matching the convention used by other resource
        // limits in ResourceLimits).
        if self.max_skill_bytes > 0 {
            let total: u64 = files.iter().map(|f| f.content.len() as u64).sum();
            if total > self.max_skill_bytes as u64 {
                return Err(DbError::Validation(format!(
                    "Skill files total {} bytes, exceeding the configured limit of {} bytes",
                    total, self.max_skill_bytes
                )));
            }
        }

        Ok(())
    }

    /// Create a new skill after enforcing invariants on its file set.
    pub async fn create(&self, input: CreateSkill) -> DbResult<Skill> {
        self.validate_files(&input.files)?;
        self.db.skills().create(input).await
    }

    /// Get a skill by ID (with full file contents).
    pub async fn get_by_id(&self, id: Uuid) -> DbResult<Option<Skill>> {
        self.db.skills().get_by_id(id).await
    }

    /// Get a skill by ID, scoped to a specific organization.
    pub async fn get_by_id_and_org(&self, id: Uuid, org_id: Uuid) -> DbResult<Option<Skill>> {
        self.db.skills().get_by_id_and_org(id, org_id).await
    }

    /// List skills by owner with pagination (file contents omitted).
    pub async fn list_by_owner(
        &self,
        owner_type: SkillOwnerType,
        owner_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<Skill>> {
        self.db
            .skills()
            .list_by_owner(owner_type, owner_id, params)
            .await
    }

    /// List all skills accessible within an organization.
    pub async fn list_by_org(
        &self,
        org_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<Skill>> {
        self.db.skills().list_by_org(org_id, params).await
    }

    /// Count skills by owner.
    pub async fn count_by_owner(
        &self,
        owner_type: SkillOwnerType,
        owner_id: Uuid,
        include_deleted: bool,
    ) -> DbResult<i64> {
        self.db
            .skills()
            .count_by_owner(owner_type, owner_id, include_deleted)
            .await
    }

    /// Update a skill. If `input.files` is provided, invariants are enforced
    /// and the full file set is replaced.
    pub async fn update(&self, id: Uuid, input: UpdateSkill) -> DbResult<Skill> {
        if let Some(ref files) = input.files {
            self.validate_files(files)?;
        }
        self.db.skills().update(id, input).await
    }

    /// Soft-delete a skill by ID.
    pub async fn delete(&self, id: Uuid) -> DbResult<()> {
        self.db.skills().delete(id).await
    }
}
