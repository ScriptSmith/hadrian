//! Attribute filter types for vector store search.
//!
//! These types implement OpenAI's filter schema for attribute-based filtering
//! in vector store searches. Filters operate on file attributes (key-value pairs
//! attached to vector store files).
//!
//! # Filter Types
//!
//! - [`ComparisonFilter`]: Compare a single attribute against a value
//! - [`CompoundFilter`]: Combine multiple filters with `and`/`or` logic
//! - [`AttributeFilter`]: Union type representing either filter type
//!
//! # Example
//!
//! ```json
//! {
//!   "type": "and",
//!   "filters": [
//!     { "type": "eq", "key": "author", "value": "John Doe" },
//!     { "type": "gte", "key": "date", "value": 1704067200 }
//!   ]
//! }
//! ```
//!
//! # OpenAI Compatibility
//!
//! This schema matches the OpenAI Vector Stores API filter specification exactly.
//! See: <https://platform.openai.com/docs/api-reference/vector-stores/search>

use serde::{Deserialize, Serialize};

/// Comparison operators for attribute filtering.
///
/// These operators compare an attribute's value against a target value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum ComparisonOperator {
    /// Equal to
    Eq,
    /// Not equal to
    Ne,
    /// Greater than
    Gt,
    /// Greater than or equal to
    Gte,
    /// Less than
    Lt,
    /// Less than or equal to
    Lte,
}

impl std::fmt::Display for ComparisonOperator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Eq => write!(f, "eq"),
            Self::Ne => write!(f, "ne"),
            Self::Gt => write!(f, "gt"),
            Self::Gte => write!(f, "gte"),
            Self::Lt => write!(f, "lt"),
            Self::Lte => write!(f, "lte"),
        }
    }
}

/// A value that can be compared in a filter.
///
/// Supports string, number (as f64), boolean, and arrays for `in`/`nin` operations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(untagged)]
pub enum FilterValue {
    /// String value
    String(String),
    /// Numeric value (integer or float)
    Number(f64),
    /// Boolean value
    Boolean(bool),
    /// Array of values (for future `in`/`nin` support)
    Array(Vec<FilterValueItem>),
}

/// Items within a filter value array.
///
/// Used for `in`/`nin` operators (future support).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(untagged)]
pub enum FilterValueItem {
    /// String item
    String(String),
    /// Numeric item
    Number(f64),
}

/// A comparison filter for attribute-based filtering.
///
/// Compares a specific attribute key to a given value using a comparison operator.
///
/// # Example
///
/// ```json
/// {
///   "type": "eq",
///   "key": "author",
///   "value": "John Doe"
/// }
/// ```
///
/// # Supported Comparisons
///
/// | Operator | Description |
/// |----------|-------------|
/// | `eq` | Equal to |
/// | `ne` | Not equal to |
/// | `gt` | Greater than |
/// | `gte` | Greater than or equal to |
/// | `lt` | Less than |
/// | `lte` | Less than or equal to |
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ComparisonFilter {
    /// The comparison operator to use.
    #[serde(rename = "type")]
    pub operator: ComparisonOperator,
    /// The attribute key to compare.
    pub key: String,
    /// The value to compare against.
    pub value: FilterValue,
}

/// Logical operators for compound filters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum LogicalOperator {
    /// All filters must match (logical AND)
    And,
    /// At least one filter must match (logical OR)
    Or,
}

impl std::fmt::Display for LogicalOperator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::And => write!(f, "and"),
            Self::Or => write!(f, "or"),
        }
    }
}

/// A compound filter that combines multiple filters with logical operators.
///
/// Allows building complex filter expressions by combining comparison filters
/// and/or other compound filters using `and` or `or` logic.
///
/// # Example
///
/// ```json
/// {
///   "type": "and",
///   "filters": [
///     { "type": "eq", "key": "category", "value": "documentation" },
///     {
///       "type": "or",
///       "filters": [
///         { "type": "eq", "key": "author", "value": "Alice" },
///         { "type": "eq", "key": "author", "value": "Bob" }
///       ]
///     }
///   ]
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CompoundFilter {
    /// The logical operator (`and` or `or`).
    #[serde(rename = "type")]
    pub operator: LogicalOperator,
    /// The filters to combine.
    pub filters: Vec<AttributeFilter>,
}

/// A filter for attribute-based search filtering.
///
/// This is a union type that can be either a [`ComparisonFilter`] for simple
/// comparisons or a [`CompoundFilter`] for combining multiple filters.
///
/// # Deserialization
///
/// The filter type is determined by the `type` field:
/// - `eq`, `ne`, `gt`, `gte`, `lt`, `lte` → [`ComparisonFilter`]
/// - `and`, `or` → [`CompoundFilter`]
///
/// # Example: Simple comparison
///
/// ```json
/// { "type": "eq", "key": "status", "value": "published" }
/// ```
///
/// # Example: Compound filter
///
/// ```json
/// {
///   "type": "and",
///   "filters": [
///     { "type": "gte", "key": "date", "value": 1704067200 },
///     { "type": "lte", "key": "date", "value": 1706745600 }
///   ]
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(untagged)]
#[cfg_attr(feature = "utoipa", schema(no_recursion))]
pub enum AttributeFilter {
    /// A simple comparison filter
    Comparison(ComparisonFilter),
    /// A compound filter combining multiple filters
    Compound(CompoundFilter),
}

impl AttributeFilter {
    /// Create an equality comparison filter.
    pub fn eq(key: impl Into<String>, value: impl Into<FilterValue>) -> Self {
        Self::Comparison(ComparisonFilter {
            operator: ComparisonOperator::Eq,
            key: key.into(),
            value: value.into(),
        })
    }

    /// Create a not-equal comparison filter.
    pub fn ne(key: impl Into<String>, value: impl Into<FilterValue>) -> Self {
        Self::Comparison(ComparisonFilter {
            operator: ComparisonOperator::Ne,
            key: key.into(),
            value: value.into(),
        })
    }

    /// Create a greater-than comparison filter.
    pub fn gt(key: impl Into<String>, value: impl Into<FilterValue>) -> Self {
        Self::Comparison(ComparisonFilter {
            operator: ComparisonOperator::Gt,
            key: key.into(),
            value: value.into(),
        })
    }

    /// Create a greater-than-or-equal comparison filter.
    pub fn gte(key: impl Into<String>, value: impl Into<FilterValue>) -> Self {
        Self::Comparison(ComparisonFilter {
            operator: ComparisonOperator::Gte,
            key: key.into(),
            value: value.into(),
        })
    }

    /// Create a less-than comparison filter.
    pub fn lt(key: impl Into<String>, value: impl Into<FilterValue>) -> Self {
        Self::Comparison(ComparisonFilter {
            operator: ComparisonOperator::Lt,
            key: key.into(),
            value: value.into(),
        })
    }

    /// Create a less-than-or-equal comparison filter.
    pub fn lte(key: impl Into<String>, value: impl Into<FilterValue>) -> Self {
        Self::Comparison(ComparisonFilter {
            operator: ComparisonOperator::Lte,
            key: key.into(),
            value: value.into(),
        })
    }

    /// Combine filters with AND logic.
    pub fn and(filters: Vec<AttributeFilter>) -> Self {
        Self::Compound(CompoundFilter {
            operator: LogicalOperator::And,
            filters,
        })
    }

    /// Combine filters with OR logic.
    pub fn or(filters: Vec<AttributeFilter>) -> Self {
        Self::Compound(CompoundFilter {
            operator: LogicalOperator::Or,
            filters,
        })
    }
}

// Convenience conversions for FilterValue
impl From<String> for FilterValue {
    fn from(s: String) -> Self {
        Self::String(s)
    }
}

impl From<&str> for FilterValue {
    fn from(s: &str) -> Self {
        Self::String(s.to_string())
    }
}

impl From<f64> for FilterValue {
    fn from(n: f64) -> Self {
        Self::Number(n)
    }
}

impl From<i64> for FilterValue {
    fn from(n: i64) -> Self {
        Self::Number(n as f64)
    }
}

impl From<i32> for FilterValue {
    fn from(n: i32) -> Self {
        Self::Number(n as f64)
    }
}

impl From<bool> for FilterValue {
    fn from(b: bool) -> Self {
        Self::Boolean(b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_comparison_filter_serialization() {
        let filter = ComparisonFilter {
            operator: ComparisonOperator::Eq,
            key: "author".to_string(),
            value: FilterValue::String("John Doe".to_string()),
        };

        let json = serde_json::to_string(&filter).unwrap();
        assert!(json.contains(r#""type":"eq""#));
        assert!(json.contains(r#""key":"author""#));
        assert!(json.contains(r#""value":"John Doe""#));
    }

    #[test]
    fn test_comparison_filter_deserialization() {
        let json = r#"{"type": "gte", "key": "date", "value": 1704067200}"#;
        let filter: ComparisonFilter = serde_json::from_str(json).unwrap();

        assert_eq!(filter.operator, ComparisonOperator::Gte);
        assert_eq!(filter.key, "date");
        assert_eq!(filter.value, FilterValue::Number(1704067200.0));
    }

    #[test]
    fn test_compound_filter_serialization() {
        let filter = CompoundFilter {
            operator: LogicalOperator::And,
            filters: vec![
                AttributeFilter::eq("author", "Alice"),
                AttributeFilter::gte("date", 1704067200),
            ],
        };

        let json = serde_json::to_string(&filter).unwrap();
        assert!(json.contains(r#""type":"and""#));
        assert!(json.contains(r#""filters""#));
    }

    #[test]
    fn test_compound_filter_deserialization() {
        let json = r#"{
            "type": "and",
            "filters": [
                {"type": "eq", "key": "author", "value": "Alice"},
                {"type": "gte", "key": "date", "value": 1704067200}
            ]
        }"#;
        let filter: CompoundFilter = serde_json::from_str(json).unwrap();

        assert_eq!(filter.operator, LogicalOperator::And);
        assert_eq!(filter.filters.len(), 2);
    }

    #[test]
    fn test_attribute_filter_comparison_deserialization() {
        let json = r#"{"type": "eq", "key": "status", "value": "published"}"#;
        let filter: AttributeFilter = serde_json::from_str(json).unwrap();

        match filter {
            AttributeFilter::Comparison(c) => {
                assert_eq!(c.operator, ComparisonOperator::Eq);
                assert_eq!(c.key, "status");
                assert_eq!(c.value, FilterValue::String("published".to_string()));
            }
            _ => panic!("Expected Comparison variant"),
        }
    }

    #[test]
    fn test_attribute_filter_compound_deserialization() {
        let json = r#"{
            "type": "or",
            "filters": [
                {"type": "eq", "key": "category", "value": "docs"},
                {"type": "eq", "key": "category", "value": "guide"}
            ]
        }"#;
        let filter: AttributeFilter = serde_json::from_str(json).unwrap();

        match filter {
            AttributeFilter::Compound(c) => {
                assert_eq!(c.operator, LogicalOperator::Or);
                assert_eq!(c.filters.len(), 2);
            }
            _ => panic!("Expected Compound variant"),
        }
    }

    #[test]
    fn test_nested_compound_filter() {
        let json = r#"{
            "type": "and",
            "filters": [
                {"type": "eq", "key": "category", "value": "documentation"},
                {
                    "type": "or",
                    "filters": [
                        {"type": "eq", "key": "author", "value": "Alice"},
                        {"type": "eq", "key": "author", "value": "Bob"}
                    ]
                }
            ]
        }"#;
        let filter: AttributeFilter = serde_json::from_str(json).unwrap();

        match filter {
            AttributeFilter::Compound(c) => {
                assert_eq!(c.operator, LogicalOperator::And);
                assert_eq!(c.filters.len(), 2);

                // Check nested compound filter
                match &c.filters[1] {
                    AttributeFilter::Compound(nested) => {
                        assert_eq!(nested.operator, LogicalOperator::Or);
                        assert_eq!(nested.filters.len(), 2);
                    }
                    _ => panic!("Expected nested Compound variant"),
                }
            }
            _ => panic!("Expected Compound variant"),
        }
    }

    #[test]
    fn test_builder_methods() {
        let filter = AttributeFilter::and(vec![
            AttributeFilter::eq("author", "John"),
            AttributeFilter::gte("date", 1704067200),
            AttributeFilter::lt("score", 0.5),
        ]);

        match filter {
            AttributeFilter::Compound(c) => {
                assert_eq!(c.operator, LogicalOperator::And);
                assert_eq!(c.filters.len(), 3);
            }
            _ => panic!("Expected Compound variant"),
        }
    }

    #[test]
    fn test_boolean_value() {
        let json = r#"{"type": "eq", "key": "is_published", "value": true}"#;
        let filter: ComparisonFilter = serde_json::from_str(json).unwrap();

        assert_eq!(filter.value, FilterValue::Boolean(true));
    }

    #[test]
    fn test_filter_value_from_conversions() {
        assert_eq!(
            FilterValue::from("test"),
            FilterValue::String("test".to_string())
        );
        assert_eq!(FilterValue::from(42i32), FilterValue::Number(42.0));
        assert_eq!(FilterValue::from(42i64), FilterValue::Number(42.0));
        assert_eq!(FilterValue::from(2.71f64), FilterValue::Number(2.71));
        assert_eq!(FilterValue::from(true), FilterValue::Boolean(true));
    }
}
