//! SCIM 2.0 PATCH Operations
//!
//! This module implements parsing and execution of SCIM PATCH operations per RFC 7644 Section 3.5.2.
//!
//! ## Operations
//!
//! - `add`: Add value(s) to an attribute
//! - `remove`: Remove attribute or specific value from multi-valued attribute
//! - `replace`: Replace attribute value
//!
//! ## Path Syntax
//!
//! ```text
//! path = attrPath / valuePath / subAttrPath
//! attrPath = ATTRNAME
//! subAttrPath = ATTRNAME "." ATTRNAME
//! valuePath = ATTRNAME "[" valueFilter "]" ["." ATTRNAME]
//! ```
//!
//! ## Examples
//!
//! ```json
//! {
//!   "schemas": ["urn:ietf:params:scim:api:messages:2.0:PatchOp"],
//!   "Operations": [
//!     { "op": "replace", "path": "displayName", "value": "New Name" },
//!     { "op": "add", "path": "emails", "value": [{"type": "home", "value": "home@example.com"}] },
//!     { "op": "remove", "path": "members[value eq \"user-123\"]" }
//!   ]
//! }
//! ```

use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{
    filter::{AttrPath, CompareOp, Filter, FilterValue, parse_filter},
    types::SCHEMA_PATCH_OP,
};

/// A SCIM PATCH request containing one or more operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchRequest {
    /// SCIM schema URIs (should contain PatchOp schema)
    pub schemas: Vec<String>,

    /// List of patch operations to apply
    #[serde(rename = "Operations")]
    pub operations: Vec<PatchOp>,
}

impl PatchRequest {
    /// Create a new patch request with operations
    pub fn new(operations: Vec<PatchOp>) -> Self {
        Self {
            schemas: vec![SCHEMA_PATCH_OP.to_string()],
            operations,
        }
    }

    /// Validate the request
    pub fn validate(&self) -> Result<(), PatchError> {
        if !self.schemas.iter().any(|s| s == SCHEMA_PATCH_OP) {
            return Err(PatchError::InvalidSchema);
        }

        for (i, op) in self.operations.iter().enumerate() {
            op.validate().map_err(|e| PatchError::InvalidOperation {
                index: i,
                error: Box::new(e),
            })?;
        }

        Ok(())
    }
}

/// A single SCIM PATCH operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "lowercase")]
pub enum PatchOp {
    /// Add value(s) to an attribute
    Add {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        path: Option<String>,
        value: Value,
    },
    /// Replace attribute value
    Replace {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        path: Option<String>,
        value: Value,
    },
    /// Remove attribute or value
    Remove { path: String },
}

impl PatchOp {
    /// Create an add operation
    pub fn add(path: impl Into<String>, value: Value) -> Self {
        PatchOp::Add {
            path: Some(path.into()),
            value,
        }
    }

    /// Create a replace operation
    pub fn replace(path: impl Into<String>, value: Value) -> Self {
        PatchOp::Replace {
            path: Some(path.into()),
            value,
        }
    }

    /// Create a remove operation
    pub fn remove(path: impl Into<String>) -> Self {
        PatchOp::Remove { path: path.into() }
    }

    /// Validate the operation
    pub fn validate(&self) -> Result<(), PatchError> {
        match self {
            PatchOp::Add { path, .. } | PatchOp::Replace { path, .. } => {
                if let Some(p) = path {
                    parse_path(p)?;
                }
                Ok(())
            }
            PatchOp::Remove { path } => {
                if path.is_empty() {
                    return Err(PatchError::NoTarget);
                }
                parse_path(path)?;
                Ok(())
            }
        }
    }

    /// Get the parsed path if present
    pub fn parsed_path(&self) -> Result<Option<PatchPath>, PatchError> {
        match self {
            PatchOp::Add { path, .. } | PatchOp::Replace { path, .. } => {
                path.as_ref().map(|p| parse_path(p)).transpose()
            }
            PatchOp::Remove { path } => Ok(Some(parse_path(path)?)),
        }
    }
}

/// A parsed SCIM PATCH path.
#[derive(Debug, Clone, PartialEq)]
pub struct PatchPath {
    /// Main attribute name
    pub attr: String,
    /// Sub-attribute (for nested paths like "name.familyName")
    pub sub_attr: Option<String>,
    /// Value filter for multi-valued attributes (e.g., `[type eq "work"]`)
    pub value_filter: Option<Filter>,
}

impl PatchPath {
    /// Create a simple path
    pub fn simple(attr: impl Into<String>) -> Self {
        Self {
            attr: attr.into(),
            sub_attr: None,
            value_filter: None,
        }
    }

    /// Create a nested path (e.g., "name.familyName")
    pub fn nested(attr: impl Into<String>, sub_attr: impl Into<String>) -> Self {
        Self {
            attr: attr.into(),
            sub_attr: Some(sub_attr.into()),
            value_filter: None,
        }
    }

    /// Convert from AttrPath
    pub fn from_attr_path(attr_path: AttrPath) -> Self {
        Self {
            attr: attr_path.attr,
            sub_attr: attr_path.sub_attr,
            value_filter: attr_path.value_filter.map(|f| *f),
        }
    }
}

impl fmt::Display for PatchPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.attr)?;
        if let Some(ref filter) = self.value_filter {
            write!(f, "[{}]", filter)?;
        }
        if let Some(ref sub) = self.sub_attr {
            write!(f, ".{}", sub)?;
        }
        Ok(())
    }
}

/// PATCH operation errors.
#[derive(Debug, Clone, PartialEq)]
pub enum PatchError {
    /// Invalid schema in request
    InvalidSchema,
    /// Remove operation missing required path
    NoTarget,
    /// Invalid path syntax
    InvalidPath(String),
    /// Path references an invalid attribute
    InvalidAttribute(String),
    /// Attempt to modify immutable attribute
    Immutable(String),
    /// Invalid value for attribute type
    InvalidValue(String),
    /// Target resource not found
    NotFound(String),
    /// Invalid operation at index
    InvalidOperation {
        index: usize,
        error: Box<PatchError>,
    },
}

impl fmt::Display for PatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PatchError::InvalidSchema => write!(f, "Request must include PatchOp schema"),
            PatchError::NoTarget => {
                write!(f, "Remove operation requires a path")
            }
            PatchError::InvalidPath(msg) => write!(f, "Invalid path: {}", msg),
            PatchError::InvalidAttribute(attr) => write!(f, "Invalid attribute: {}", attr),
            PatchError::Immutable(attr) => write!(f, "Attribute '{}' is immutable", attr),
            PatchError::InvalidValue(msg) => write!(f, "Invalid value: {}", msg),
            PatchError::NotFound(msg) => write!(f, "Target not found: {}", msg),
            PatchError::InvalidOperation { index, error } => {
                write!(f, "Invalid operation at index {}: {}", index, error)
            }
        }
    }
}

impl std::error::Error for PatchError {}

impl From<super::filter::FilterParseError> for PatchError {
    fn from(e: super::filter::FilterParseError) -> Self {
        PatchError::InvalidPath(e.to_string())
    }
}

/// Parse a SCIM PATCH path string.
///
/// # Examples
///
/// ```
/// use gateway::scim::patch::parse_path;
///
/// let path = parse_path("displayName").unwrap();
/// let path = parse_path("name.familyName").unwrap();
/// let path = parse_path("emails[type eq \"work\"].value").unwrap();
/// ```
pub fn parse_path(input: &str) -> Result<PatchPath, PatchError> {
    let input = input.trim();

    if input.is_empty() {
        return Err(PatchError::InvalidPath("Empty path".to_string()));
    }

    // Check for value filter
    if let Some(bracket_pos) = input.find('[') {
        let attr = &input[..bracket_pos];

        // Find matching close bracket
        let close_bracket = input
            .rfind(']')
            .ok_or_else(|| PatchError::InvalidPath("Unclosed bracket in path".to_string()))?;

        let filter_str = &input[bracket_pos + 1..close_bracket];

        // Parse the filter
        let filter = parse_filter(filter_str)
            .map_err(|e| PatchError::InvalidPath(format!("Invalid value filter: {}", e)))?;

        // Check for sub-attribute after the bracket
        let sub_attr = if close_bracket + 1 < input.len() {
            let remaining = &input[close_bracket + 1..];
            if let Some(stripped) = remaining.strip_prefix('.') {
                Some(stripped.to_string())
            } else {
                return Err(PatchError::InvalidPath(format!(
                    "Unexpected characters after filter: '{}'",
                    remaining
                )));
            }
        } else {
            None
        };

        return Ok(PatchPath {
            attr: attr.to_string(),
            sub_attr,
            value_filter: Some(filter),
        });
    }

    // Check for nested attribute (e.g., "name.familyName")
    if let Some(dot_pos) = input.find('.') {
        let attr = &input[..dot_pos];
        let sub_attr = &input[dot_pos + 1..];

        if sub_attr.is_empty() {
            return Err(PatchError::InvalidPath(
                "Sub-attribute cannot be empty".to_string(),
            ));
        }

        return Ok(PatchPath {
            attr: attr.to_string(),
            sub_attr: Some(sub_attr.to_string()),
            value_filter: None,
        });
    }

    // Simple attribute path
    Ok(PatchPath {
        attr: input.to_string(),
        sub_attr: None,
        value_filter: None,
    })
}

/// Evaluate a value filter against a JSON object.
///
/// Returns true if the object matches the filter.
pub fn matches_filter(filter: &Filter, obj: &Value) -> bool {
    match filter {
        Filter::Compare { attr, op, value } => {
            let attr_value = get_attr_value(obj, &attr.attr, attr.sub_attr.as_deref());
            compare_values(attr_value, op, value)
        }
        Filter::Present { attr } => {
            let attr_value = get_attr_value(obj, &attr.attr, attr.sub_attr.as_deref());
            !attr_value.is_null()
        }
        Filter::And(left, right) => matches_filter(left, obj) && matches_filter(right, obj),
        Filter::Or(left, right) => matches_filter(left, obj) || matches_filter(right, obj),
        Filter::Not(inner) => !matches_filter(inner, obj),
    }
}

fn get_attr_value<'a>(obj: &'a Value, attr: &str, sub_attr: Option<&str>) -> &'a Value {
    // Try camelCase first, then lowercase
    let value = obj.get(attr).or_else(|| obj.get(attr.to_lowercase()));

    match (value, sub_attr) {
        (Some(v), Some(sub)) => v
            .get(sub)
            .or_else(|| v.get(sub.to_lowercase()))
            .unwrap_or(&Value::Null),
        (Some(v), None) => v,
        (None, _) => &Value::Null,
    }
}

fn compare_values(json_value: &Value, op: &CompareOp, filter_value: &FilterValue) -> bool {
    match (json_value, filter_value) {
        (Value::String(s), FilterValue::String(fs)) => {
            let s_lower = s.to_lowercase();
            let fs_lower = fs.to_lowercase();
            match op {
                CompareOp::Eq => s_lower == fs_lower,
                CompareOp::Ne => s_lower != fs_lower,
                CompareOp::Co => s_lower.contains(&fs_lower),
                CompareOp::Sw => s_lower.starts_with(&fs_lower),
                CompareOp::Ew => s_lower.ends_with(&fs_lower),
                CompareOp::Gt => s > fs,
                CompareOp::Ge => s >= fs,
                CompareOp::Lt => s < fs,
                CompareOp::Le => s <= fs,
            }
        }
        (Value::Bool(b), FilterValue::Bool(fb)) => match op {
            CompareOp::Eq => b == fb,
            CompareOp::Ne => b != fb,
            _ => false,
        },
        (Value::Number(n), FilterValue::Number(fn_)) => {
            let n = n.as_f64().unwrap_or(f64::NAN);
            match op {
                CompareOp::Eq => (n - fn_).abs() < f64::EPSILON,
                CompareOp::Ne => (n - fn_).abs() >= f64::EPSILON,
                CompareOp::Gt => n > *fn_,
                CompareOp::Ge => n >= *fn_,
                CompareOp::Lt => n < *fn_,
                CompareOp::Le => n <= *fn_,
                _ => false,
            }
        }
        (Value::Null, FilterValue::Null) => matches!(op, CompareOp::Eq),
        (_, FilterValue::Null) => matches!(op, CompareOp::Ne),
        _ => false,
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn test_parse_simple_path() {
        let path = parse_path("displayName").unwrap();
        assert_eq!(path.attr, "displayName");
        assert_eq!(path.sub_attr, None);
        assert!(path.value_filter.is_none());
    }

    #[test]
    fn test_parse_nested_path() {
        let path = parse_path("name.familyName").unwrap();
        assert_eq!(path.attr, "name");
        assert_eq!(path.sub_attr, Some("familyName".to_string()));
        assert!(path.value_filter.is_none());
    }

    #[test]
    fn test_parse_path_with_filter() {
        let path = parse_path("emails[type eq \"work\"]").unwrap();
        assert_eq!(path.attr, "emails");
        assert!(path.value_filter.is_some());
        assert_eq!(path.sub_attr, None);
    }

    #[test]
    fn test_parse_path_with_filter_and_subattr() {
        let path = parse_path("emails[type eq \"work\"].value").unwrap();
        assert_eq!(path.attr, "emails");
        assert!(path.value_filter.is_some());
        assert_eq!(path.sub_attr, Some("value".to_string()));
    }

    #[test]
    fn test_parse_path_empty() {
        let result = parse_path("");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_path_unclosed_bracket() {
        let result = parse_path("emails[type eq \"work\"");
        assert!(result.is_err());
    }

    #[test]
    fn test_patch_request_serialization() {
        let request = PatchRequest::new(vec![
            PatchOp::replace("displayName", json!("New Name")),
            PatchOp::add(
                "emails",
                json!([{"type": "home", "value": "home@example.com"}]),
            ),
            PatchOp::remove("members[value eq \"user-123\"]"),
        ]);

        let json = serde_json::to_string_pretty(&request).unwrap();
        assert!(json.contains("\"op\": \"replace\""));
        assert!(json.contains("\"op\": \"add\""));
        assert!(json.contains("\"op\": \"remove\""));
    }

    #[test]
    fn test_patch_request_deserialization() {
        let json = r#"{
            "schemas": ["urn:ietf:params:scim:api:messages:2.0:PatchOp"],
            "Operations": [
                {"op": "replace", "path": "displayName", "value": "New Name"},
                {"op": "add", "path": "emails", "value": [{"type": "work", "value": "work@example.com"}]},
                {"op": "remove", "path": "groups[value eq \"group-123\"]"}
            ]
        }"#;

        let request: PatchRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.operations.len(), 3);

        match &request.operations[0] {
            PatchOp::Replace { path, value } => {
                assert_eq!(path.as_deref(), Some("displayName"));
                assert_eq!(value, &json!("New Name"));
            }
            _ => panic!("Expected Replace operation"),
        }

        match &request.operations[2] {
            PatchOp::Remove { path } => {
                assert_eq!(path, "groups[value eq \"group-123\"]");
            }
            _ => panic!("Expected Remove operation"),
        }
    }

    #[test]
    fn test_patch_request_validation() {
        let valid_request =
            PatchRequest::new(vec![PatchOp::replace("displayName", json!("New Name"))]);
        assert!(valid_request.validate().is_ok());

        // Remove without path is invalid
        let invalid_request = PatchRequest::new(vec![PatchOp::Remove {
            path: String::new(),
        }]);
        assert!(invalid_request.validate().is_err());
    }

    #[test]
    fn test_patch_op_validate() {
        // Valid operations
        assert!(
            PatchOp::add("displayName", json!("test"))
                .validate()
                .is_ok()
        );
        assert!(
            PatchOp::replace("name.familyName", json!("Doe"))
                .validate()
                .is_ok()
        );
        assert!(
            PatchOp::remove("emails[type eq \"work\"]")
                .validate()
                .is_ok()
        );

        // Invalid path
        let op = PatchOp::Add {
            path: Some("emails[invalid".to_string()),
            value: json!("test"),
        };
        assert!(op.validate().is_err());
    }

    #[test]
    fn test_matches_filter_string_eq() {
        let filter = parse_filter("type eq \"work\"").unwrap();
        let obj = json!({"type": "work", "value": "test@example.com"});
        assert!(matches_filter(&filter, &obj));

        let obj = json!({"type": "home", "value": "test@example.com"});
        assert!(!matches_filter(&filter, &obj));
    }

    #[test]
    fn test_matches_filter_case_insensitive() {
        let filter = parse_filter("type eq \"WORK\"").unwrap();
        let obj = json!({"type": "work", "value": "test@example.com"});
        assert!(matches_filter(&filter, &obj));
    }

    #[test]
    fn test_matches_filter_boolean() {
        let filter = parse_filter("primary eq true").unwrap();
        let obj = json!({"type": "work", "primary": true});
        assert!(matches_filter(&filter, &obj));

        let obj = json!({"type": "work", "primary": false});
        assert!(!matches_filter(&filter, &obj));
    }

    #[test]
    fn test_matches_filter_contains() {
        let filter = parse_filter("value co \"example\"").unwrap();
        let obj = json!({"value": "test@example.com"});
        assert!(matches_filter(&filter, &obj));

        let obj = json!({"value": "test@other.com"});
        assert!(!matches_filter(&filter, &obj));
    }

    #[test]
    fn test_matches_filter_starts_with() {
        let filter = parse_filter("value sw \"test\"").unwrap();
        let obj = json!({"value": "test@example.com"});
        assert!(matches_filter(&filter, &obj));

        let obj = json!({"value": "other@example.com"});
        assert!(!matches_filter(&filter, &obj));
    }

    #[test]
    fn test_matches_filter_ends_with() {
        let filter = parse_filter("value ew \"example.com\"").unwrap();
        let obj = json!({"value": "test@example.com"});
        assert!(matches_filter(&filter, &obj));

        let obj = json!({"value": "test@other.com"});
        assert!(!matches_filter(&filter, &obj));
    }

    #[test]
    fn test_matches_filter_present() {
        let filter = parse_filter("primary pr").unwrap();
        let obj = json!({"type": "work", "primary": true});
        assert!(matches_filter(&filter, &obj));

        let obj = json!({"type": "work"});
        assert!(!matches_filter(&filter, &obj));
    }

    #[test]
    fn test_matches_filter_and() {
        let filter = parse_filter("type eq \"work\" and primary eq true").unwrap();
        let obj = json!({"type": "work", "primary": true});
        assert!(matches_filter(&filter, &obj));

        let obj = json!({"type": "work", "primary": false});
        assert!(!matches_filter(&filter, &obj));

        let obj = json!({"type": "home", "primary": true});
        assert!(!matches_filter(&filter, &obj));
    }

    #[test]
    fn test_matches_filter_or() {
        let filter = parse_filter("type eq \"work\" or type eq \"home\"").unwrap();
        let obj = json!({"type": "work"});
        assert!(matches_filter(&filter, &obj));

        let obj = json!({"type": "home"});
        assert!(matches_filter(&filter, &obj));

        let obj = json!({"type": "other"});
        assert!(!matches_filter(&filter, &obj));
    }

    #[test]
    fn test_matches_filter_not() {
        let filter = parse_filter("not (type eq \"home\")").unwrap();
        let obj = json!({"type": "work"});
        assert!(matches_filter(&filter, &obj));

        let obj = json!({"type": "home"});
        assert!(!matches_filter(&filter, &obj));
    }

    #[test]
    fn test_patch_path_display() {
        let path = PatchPath::simple("displayName");
        assert_eq!(format!("{}", path), "displayName");

        let path = PatchPath::nested("name", "familyName");
        assert_eq!(format!("{}", path), "name.familyName");
    }
}
