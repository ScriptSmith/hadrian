//! SCIM Filter to SQL Translation
//!
//! This module converts SCIM filter expressions (RFC 7644) to SQL WHERE clauses
//! for efficient database-level filtering of SCIM resources.
//!
//! ## Supported Filters
//!
//! - Simple attribute comparisons: `userName eq "john"`
//! - Boolean comparisons: `active eq true`
//! - String operators: `co` (contains), `sw` (starts with), `ew` (ends with)
//! - Logical operators: `and`, `or`, `not`
//! - Presence checks: `displayName pr`
//!
//! ### Supported User Attributes
//!
//! - `userName` → `scim_user_mappings.scim_external_id`
//! - `externalId` → `scim_user_mappings.scim_external_id`
//! - `active` → `scim_user_mappings.active`
//! - `displayName` → `users.name`
//! - `name.formatted` → `users.name`
//! - `emails.value` → `users.email`
//!
//! ### Supported Group Attributes
//!
//! - `id` → `scim_group_mappings.id`
//! - `externalId` → `scim_group_mappings.scim_group_id`
//! - `displayName` → `scim_group_mappings.display_name`
//!
//! ## Unsupported Filters
//!
//! The following patterns return `None`, causing the caller to return an error:
//!
//! - **Value filters**: `emails[type eq "work"].value eq "john@example.com"`
//!   - Hadrian stores a single email per user, not a multi-valued array with type metadata.
//!   - Supporting this would require a normalized schema (separate emails table) or JSONB storage.
//!
//! - **`members` attribute** (groups): `members eq "user-123"`
//!   - Requires a subquery join to team_memberships, which is complex and rarely needed.
//!   - Most IdPs filter groups by `displayName` or `externalId`, not by membership.
//!
//! - **Unknown attributes**: `phoneNumbers`, `addresses`, `ims`, etc.
//!   - Hadrian's user model doesn't include these SCIM-defined attributes.
//!
//! ## Why Not Fall Back to In-Memory Filtering?
//!
//! A previous implementation attempted to filter in-memory when SQL translation failed.
//! This was removed because it produced incorrect results:
//!
//! 1. **Broken pagination**: Fetching `LIMIT 10` then filtering in-memory might return
//!    only 3 results, even when more matching records exist.
//! 2. **Wrong `totalResults`**: The count reflected all records, not filtered records.
//! 3. **Memory concerns**: Fetching all records for large organizations is expensive.
//!
//! Returning an explicit error is better than returning incorrect data.

use super::filter::{AttrPath, CompareOp, Filter, FilterValue};

/// Result of converting a SCIM filter to SQL.
#[derive(Debug, Clone)]
pub struct SqlFilter {
    /// SQL WHERE clause fragment (e.g., "LOWER(u.email) = LOWER(?)")
    pub where_clause: String,
    /// Bind values in order
    pub bindings: Vec<SqlValue>,
}

/// SQL bind value types.
#[derive(Debug, Clone, PartialEq)]
pub enum SqlValue {
    String(String),
    Bool(bool),
    Float(f64),
}

/// SCIM resource type for attribute mapping context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScimResourceType {
    User,
    Group,
}

/// Attribute mapping from SCIM attribute name to database column.
#[derive(Debug, Clone, Copy)]
struct AttrMapping {
    /// Table alias (e.g., "m" for mappings, "u" for users)
    table_alias: &'static str,
    /// Column name
    column: &'static str,
    /// Whether this is a string column (affects case sensitivity and presence checks)
    is_string: bool,
}

impl AttrMapping {
    const fn string(table_alias: &'static str, column: &'static str) -> Self {
        Self {
            table_alias,
            column,
            is_string: true,
        }
    }

    const fn bool(table_alias: &'static str, column: &'static str) -> Self {
        Self {
            table_alias,
            column,
            is_string: false,
        }
    }

    fn full_column(&self) -> String {
        format!("{}.{}", self.table_alias, self.column)
    }
}

/// User attribute mappings.
/// Table aliases: m = scim_user_mappings, u = users
fn user_attr_mapping(attr: &str, sub_attr: Option<&str>) -> Option<AttrMapping> {
    match (
        attr.to_lowercase().as_str(),
        sub_attr.map(|s| s.to_lowercase()),
    ) {
        // Direct mapping attributes
        ("id", None) => Some(AttrMapping::string("m", "id")),
        ("externalid", None) => Some(AttrMapping::string("m", "scim_external_id")),
        ("username", None) => Some(AttrMapping::string("m", "scim_external_id")),
        ("active", None) => Some(AttrMapping::bool("m", "active")),
        ("displayname", None) => Some(AttrMapping::string("u", "name")),

        // Nested name attributes
        ("name", Some(ref s)) if s == "formatted" => Some(AttrMapping::string("u", "name")),
        ("name", None) => Some(AttrMapping::string("u", "name")), // Treat bare "name" as name.formatted

        // Email - simplified mapping (we only store one email)
        ("emails", Some(ref s)) if s == "value" => Some(AttrMapping::string("u", "email")),
        ("emails", None) => Some(AttrMapping::string("u", "email")), // Treat bare "emails" as emails.value

        _ => None,
    }
}

/// Group attribute mappings.
/// Table aliases: m = scim_group_mappings, t = teams
fn group_attr_mapping(attr: &str, sub_attr: Option<&str>) -> Option<AttrMapping> {
    match (attr.to_lowercase().as_str(), sub_attr) {
        ("id", None) => Some(AttrMapping::string("m", "id")),
        ("externalid", None) => Some(AttrMapping::string("m", "scim_group_id")),
        ("displayname", None) => Some(AttrMapping::string("m", "display_name")),
        // members attribute is not supported for SQL filtering (requires complex join)
        _ => None,
    }
}

/// Get the attribute mapping for a given SCIM attribute path.
fn get_attr_mapping(attr_path: &AttrPath, resource_type: ScimResourceType) -> Option<AttrMapping> {
    // Reject value filters - they require complex logic
    if attr_path.value_filter.is_some() {
        return None;
    }

    match resource_type {
        ScimResourceType::User => user_attr_mapping(&attr_path.attr, attr_path.sub_attr.as_deref()),
        ScimResourceType::Group => {
            group_attr_mapping(&attr_path.attr, attr_path.sub_attr.as_deref())
        }
    }
}

/// Convert a SCIM filter to SQL WHERE clause.
///
/// Returns `None` if the filter cannot be translated to SQL (caller should
/// fall back to in-memory filtering).
///
/// # Arguments
///
/// * `filter` - The parsed SCIM filter
/// * `resource_type` - Whether filtering users or groups
///
/// # Example
///
/// ```ignore
/// let filter = parse_filter("userName eq \"john\" and active eq true")?;
/// if let Some(sql) = filter_to_sql(&filter, ScimResourceType::User) {
///     // Use sql.where_clause and sql.bindings in query
/// } else {
///     // Fall back to in-memory filtering
/// }
/// ```
pub fn filter_to_sql(filter: &Filter, resource_type: ScimResourceType) -> Option<SqlFilter> {
    let mut ctx = TranslationContext::new();
    let where_clause = ctx.translate_filter(filter, resource_type)?;
    Some(SqlFilter {
        where_clause,
        bindings: ctx.bindings,
    })
}

/// Internal context for building SQL queries.
struct TranslationContext {
    bindings: Vec<SqlValue>,
}

impl TranslationContext {
    fn new() -> Self {
        Self {
            bindings: Vec::new(),
        }
    }

    /// Add a binding and return the placeholder (e.g., "?")
    fn add_binding(&mut self, value: SqlValue) -> String {
        self.bindings.push(value);
        "?".to_string()
    }

    /// Translate a filter to SQL WHERE clause fragment.
    fn translate_filter(
        &mut self,
        filter: &Filter,
        resource_type: ScimResourceType,
    ) -> Option<String> {
        match filter {
            Filter::Compare { attr, op, value } => {
                self.translate_compare(attr, *op, value, resource_type)
            }
            Filter::Present { attr } => self.translate_present(attr, resource_type),
            Filter::And(left, right) => {
                let left_sql = self.translate_filter(left, resource_type)?;
                let right_sql = self.translate_filter(right, resource_type)?;
                Some(format!("({} AND {})", left_sql, right_sql))
            }
            Filter::Or(left, right) => {
                let left_sql = self.translate_filter(left, resource_type)?;
                let right_sql = self.translate_filter(right, resource_type)?;
                Some(format!("({} OR {})", left_sql, right_sql))
            }
            Filter::Not(inner) => {
                let inner_sql = self.translate_filter(inner, resource_type)?;
                Some(format!("NOT ({})", inner_sql))
            }
        }
    }

    /// Translate a comparison expression.
    fn translate_compare(
        &mut self,
        attr: &AttrPath,
        op: CompareOp,
        value: &FilterValue,
        resource_type: ScimResourceType,
    ) -> Option<String> {
        let mapping = get_attr_mapping(attr, resource_type)?;
        let col = mapping.full_column();

        match (value, mapping.is_string) {
            // String comparisons (case-insensitive per SCIM spec)
            (FilterValue::String(s), true) => self.translate_string_compare(&col, op, s),
            // Boolean comparisons
            (FilterValue::Bool(b), false) => self.translate_bool_compare(&col, op, *b),
            // Number comparisons (not commonly used in SCIM user/group filters)
            (FilterValue::Number(n), _) => self.translate_number_compare(&col, op, *n),
            // Null comparisons
            (FilterValue::Null, _) => self.translate_null_compare(&col, op),
            // Type mismatch (e.g., comparing string column with boolean)
            _ => None,
        }
    }

    /// Translate string comparison (case-insensitive).
    fn translate_string_compare(
        &mut self,
        col: &str,
        op: CompareOp,
        value: &str,
    ) -> Option<String> {
        match op {
            CompareOp::Eq => {
                let placeholder = self.add_binding(SqlValue::String(value.to_string()));
                Some(format!("LOWER({}) = LOWER({})", col, placeholder))
            }
            CompareOp::Ne => {
                let placeholder = self.add_binding(SqlValue::String(value.to_string()));
                Some(format!("LOWER({}) != LOWER({})", col, placeholder))
            }
            CompareOp::Co => {
                // Contains: LIKE '%value%'
                let escaped = escape_like_pattern(value);
                let placeholder = self.add_binding(SqlValue::String(format!("%{}%", escaped)));
                Some(format!(
                    "LOWER({}) LIKE LOWER({}) ESCAPE '\\'",
                    col, placeholder
                ))
            }
            CompareOp::Sw => {
                // Starts with: LIKE 'value%'
                let escaped = escape_like_pattern(value);
                let placeholder = self.add_binding(SqlValue::String(format!("{}%", escaped)));
                Some(format!(
                    "LOWER({}) LIKE LOWER({}) ESCAPE '\\'",
                    col, placeholder
                ))
            }
            CompareOp::Ew => {
                // Ends with: LIKE '%value'
                let escaped = escape_like_pattern(value);
                let placeholder = self.add_binding(SqlValue::String(format!("%{}", escaped)));
                Some(format!(
                    "LOWER({}) LIKE LOWER({}) ESCAPE '\\'",
                    col, placeholder
                ))
            }
            // String ordering comparisons (less common but valid)
            CompareOp::Gt => {
                let placeholder = self.add_binding(SqlValue::String(value.to_string()));
                Some(format!("LOWER({}) > LOWER({})", col, placeholder))
            }
            CompareOp::Ge => {
                let placeholder = self.add_binding(SqlValue::String(value.to_string()));
                Some(format!("LOWER({}) >= LOWER({})", col, placeholder))
            }
            CompareOp::Lt => {
                let placeholder = self.add_binding(SqlValue::String(value.to_string()));
                Some(format!("LOWER({}) < LOWER({})", col, placeholder))
            }
            CompareOp::Le => {
                let placeholder = self.add_binding(SqlValue::String(value.to_string()));
                Some(format!("LOWER({}) <= LOWER({})", col, placeholder))
            }
        }
    }

    /// Translate boolean comparison.
    fn translate_bool_compare(&mut self, col: &str, op: CompareOp, value: bool) -> Option<String> {
        match op {
            CompareOp::Eq => {
                let placeholder = self.add_binding(SqlValue::Bool(value));
                Some(format!("{} = {}", col, placeholder))
            }
            CompareOp::Ne => {
                let placeholder = self.add_binding(SqlValue::Bool(!value));
                Some(format!("{} = {}", col, placeholder))
            }
            // Other operators don't make sense for booleans
            _ => None,
        }
    }

    /// Translate number comparison.
    fn translate_number_compare(&mut self, col: &str, op: CompareOp, value: f64) -> Option<String> {
        let placeholder = self.add_binding(SqlValue::Float(value));
        let sql_op = match op {
            CompareOp::Eq => "=",
            CompareOp::Ne => "!=",
            CompareOp::Gt => ">",
            CompareOp::Ge => ">=",
            CompareOp::Lt => "<",
            CompareOp::Le => "<=",
            // String operators don't make sense for numbers
            CompareOp::Co | CompareOp::Sw | CompareOp::Ew => return None,
        };
        Some(format!("{} {} {}", col, sql_op, placeholder))
    }

    /// Translate null comparison.
    fn translate_null_compare(&mut self, col: &str, op: CompareOp) -> Option<String> {
        match op {
            CompareOp::Eq => Some(format!("{} IS NULL", col)),
            CompareOp::Ne => Some(format!("{} IS NOT NULL", col)),
            _ => None,
        }
    }

    /// Translate presence check.
    fn translate_present(
        &mut self,
        attr: &AttrPath,
        resource_type: ScimResourceType,
    ) -> Option<String> {
        let mapping = get_attr_mapping(attr, resource_type)?;
        let col = mapping.full_column();

        if mapping.is_string {
            // For strings, check not null AND not empty
            Some(format!("({} IS NOT NULL AND {} != '')", col, col))
        } else {
            // For non-strings, just check not null
            Some(format!("{} IS NOT NULL", col))
        }
    }
}

/// Escape special characters in LIKE patterns.
/// Escapes: %, _, and \
fn escape_like_pattern(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '%' | '_' | '\\' => {
                result.push('\\');
                result.push(c);
            }
            _ => result.push(c),
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scim::filter::parse_filter;

    fn translate_user(filter_str: &str) -> Option<SqlFilter> {
        let filter = parse_filter(filter_str).expect("Failed to parse filter");
        filter_to_sql(&filter, ScimResourceType::User)
    }

    fn translate_group(filter_str: &str) -> Option<SqlFilter> {
        let filter = parse_filter(filter_str).expect("Failed to parse filter");
        filter_to_sql(&filter, ScimResourceType::Group)
    }

    #[test]
    fn test_simple_equality() {
        let result = translate_user(r#"userName eq "john""#).unwrap();
        assert_eq!(result.where_clause, "LOWER(m.scim_external_id) = LOWER(?)");
        assert_eq!(result.bindings, vec![SqlValue::String("john".to_string())]);
    }

    #[test]
    fn test_external_id() {
        let result = translate_user(r#"externalId eq "ext-123""#).unwrap();
        assert_eq!(result.where_clause, "LOWER(m.scim_external_id) = LOWER(?)");
        assert_eq!(
            result.bindings,
            vec![SqlValue::String("ext-123".to_string())]
        );
    }

    #[test]
    fn test_boolean_active_true() {
        let result = translate_user("active eq true").unwrap();
        assert_eq!(result.where_clause, "m.active = ?");
        assert_eq!(result.bindings, vec![SqlValue::Bool(true)]);
    }

    #[test]
    fn test_boolean_active_false() {
        let result = translate_user("active eq false").unwrap();
        assert_eq!(result.where_clause, "m.active = ?");
        assert_eq!(result.bindings, vec![SqlValue::Bool(false)]);
    }

    #[test]
    fn test_boolean_not_equal() {
        let result = translate_user("active ne true").unwrap();
        assert_eq!(result.where_clause, "m.active = ?");
        assert_eq!(result.bindings, vec![SqlValue::Bool(false)]); // ne true -> = false
    }

    #[test]
    fn test_display_name() {
        let result = translate_user(r#"displayName eq "John Doe""#).unwrap();
        assert_eq!(result.where_clause, "LOWER(u.name) = LOWER(?)");
        assert_eq!(
            result.bindings,
            vec![SqlValue::String("John Doe".to_string())]
        );
    }

    #[test]
    fn test_name_formatted() {
        let result = translate_user(r#"name.formatted eq "John Doe""#).unwrap();
        assert_eq!(result.where_clause, "LOWER(u.name) = LOWER(?)");
        assert_eq!(
            result.bindings,
            vec![SqlValue::String("John Doe".to_string())]
        );
    }

    #[test]
    fn test_contains() {
        let result = translate_user(r#"displayName co "doe""#).unwrap();
        assert_eq!(
            result.where_clause,
            "LOWER(u.name) LIKE LOWER(?) ESCAPE '\\'"
        );
        assert_eq!(result.bindings, vec![SqlValue::String("%doe%".to_string())]);
    }

    #[test]
    fn test_starts_with() {
        let result = translate_user(r#"userName sw "john""#).unwrap();
        assert_eq!(
            result.where_clause,
            "LOWER(m.scim_external_id) LIKE LOWER(?) ESCAPE '\\'"
        );
        assert_eq!(result.bindings, vec![SqlValue::String("john%".to_string())]);
    }

    #[test]
    fn test_ends_with() {
        let result = translate_user(r#"userName ew "@example.com""#).unwrap();
        assert_eq!(
            result.where_clause,
            "LOWER(m.scim_external_id) LIKE LOWER(?) ESCAPE '\\'"
        );
        assert_eq!(
            result.bindings,
            vec![SqlValue::String("%@example.com".to_string())]
        );
    }

    #[test]
    fn test_like_escape() {
        // Test that special LIKE characters are escaped
        let result = translate_user(r#"displayName co "100%""#).unwrap();
        assert_eq!(
            result.bindings,
            vec![SqlValue::String("%100\\%%".to_string())]
        );

        let result = translate_user(r#"displayName co "foo_bar""#).unwrap();
        assert_eq!(
            result.bindings,
            vec![SqlValue::String("%foo\\_bar%".to_string())]
        );
    }

    #[test]
    fn test_logical_and() {
        let result = translate_user(r#"active eq true and userName sw "j""#).unwrap();
        assert_eq!(
            result.where_clause,
            "(m.active = ? AND LOWER(m.scim_external_id) LIKE LOWER(?) ESCAPE '\\')"
        );
        assert_eq!(
            result.bindings,
            vec![SqlValue::Bool(true), SqlValue::String("j%".to_string()),]
        );
    }

    #[test]
    fn test_logical_or() {
        let result = translate_user(r#"userName eq "a" or userName eq "b""#).unwrap();
        assert_eq!(
            result.where_clause,
            "(LOWER(m.scim_external_id) = LOWER(?) OR LOWER(m.scim_external_id) = LOWER(?))"
        );
        assert_eq!(
            result.bindings,
            vec![
                SqlValue::String("a".to_string()),
                SqlValue::String("b".to_string()),
            ]
        );
    }

    #[test]
    fn test_logical_not() {
        let result = translate_user("not (active eq false)").unwrap();
        assert_eq!(result.where_clause, "NOT (m.active = ?)");
        assert_eq!(result.bindings, vec![SqlValue::Bool(false)]);
    }

    #[test]
    fn test_complex_filter() {
        let result =
            translate_user(r#"(userName sw "a" or userName sw "b") and active eq true"#).unwrap();
        assert!(result.where_clause.contains("AND"));
        assert!(result.where_clause.contains("OR"));
        assert_eq!(result.bindings.len(), 3);
    }

    #[test]
    fn test_presence() {
        let result = translate_user("displayName pr").unwrap();
        assert_eq!(result.where_clause, "(u.name IS NOT NULL AND u.name != '')");
        assert!(result.bindings.is_empty());
    }

    #[test]
    fn test_presence_boolean() {
        let result = translate_user("active pr").unwrap();
        assert_eq!(result.where_clause, "m.active IS NOT NULL");
        assert!(result.bindings.is_empty());
    }

    #[test]
    fn test_null_comparison() {
        let result = translate_user("displayName eq null").unwrap();
        assert_eq!(result.where_clause, "u.name IS NULL");
        assert!(result.bindings.is_empty());

        let result = translate_user("displayName ne null").unwrap();
        assert_eq!(result.where_clause, "u.name IS NOT NULL");
        assert!(result.bindings.is_empty());
    }

    #[test]
    fn test_group_display_name() {
        let result = translate_group(r#"displayName eq "Engineering""#).unwrap();
        assert_eq!(result.where_clause, "LOWER(m.display_name) = LOWER(?)");
        assert_eq!(
            result.bindings,
            vec![SqlValue::String("Engineering".to_string())]
        );
    }

    #[test]
    fn test_group_external_id() {
        let result = translate_group(r#"externalId eq "grp-123""#).unwrap();
        assert_eq!(result.where_clause, "LOWER(m.scim_group_id) = LOWER(?)");
        assert_eq!(
            result.bindings,
            vec![SqlValue::String("grp-123".to_string())]
        );
    }

    #[test]
    fn test_unsupported_value_filter() {
        // Value filters should return None (fall back to in-memory)
        let filter = parse_filter(r#"emails[type eq "work"].value eq "john@example.com""#).unwrap();
        let result = filter_to_sql(&filter, ScimResourceType::User);
        assert!(result.is_none());
    }

    #[test]
    fn test_unsupported_members() {
        // members attribute should return None
        let filter = parse_filter(r#"members eq "user-123""#).unwrap();
        let result = filter_to_sql(&filter, ScimResourceType::Group);
        assert!(result.is_none());
    }

    #[test]
    fn test_unsupported_unknown_attr() {
        // Unknown attributes should return None
        let filter = parse_filter(r#"unknownAttr eq "value""#).unwrap();
        let result = filter_to_sql(&filter, ScimResourceType::User);
        assert!(result.is_none());
    }

    #[test]
    fn test_case_insensitive_attr_name() {
        // Attribute names should be case-insensitive
        let result1 = translate_user(r#"userName eq "john""#).unwrap();
        let result2 = translate_user(r#"USERNAME eq "john""#).unwrap();
        let result3 = translate_user(r#"UserName eq "john""#).unwrap();

        assert_eq!(result1.where_clause, result2.where_clause);
        assert_eq!(result2.where_clause, result3.where_clause);
    }

    #[test]
    fn test_emails_value() {
        // emails.value should map to u.email
        let result = translate_user(r#"emails.value eq "john@example.com""#).unwrap();
        assert_eq!(result.where_clause, "LOWER(u.email) = LOWER(?)");
    }

    #[test]
    fn test_partial_unsupported_filter() {
        // If part of an AND/OR filter is unsupported, entire filter returns None
        let filter = parse_filter(r#"userName eq "john" and members eq "user-123""#).unwrap();
        let result = filter_to_sql(&filter, ScimResourceType::User);
        // members is not a user attribute, so this should return None
        assert!(result.is_none());
    }

    #[test]
    fn test_not_equal_string() {
        let result = translate_user(r#"userName ne "admin""#).unwrap();
        assert_eq!(result.where_clause, "LOWER(m.scim_external_id) != LOWER(?)");
        assert_eq!(result.bindings, vec![SqlValue::String("admin".to_string())]);
    }

    #[test]
    fn test_escape_like_pattern() {
        assert_eq!(escape_like_pattern("hello"), "hello");
        assert_eq!(escape_like_pattern("100%"), "100\\%");
        assert_eq!(escape_like_pattern("foo_bar"), "foo\\_bar");
        assert_eq!(escape_like_pattern("a\\b"), "a\\\\b");
        assert_eq!(escape_like_pattern("a%_\\b"), "a\\%\\_\\\\b");
    }
}
