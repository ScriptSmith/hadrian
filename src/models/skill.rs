//! Agent Skills per https://agentskills.io/specification.md.
//!
//! A skill is a packaged set of instructions (SKILL.md) plus optional
//! bundled files (scripts, references, assets). Hadrian's extension of the
//! spec is that every skill is owned by an organization, team, project, or
//! user — matching the ownership model used by prompt templates.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::{Validate, ValidationError};

/// The filename of the required main instructions file in every skill.
pub const SKILL_MAIN_FILE: &str = "SKILL.md";

/// Owner type for skills (organization, team, project, or user).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum SkillOwnerType {
    Organization,
    Team,
    Project,
    User,
}

impl SkillOwnerType {
    pub fn as_str(&self) -> &'static str {
        match self {
            SkillOwnerType::Organization => "organization",
            SkillOwnerType::Team => "team",
            SkillOwnerType::Project => "project",
            SkillOwnerType::User => "user",
        }
    }
}

impl std::str::FromStr for SkillOwnerType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "organization" => Ok(SkillOwnerType::Organization),
            "team" => Ok(SkillOwnerType::Team),
            "project" => Ok(SkillOwnerType::Project),
            "user" => Ok(SkillOwnerType::User),
            _ => Err(format!("Invalid skill owner type: {}", s)),
        }
    }
}

/// Validate skill `name` per https://agentskills.io/specification.md:
/// 1..=64 chars, lowercase ASCII alphanumeric or hyphen, no leading or
/// trailing hyphen, no consecutive hyphens.
pub fn validate_skill_name(name: &str) -> Result<(), ValidationError> {
    if !(1..=64).contains(&name.len()) {
        return Err(ValidationError::new("skill_name_length"));
    }
    if name.starts_with('-') || name.ends_with('-') {
        return Err(ValidationError::new("skill_name_hyphen_boundary"));
    }
    if name.contains("--") {
        return Err(ValidationError::new("skill_name_consecutive_hyphens"));
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(ValidationError::new("skill_name_charset"));
    }
    Ok(())
}

/// Validate a relative skill-file path. No absolute paths, no `..` segments,
/// no empty segments, 1..=255 bytes.
pub fn validate_skill_path(path: &str) -> Result<(), ValidationError> {
    if path.is_empty() || path.len() > 255 {
        return Err(ValidationError::new("skill_path_length"));
    }
    if path.starts_with('/') || path.starts_with('\\') {
        return Err(ValidationError::new("skill_path_absolute"));
    }
    for seg in path.split(['/', '\\']) {
        if seg.is_empty() || seg == ".." || seg == "." {
            return Err(ValidationError::new("skill_path_traversal"));
        }
    }
    Ok(())
}

/// A file bundled with a skill. Returned in full detail by get-by-id; list
/// endpoints populate [`SkillFileManifest`] instead.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct SkillFile {
    /// Relative path inside the skill, e.g. "SKILL.md" or "scripts/extract.py".
    pub path: String,
    /// File contents. Text-only in v1 (binary assets unsupported).
    pub content: String,
    /// Byte length of `content`.
    pub byte_size: i64,
    /// MIME type, e.g. "text/markdown".
    pub content_type: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Lightweight file entry returned by list endpoints — contents omitted.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct SkillFileManifest {
    pub path: String,
    pub byte_size: i64,
    pub content_type: String,
}

/// A packaged Agent Skill.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct Skill {
    pub id: Uuid,
    pub owner_type: SkillOwnerType,
    pub owner_id: Uuid,
    /// Skill name (unique per owner). See [`validate_skill_name`].
    pub name: String,
    /// Human-readable description. Used by the model to decide when to
    /// invoke the skill.
    pub description: String,

    /// If `false`, the skill is hidden from the user-visible slash-command
    /// list. `None` = unset (defaults to `true`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_invocable: Option<bool>,
    /// If `true`, the model cannot auto-invoke this skill. `None` = unset
    /// (defaults to `false`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disable_model_invocation: Option<bool>,
    /// Tools the skill is allowed to use. Informational; the chat UI may
    /// use this to pre-approve tools while the skill is active.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_tools: Option<Vec<String>>,
    /// Hint shown during autocomplete to describe expected arguments.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub argument_hint: Option<String>,
    /// Origin URL if imported (e.g. a GitHub tree URL).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    /// Git ref if imported.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<String>,
    /// Unknown / forward-compat frontmatter keys preserved verbatim.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frontmatter_extra: Option<HashMap<String, serde_json::Value>>,

    /// Cached total size across all files (bytes).
    pub total_bytes: i64,

    /// Full file contents. Populated by get-by-id endpoints.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<SkillFile>,
    /// File summary (no contents). Populated by list endpoints.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files_manifest: Vec<SkillFileManifest>,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Owner specification for creating a skill.
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SkillOwner {
    Organization { organization_id: Uuid },
    Team { team_id: Uuid },
    Project { project_id: Uuid },
    User { user_id: Uuid },
}

impl SkillOwner {
    pub fn owner_type(&self) -> SkillOwnerType {
        match self {
            SkillOwner::Organization { .. } => SkillOwnerType::Organization,
            SkillOwner::Team { .. } => SkillOwnerType::Team,
            SkillOwner::Project { .. } => SkillOwnerType::Project,
            SkillOwner::User { .. } => SkillOwnerType::User,
        }
    }

    pub fn owner_id(&self) -> Uuid {
        match self {
            SkillOwner::Organization { organization_id } => *organization_id,
            SkillOwner::Team { team_id } => *team_id,
            SkillOwner::Project { project_id } => *project_id,
            SkillOwner::User { user_id } => *user_id,
        }
    }
}

/// A single file in a create/update request.
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct SkillFileInput {
    #[validate(custom(function = "validate_skill_path"))]
    pub path: String,
    #[validate(length(min = 1))]
    pub content: String,
    /// Optional MIME type. If omitted, the service sniffs it from the path
    /// extension.
    #[validate(length(max = 127))]
    pub content_type: Option<String>,
}

/// Request to create a new skill. `files` must contain exactly one entry
/// with `path == "SKILL.md"`; the service layer rejects otherwise.
#[derive(Debug, Clone, Deserialize, Validate)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CreateSkill {
    pub owner: SkillOwner,
    #[validate(custom(function = "validate_skill_name"))]
    pub name: String,
    #[validate(length(min = 1, max = 1024))]
    pub description: String,
    #[validate(length(min = 1), nested)]
    pub files: Vec<SkillFileInput>,

    pub user_invocable: Option<bool>,
    pub disable_model_invocation: Option<bool>,
    pub allowed_tools: Option<Vec<String>>,
    #[validate(length(max = 255))]
    pub argument_hint: Option<String>,
    #[validate(length(max = 2048))]
    pub source_url: Option<String>,
    #[validate(length(max = 255))]
    pub source_ref: Option<String>,
    pub frontmatter_extra: Option<HashMap<String, serde_json::Value>>,
}

/// Request to update a skill. Any field that is `Some(_)` replaces the
/// stored value. When `files` is `Some(_)`, the entire file set is
/// replaced.
#[derive(Debug, Clone, Default, Deserialize, Validate)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct UpdateSkill {
    #[validate(custom(function = "validate_skill_name"))]
    pub name: Option<String>,
    #[validate(length(min = 1, max = 1024))]
    pub description: Option<String>,
    #[validate(nested)]
    pub files: Option<Vec<SkillFileInput>>,

    pub user_invocable: Option<bool>,
    pub disable_model_invocation: Option<bool>,
    pub allowed_tools: Option<Vec<String>>,
    #[validate(length(max = 255))]
    pub argument_hint: Option<String>,
    #[validate(length(max = 2048))]
    pub source_url: Option<String>,
    #[validate(length(max = 255))]
    pub source_ref: Option<String>,
    pub frontmatter_extra: Option<HashMap<String, serde_json::Value>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skill_name_accepts_valid_examples() {
        for name in [
            "pdf-processing",
            "data-analysis",
            "code-review",
            "a",
            "abc123",
        ] {
            assert!(
                validate_skill_name(name).is_ok(),
                "expected {name:?} to be valid"
            );
        }
    }

    #[test]
    fn skill_name_rejects_bad_examples() {
        for name in [
            "",
            "PDF-Processing",
            "-pdf",
            "pdf-",
            "pdf--processing",
            "pdf_processing",
            "pdf processing",
            &"x".repeat(65),
        ] {
            assert!(
                validate_skill_name(name).is_err(),
                "expected {name:?} to be invalid"
            );
        }
    }

    #[test]
    fn skill_path_accepts_valid_examples() {
        for path in [
            "SKILL.md",
            "scripts/extract.py",
            "references/REFERENCE.md",
            "assets/template.txt",
            "a/b/c/d.txt",
        ] {
            assert!(
                validate_skill_path(path).is_ok(),
                "expected {path:?} to be valid"
            );
        }
    }

    #[test]
    fn skill_path_rejects_bad_examples() {
        for path in [
            "",
            "/absolute/path.md",
            "\\windows\\style.md",
            "../escape.md",
            "ok/../escape.md",
            "./SKILL.md",
            "scripts/./helper.py",
            "double//slash.md",
            &"x".repeat(256),
        ] {
            assert!(
                validate_skill_path(path).is_err(),
                "expected {path:?} to be invalid"
            );
        }
    }

    #[test]
    fn skill_owner_type_roundtrips() {
        for ot in [
            SkillOwnerType::Organization,
            SkillOwnerType::Team,
            SkillOwnerType::Project,
            SkillOwnerType::User,
        ] {
            assert_eq!(ot.as_str().parse::<SkillOwnerType>().unwrap(), ot);
        }
    }

    #[test]
    fn skill_owner_extracts_type_and_id() {
        let org = Uuid::new_v4();
        let owner = SkillOwner::Organization {
            organization_id: org,
        };
        assert_eq!(owner.owner_type(), SkillOwnerType::Organization);
        assert_eq!(owner.owner_id(), org);
    }
}
