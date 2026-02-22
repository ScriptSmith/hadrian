//! SCIM 2.0 Filter Parser
//!
//! This module implements a parser for SCIM filter expressions per RFC 7644 Section 3.4.2.
//!
//! ## Grammar (simplified)
//!
//! ```text
//! filter     = logExpr
//! logExpr    = andExpr { "or" andExpr }
//! andExpr    = notExpr { "and" notExpr }
//! notExpr    = "not" "(" filter ")" | "(" filter ")" | attrExpr
//! attrExpr   = attrPath "pr" | attrPath compareOp compValue
//! attrPath   = ATTRNAME ["." ATTRNAME] ["[" valFilter "]"]
//! valFilter  = attrPath compareOp compValue
//! compareOp  = "eq" | "ne" | "co" | "sw" | "ew" | "gt" | "ge" | "lt" | "le"
//! compValue  = "true" | "false" | "null" | NUMBER | STRING
//! ```
//!
//! ## Examples
//!
//! ```text
//! userName eq "john"
//! active eq true
//! name.familyName co "doe"
//! emails[type eq "work"].value sw "john"
//! userName eq "john" and active eq true
//! not (active eq false)
//! ```
//!
//! ## Security Limits
//!
//! To prevent DoS attacks from malicious filter expressions:
//! - Maximum filter length: 4096 bytes
//! - Maximum nesting depth: 32 levels

use std::fmt;

use serde::{Deserialize, Serialize};

/// Maximum allowed length of a SCIM filter expression (bytes).
///
/// This limit prevents excessive memory usage and CPU time when parsing
/// maliciously crafted filter expressions. 4KB is generous for any real-world
/// SCIM filter while providing protection against abuse.
pub const MAX_FILTER_LENGTH: usize = 4096;

/// Maximum allowed nesting depth of a SCIM filter expression.
///
/// This limit prevents stack overflow from deeply nested expressions like
/// `not (not (not (...)))` or `a[b[c[...]]]`. 32 levels is well beyond any
/// legitimate use case.
pub const MAX_FILTER_DEPTH: usize = 32;

/// A parsed SCIM filter expression.
#[derive(Debug, Clone, PartialEq)]
pub enum Filter {
    /// Attribute comparison (e.g., `userName eq "john"`)
    Compare {
        attr: AttrPath,
        op: CompareOp,
        value: FilterValue,
    },
    /// Attribute presence check (e.g., `name pr`)
    Present { attr: AttrPath },
    /// Logical AND of two filters
    And(Box<Filter>, Box<Filter>),
    /// Logical OR of two filters
    Or(Box<Filter>, Box<Filter>),
    /// Logical NOT of a filter
    Not(Box<Filter>),
}

impl fmt::Display for Filter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Filter::Compare { attr, op, value } => write!(f, "{} {} {}", attr, op, value),
            Filter::Present { attr } => write!(f, "{} pr", attr),
            Filter::And(left, right) => write!(f, "({} and {})", left, right),
            Filter::Or(left, right) => write!(f, "({} or {})", left, right),
            Filter::Not(inner) => write!(f, "not ({})", inner),
        }
    }
}

/// An attribute path, optionally with sub-attribute and value filter.
#[derive(Debug, Clone, PartialEq)]
pub struct AttrPath {
    /// Main attribute name (e.g., "userName", "emails")
    pub attr: String,
    /// Sub-attribute for complex types (e.g., "familyName" in "name.familyName")
    pub sub_attr: Option<String>,
    /// Value filter for multi-valued attributes (e.g., `[type eq "work"]`)
    pub value_filter: Option<Box<Filter>>,
}

impl AttrPath {
    /// Create a simple attribute path
    pub fn simple(attr: impl Into<String>) -> Self {
        Self {
            attr: attr.into(),
            sub_attr: None,
            value_filter: None,
        }
    }

    /// Create a nested attribute path (e.g., "name.familyName")
    pub fn nested(attr: impl Into<String>, sub_attr: impl Into<String>) -> Self {
        Self {
            attr: attr.into(),
            sub_attr: Some(sub_attr.into()),
            value_filter: None,
        }
    }
}

impl fmt::Display for AttrPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.attr)?;
        if let Some(filter) = &self.value_filter {
            write!(f, "[{}]", filter)?;
        }
        if let Some(sub) = &self.sub_attr {
            write!(f, ".{}", sub)?;
        }
        Ok(())
    }
}

/// Comparison operators per RFC 7644.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CompareOp {
    /// Equal
    Eq,
    /// Not equal
    Ne,
    /// Contains
    Co,
    /// Starts with
    Sw,
    /// Ends with
    Ew,
    /// Greater than
    Gt,
    /// Greater than or equal
    Ge,
    /// Less than
    Lt,
    /// Less than or equal
    Le,
}

impl fmt::Display for CompareOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            CompareOp::Eq => "eq",
            CompareOp::Ne => "ne",
            CompareOp::Co => "co",
            CompareOp::Sw => "sw",
            CompareOp::Ew => "ew",
            CompareOp::Gt => "gt",
            CompareOp::Ge => "ge",
            CompareOp::Lt => "lt",
            CompareOp::Le => "le",
        };
        write!(f, "{}", s)
    }
}

impl CompareOp {
    fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "eq" => Some(CompareOp::Eq),
            "ne" => Some(CompareOp::Ne),
            "co" => Some(CompareOp::Co),
            "sw" => Some(CompareOp::Sw),
            "ew" => Some(CompareOp::Ew),
            "gt" => Some(CompareOp::Gt),
            "ge" => Some(CompareOp::Ge),
            "lt" => Some(CompareOp::Lt),
            "le" => Some(CompareOp::Le),
            _ => None,
        }
    }
}

/// Filter comparison values.
#[derive(Debug, Clone, PartialEq)]
pub enum FilterValue {
    String(String),
    Bool(bool),
    Number(f64),
    Null,
}

impl fmt::Display for FilterValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FilterValue::String(s) => write!(f, "\"{}\"", s.replace('"', "\\\"")),
            FilterValue::Bool(b) => write!(f, "{}", b),
            FilterValue::Number(n) => write!(f, "{}", n),
            FilterValue::Null => write!(f, "null"),
        }
    }
}

/// Filter parsing error.
#[derive(Debug, Clone, PartialEq)]
pub struct FilterParseError {
    pub message: String,
    pub position: usize,
}

impl fmt::Display for FilterParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at position {}", self.message, self.position)
    }
}

impl std::error::Error for FilterParseError {}

/// Parse a SCIM filter expression.
///
/// # Errors
///
/// Returns an error if:
/// - The filter exceeds [`MAX_FILTER_LENGTH`] bytes
/// - The filter exceeds [`MAX_FILTER_DEPTH`] nesting levels
/// - The filter has invalid syntax
///
/// # Examples
///
/// ```
/// use gateway::scim::filter::parse_filter;
///
/// let filter = parse_filter("userName eq \"john\"").unwrap();
/// let filter = parse_filter("active eq true and emails pr").unwrap();
/// ```
pub fn parse_filter(input: &str) -> Result<Filter, FilterParseError> {
    // Check length limit before parsing
    if input.len() > MAX_FILTER_LENGTH {
        return Err(FilterParseError {
            message: format!(
                "Filter exceeds maximum length ({} bytes, max {})",
                input.len(),
                MAX_FILTER_LENGTH
            ),
            position: 0,
        });
    }

    let mut parser = Parser::new(input);
    let filter = parser.parse_filter()?;

    // Ensure we consumed all input
    parser.skip_whitespace();
    if parser.position < parser.input.len() {
        return Err(FilterParseError {
            message: format!("Unexpected input: '{}'", &parser.input[parser.position..]),
            position: parser.position,
        });
    }

    Ok(filter)
}

// =============================================================================
// Parser Implementation
// =============================================================================

struct Parser<'a> {
    input: &'a str,
    position: usize,
    depth: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input,
            position: 0,
            depth: 0,
        }
    }

    /// Enter a nested scope (parentheses, brackets, etc.).
    /// Returns an error if the maximum nesting depth is exceeded.
    fn enter_scope(&mut self) -> Result<(), FilterParseError> {
        self.depth += 1;
        if self.depth > MAX_FILTER_DEPTH {
            return Err(FilterParseError {
                message: format!(
                    "Filter exceeds maximum nesting depth ({})",
                    MAX_FILTER_DEPTH
                ),
                position: self.position,
            });
        }
        Ok(())
    }

    /// Exit a nested scope.
    fn exit_scope(&mut self) {
        self.depth = self.depth.saturating_sub(1);
    }

    fn parse_filter(&mut self) -> Result<Filter, FilterParseError> {
        self.parse_or_expr()
    }

    // logExpr = andExpr { "or" andExpr }
    fn parse_or_expr(&mut self) -> Result<Filter, FilterParseError> {
        let mut left = self.parse_and_expr()?;

        while self.try_keyword("or") {
            let right = self.parse_and_expr()?;
            left = Filter::Or(Box::new(left), Box::new(right));
        }

        Ok(left)
    }

    // andExpr = notExpr { "and" notExpr }
    fn parse_and_expr(&mut self) -> Result<Filter, FilterParseError> {
        let mut left = self.parse_not_expr()?;

        while self.try_keyword("and") {
            let right = self.parse_not_expr()?;
            left = Filter::And(Box::new(left), Box::new(right));
        }

        Ok(left)
    }

    // notExpr = "not" "(" filter ")" | "(" filter ")" | attrExpr
    fn parse_not_expr(&mut self) -> Result<Filter, FilterParseError> {
        self.skip_whitespace();

        // Check for "not" keyword
        if self.try_keyword("not") {
            self.skip_whitespace();
            if !self.try_char('(') {
                return Err(FilterParseError {
                    message: "Expected '(' after 'not'".to_string(),
                    position: self.position,
                });
            }
            self.enter_scope()?;
            let inner = self.parse_filter()?;
            self.exit_scope();
            self.skip_whitespace();
            if !self.try_char(')') {
                return Err(FilterParseError {
                    message: "Expected ')' to close 'not' expression".to_string(),
                    position: self.position,
                });
            }
            return Ok(Filter::Not(Box::new(inner)));
        }

        // Check for grouped expression
        if self.try_char('(') {
            self.enter_scope()?;
            let inner = self.parse_filter()?;
            self.exit_scope();
            self.skip_whitespace();
            if !self.try_char(')') {
                return Err(FilterParseError {
                    message: "Expected ')' to close grouped expression".to_string(),
                    position: self.position,
                });
            }
            return Ok(inner);
        }

        // Otherwise, parse attribute expression
        self.parse_attr_expr()
    }

    // attrExpr = attrPath "pr" | attrPath compareOp compValue
    fn parse_attr_expr(&mut self) -> Result<Filter, FilterParseError> {
        let attr = self.parse_attr_path()?;

        self.skip_whitespace();

        // Check for presence operator
        if self.try_keyword("pr") {
            return Ok(Filter::Present { attr });
        }

        // Parse comparison operator
        let op = self.parse_compare_op()?;

        self.skip_whitespace();

        // Parse comparison value
        let value = self.parse_value()?;

        Ok(Filter::Compare { attr, op, value })
    }

    // attrPath = ATTRNAME ["[" valFilter "]"] ["." ATTRNAME]
    fn parse_attr_path(&mut self) -> Result<AttrPath, FilterParseError> {
        self.skip_whitespace();

        let attr = self.parse_attr_name()?;

        // Check for value filter
        let value_filter = if self.try_char('[') {
            self.enter_scope()?;
            let filter = self.parse_value_filter()?;
            self.exit_scope();
            self.skip_whitespace();
            if !self.try_char(']') {
                return Err(FilterParseError {
                    message: "Expected ']' to close value filter".to_string(),
                    position: self.position,
                });
            }
            Some(Box::new(filter))
        } else {
            None
        };

        // Check for sub-attribute
        let sub_attr = if self.try_char('.') {
            Some(self.parse_attr_name()?)
        } else {
            None
        };

        Ok(AttrPath {
            attr,
            sub_attr,
            value_filter,
        })
    }

    // valFilter = attrPath compareOp compValue (simplified - no logical ops in value filter)
    fn parse_value_filter(&mut self) -> Result<Filter, FilterParseError> {
        let attr = self.parse_attr_path()?;

        self.skip_whitespace();

        // Check for presence operator
        if self.try_keyword("pr") {
            return Ok(Filter::Present { attr });
        }

        let op = self.parse_compare_op()?;

        self.skip_whitespace();

        let value = self.parse_value()?;

        Ok(Filter::Compare { attr, op, value })
    }

    fn parse_attr_name(&mut self) -> Result<String, FilterParseError> {
        self.skip_whitespace();

        let start = self.position;

        // Attribute names must start with a letter
        if !self.peek().is_some_and(|c| c.is_ascii_alphabetic()) {
            return Err(FilterParseError {
                message: "Expected attribute name".to_string(),
                position: self.position,
            });
        }

        // Consume alphanumeric characters, underscores, and hyphens
        while self
            .peek()
            .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            self.advance();
        }

        if self.position == start {
            return Err(FilterParseError {
                message: "Expected attribute name".to_string(),
                position: self.position,
            });
        }

        Ok(self.input[start..self.position].to_string())
    }

    fn parse_compare_op(&mut self) -> Result<CompareOp, FilterParseError> {
        self.skip_whitespace();

        let start = self.position;

        // Read operator (2 characters)
        while self.peek().is_some_and(|c| c.is_ascii_alphabetic()) && self.position - start < 2 {
            self.advance();
        }

        let op_str = &self.input[start..self.position];

        CompareOp::from_str(op_str).ok_or_else(|| FilterParseError {
            message: format!("Unknown operator: '{}'", op_str),
            position: start,
        })
    }

    fn parse_value(&mut self) -> Result<FilterValue, FilterParseError> {
        self.skip_whitespace();

        // String value
        if self.peek() == Some('"') {
            return self.parse_string_value();
        }

        // Boolean or null
        if self.try_keyword("true") {
            return Ok(FilterValue::Bool(true));
        }
        if self.try_keyword("false") {
            return Ok(FilterValue::Bool(false));
        }
        if self.try_keyword("null") {
            return Ok(FilterValue::Null);
        }

        // Number
        if self
            .peek()
            .is_some_and(|c| c.is_ascii_digit() || c == '-' || c == '+')
        {
            return self.parse_number_value();
        }

        Err(FilterParseError {
            message: "Expected value (string, boolean, number, or null)".to_string(),
            position: self.position,
        })
    }

    fn parse_string_value(&mut self) -> Result<FilterValue, FilterParseError> {
        if !self.try_char('"') {
            return Err(FilterParseError {
                message: "Expected '\"' to start string".to_string(),
                position: self.position,
            });
        }

        let mut value = String::new();

        loop {
            match self.peek() {
                None => {
                    return Err(FilterParseError {
                        message: "Unterminated string".to_string(),
                        position: self.position,
                    });
                }
                Some('"') => {
                    self.advance();
                    break;
                }
                Some('\\') => {
                    self.advance();
                    match self.peek() {
                        Some('"') => {
                            value.push('"');
                            self.advance();
                        }
                        Some('\\') => {
                            value.push('\\');
                            self.advance();
                        }
                        Some('n') => {
                            value.push('\n');
                            self.advance();
                        }
                        Some('t') => {
                            value.push('\t');
                            self.advance();
                        }
                        Some('r') => {
                            value.push('\r');
                            self.advance();
                        }
                        _ => {
                            return Err(FilterParseError {
                                message: "Invalid escape sequence".to_string(),
                                position: self.position,
                            });
                        }
                    }
                }
                Some(c) => {
                    value.push(c);
                    self.advance();
                }
            }
        }

        Ok(FilterValue::String(value))
    }

    fn parse_number_value(&mut self) -> Result<FilterValue, FilterParseError> {
        let start = self.position;

        // Optional sign
        if self.peek() == Some('-') || self.peek() == Some('+') {
            self.advance();
        }

        // Integer part
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.advance();
        }

        // Decimal part
        if self.peek() == Some('.') {
            self.advance();
            while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                self.advance();
            }
        }

        // Exponent part
        if self.peek().is_some_and(|c| c == 'e' || c == 'E') {
            self.advance();
            if self.peek() == Some('-') || self.peek() == Some('+') {
                self.advance();
            }
            while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                self.advance();
            }
        }

        let num_str = &self.input[start..self.position];
        num_str
            .parse::<f64>()
            .map(FilterValue::Number)
            .map_err(|_| FilterParseError {
                message: format!("Invalid number: '{}'", num_str),
                position: start,
            })
    }

    // Helper methods

    fn peek(&self) -> Option<char> {
        self.input[self.position..].chars().next()
    }

    fn advance(&mut self) {
        if let Some(c) = self.peek() {
            self.position += c.len_utf8();
        }
    }

    fn skip_whitespace(&mut self) {
        while self.peek().is_some_and(|c| c.is_whitespace()) {
            self.advance();
        }
    }

    fn try_char(&mut self, expected: char) -> bool {
        if self.peek() == Some(expected) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn try_keyword(&mut self, keyword: &str) -> bool {
        self.skip_whitespace();

        let remaining = &self.input[self.position..];

        // Case-insensitive comparison
        if remaining.len() >= keyword.len()
            && remaining[..keyword.len()].eq_ignore_ascii_case(keyword)
        {
            // Make sure keyword is not part of a larger identifier
            let after_keyword = remaining[keyword.len()..].chars().next();
            if after_keyword.is_none_or(|c| !c.is_ascii_alphanumeric()) {
                self.position += keyword.len();
                return true;
            }
        }

        false
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_equality() {
        let filter = parse_filter("userName eq \"john\"").unwrap();
        match filter {
            Filter::Compare { attr, op, value } => {
                assert_eq!(attr.attr, "userName");
                assert_eq!(op, CompareOp::Eq);
                assert_eq!(value, FilterValue::String("john".to_string()));
            }
            _ => panic!("Expected Compare filter"),
        }
    }

    #[test]
    fn test_boolean_value() {
        let filter = parse_filter("active eq true").unwrap();
        match filter {
            Filter::Compare { attr, op, value } => {
                assert_eq!(attr.attr, "active");
                assert_eq!(op, CompareOp::Eq);
                assert_eq!(value, FilterValue::Bool(true));
            }
            _ => panic!("Expected Compare filter"),
        }

        let filter = parse_filter("active eq false").unwrap();
        match filter {
            Filter::Compare { value, .. } => {
                assert_eq!(value, FilterValue::Bool(false));
            }
            _ => panic!("Expected Compare filter"),
        }
    }

    #[test]
    fn test_null_value() {
        let filter = parse_filter("manager eq null").unwrap();
        match filter {
            Filter::Compare { value, .. } => {
                assert_eq!(value, FilterValue::Null);
            }
            _ => panic!("Expected Compare filter"),
        }
    }

    #[test]
    fn test_number_value() {
        let filter = parse_filter("age gt 21").unwrap();
        match filter {
            Filter::Compare { value, .. } => {
                assert_eq!(value, FilterValue::Number(21.0));
            }
            _ => panic!("Expected Compare filter"),
        }

        let filter = parse_filter("score le -5.5").unwrap();
        match filter {
            Filter::Compare { value, .. } => {
                assert_eq!(value, FilterValue::Number(-5.5));
            }
            _ => panic!("Expected Compare filter"),
        }
    }

    #[test]
    fn test_presence_operator() {
        let filter = parse_filter("name pr").unwrap();
        match filter {
            Filter::Present { attr } => {
                assert_eq!(attr.attr, "name");
            }
            _ => panic!("Expected Present filter"),
        }
    }

    #[test]
    fn test_nested_attribute() {
        let filter = parse_filter("name.familyName eq \"Doe\"").unwrap();
        match filter {
            Filter::Compare { attr, .. } => {
                assert_eq!(attr.attr, "name");
                assert_eq!(attr.sub_attr, Some("familyName".to_string()));
            }
            _ => panic!("Expected Compare filter"),
        }
    }

    #[test]
    fn test_value_filter() {
        let filter =
            parse_filter("emails[type eq \"work\"].value eq \"john@example.com\"").unwrap();
        match filter {
            Filter::Compare { attr, .. } => {
                assert_eq!(attr.attr, "emails");
                assert!(attr.value_filter.is_some());
                assert_eq!(attr.sub_attr, Some("value".to_string()));

                // Check the value filter
                if let Some(ref vf) = attr.value_filter {
                    match vf.as_ref() {
                        Filter::Compare { attr, op, value } => {
                            assert_eq!(attr.attr, "type");
                            assert_eq!(*op, CompareOp::Eq);
                            assert_eq!(*value, FilterValue::String("work".to_string()));
                        }
                        _ => panic!("Expected Compare in value filter"),
                    }
                }
            }
            _ => panic!("Expected Compare filter"),
        }
    }

    #[test]
    fn test_logical_and() {
        let filter = parse_filter("active eq true and userName sw \"j\"").unwrap();
        match filter {
            Filter::And(left, right) => {
                match left.as_ref() {
                    Filter::Compare { attr, .. } => assert_eq!(attr.attr, "active"),
                    _ => panic!("Expected Compare filter"),
                }
                match right.as_ref() {
                    Filter::Compare { attr, op, .. } => {
                        assert_eq!(attr.attr, "userName");
                        assert_eq!(*op, CompareOp::Sw);
                    }
                    _ => panic!("Expected Compare filter"),
                }
            }
            _ => panic!("Expected And filter"),
        }
    }

    #[test]
    fn test_logical_or() {
        let filter = parse_filter("userName eq \"a\" or userName eq \"b\"").unwrap();
        match filter {
            Filter::Or(_, _) => {}
            _ => panic!("Expected Or filter"),
        }
    }

    #[test]
    fn test_not_operator() {
        let filter = parse_filter("not (active eq false)").unwrap();
        match filter {
            Filter::Not(inner) => match inner.as_ref() {
                Filter::Compare { attr, .. } => {
                    assert_eq!(attr.attr, "active");
                }
                _ => panic!("Expected Compare inside Not"),
            },
            _ => panic!("Expected Not filter"),
        }
    }

    #[test]
    fn test_grouped_expression() {
        let filter = parse_filter("(a eq \"1\" or b eq \"2\") and c eq \"3\"").unwrap();
        match filter {
            Filter::And(left, right) => {
                match left.as_ref() {
                    Filter::Or(_, _) => {}
                    _ => panic!("Expected Or inside And"),
                }
                match right.as_ref() {
                    Filter::Compare { attr, .. } => {
                        assert_eq!(attr.attr, "c");
                    }
                    _ => panic!("Expected Compare"),
                }
            }
            _ => panic!("Expected And filter"),
        }
    }

    #[test]
    fn test_case_insensitive_operators() {
        // Operators should be case-insensitive
        let filter = parse_filter("userName EQ \"john\"").unwrap();
        match filter {
            Filter::Compare { op, .. } => {
                assert_eq!(op, CompareOp::Eq);
            }
            _ => panic!("Expected Compare filter"),
        }

        let filter = parse_filter("active EQ TRUE AND userName SW \"j\"").unwrap();
        match filter {
            Filter::And(_, _) => {}
            _ => panic!("Expected And filter"),
        }
    }

    #[test]
    fn test_all_comparison_operators() {
        let ops = [
            ("eq", CompareOp::Eq),
            ("ne", CompareOp::Ne),
            ("co", CompareOp::Co),
            ("sw", CompareOp::Sw),
            ("ew", CompareOp::Ew),
            ("gt", CompareOp::Gt),
            ("ge", CompareOp::Ge),
            ("lt", CompareOp::Lt),
            ("le", CompareOp::Le),
        ];

        for (op_str, expected_op) in ops {
            let filter_str = format!("attr {} \"value\"", op_str);
            let filter = parse_filter(&filter_str).unwrap();
            match filter {
                Filter::Compare { op, .. } => {
                    assert_eq!(op, expected_op, "Failed for operator: {}", op_str);
                }
                _ => panic!("Expected Compare filter for operator: {}", op_str),
            }
        }
    }

    #[test]
    fn test_escaped_string() {
        let filter = parse_filter(r#"name eq "John \"Doe\"""#).unwrap();
        match filter {
            Filter::Compare { value, .. } => {
                assert_eq!(value, FilterValue::String("John \"Doe\"".to_string()));
            }
            _ => panic!("Expected Compare filter"),
        }
    }

    #[test]
    fn test_complex_filter() {
        // Real-world filter from Okta
        let filter =
            parse_filter(r#"userName eq "john.doe@example.com" and active eq true"#).unwrap();

        match filter {
            Filter::And(_, _) => {}
            _ => panic!("Expected And filter"),
        }
    }

    #[test]
    fn test_error_invalid_operator() {
        let result = parse_filter("userName xx \"john\"");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("Unknown operator"));
    }

    #[test]
    fn test_error_unterminated_string() {
        let result = parse_filter("userName eq \"john");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("Unterminated string"));
    }

    #[test]
    fn test_error_missing_value() {
        let result = parse_filter("userName eq");
        assert!(result.is_err());
    }

    #[test]
    fn test_error_unexpected_input() {
        let result = parse_filter("userName eq \"john\" extra");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("Unexpected input"));
    }

    #[test]
    fn test_display() {
        let filter = parse_filter("userName eq \"john\"").unwrap();
        let display = format!("{}", filter);
        assert_eq!(display, "userName eq \"john\"");

        let filter = parse_filter("active eq true and name pr").unwrap();
        let display = format!("{}", filter);
        assert!(display.contains("and"));
    }

    #[test]
    fn test_whitespace_handling() {
        // Various whitespace configurations
        let filter = parse_filter("  userName   eq   \"john\"  ").unwrap();
        match filter {
            Filter::Compare { attr, .. } => {
                assert_eq!(attr.attr, "userName");
            }
            _ => panic!("Expected Compare filter"),
        }
    }

    #[test]
    fn test_value_filter_with_presence() {
        // Test that emails with a 'type' attribute present are checked
        // The filter is: presence check on emails that have a 'type' attribute
        let filter = parse_filter("emails[type pr] pr").unwrap();
        match filter {
            Filter::Present { attr } => {
                assert_eq!(attr.attr, "emails");
                assert!(attr.value_filter.is_some());
                // Check that value filter is a presence check on 'type'
                if let Some(ref vf) = attr.value_filter {
                    match vf.as_ref() {
                        Filter::Present { attr: inner } => {
                            assert_eq!(inner.attr, "type");
                        }
                        _ => panic!("Expected Present filter inside value filter"),
                    }
                }
            }
            _ => panic!("Expected Present filter"),
        }
    }

    // =========================================================================
    // Complexity Limit Tests
    // =========================================================================

    #[test]
    fn test_filter_at_max_length() {
        // Create a filter at exactly MAX_FILTER_LENGTH
        // Use a simple pattern that's easy to extend: "a eq \"...\""
        let prefix = "a eq \"";
        let suffix = "\"";
        let padding_needed = MAX_FILTER_LENGTH - prefix.len() - suffix.len();
        let long_value = "x".repeat(padding_needed);
        let filter_str = format!("{}{}{}", prefix, long_value, suffix);

        assert_eq!(filter_str.len(), MAX_FILTER_LENGTH);
        let result = parse_filter(&filter_str);
        assert!(
            result.is_ok(),
            "Filter at max length should parse successfully"
        );
    }

    #[test]
    fn test_filter_exceeds_max_length() {
        // Create a filter that exceeds MAX_FILTER_LENGTH by 1 byte
        let prefix = "a eq \"";
        let suffix = "\"";
        let padding_needed = MAX_FILTER_LENGTH - prefix.len() - suffix.len() + 1;
        let long_value = "x".repeat(padding_needed);
        let filter_str = format!("{}{}{}", prefix, long_value, suffix);

        assert_eq!(filter_str.len(), MAX_FILTER_LENGTH + 1);
        let result = parse_filter(&filter_str);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.message.contains("maximum length"),
            "Error should mention maximum length: {}",
            err.message
        );
    }

    #[test]
    fn test_filter_at_max_depth_with_not() {
        // Create a filter at exactly MAX_FILTER_DEPTH using nested not()
        // not (not (not (...))) with MAX_FILTER_DEPTH levels
        let mut filter_str = "a pr".to_string();
        for _ in 0..MAX_FILTER_DEPTH {
            filter_str = format!("not ({})", filter_str);
        }

        let result = parse_filter(&filter_str);
        assert!(
            result.is_ok(),
            "Filter at max depth should parse successfully: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_filter_exceeds_max_depth_with_not() {
        // Create a filter that exceeds MAX_FILTER_DEPTH by 1 level using nested not()
        let mut filter_str = "a pr".to_string();
        for _ in 0..=MAX_FILTER_DEPTH {
            filter_str = format!("not ({})", filter_str);
        }

        let result = parse_filter(&filter_str);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.message.contains("maximum nesting depth"),
            "Error should mention maximum nesting depth: {}",
            err.message
        );
    }

    #[test]
    fn test_filter_at_max_depth_with_groups() {
        // Create a filter at exactly MAX_FILTER_DEPTH using nested groups
        // (((...))) with MAX_FILTER_DEPTH levels
        let mut filter_str = "a pr".to_string();
        for _ in 0..MAX_FILTER_DEPTH {
            filter_str = format!("({})", filter_str);
        }

        let result = parse_filter(&filter_str);
        assert!(
            result.is_ok(),
            "Filter at max depth should parse successfully: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_filter_exceeds_max_depth_with_groups() {
        // Create a filter that exceeds MAX_FILTER_DEPTH by 1 level using nested groups
        let mut filter_str = "a pr".to_string();
        for _ in 0..=MAX_FILTER_DEPTH {
            filter_str = format!("({})", filter_str);
        }

        let result = parse_filter(&filter_str);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.message.contains("maximum nesting depth"),
            "Error should mention maximum nesting depth: {}",
            err.message
        );
    }

    #[test]
    fn test_filter_depth_with_value_filter() {
        // Test that value filters contribute to depth
        // emails[type eq "work"].value eq "a" - 1 level of nesting from []
        let filter =
            parse_filter("emails[type eq \"work\"].value eq \"test@example.com\"").unwrap();
        match filter {
            Filter::Compare { attr, .. } => {
                assert!(attr.value_filter.is_some());
            }
            _ => panic!("Expected Compare filter with value filter"),
        }
    }

    #[test]
    fn test_filter_exceeds_depth_via_value_filter() {
        // Create a filter at MAX_FILTER_DEPTH with groups, then add one more via value filter
        // (((...(emails[type eq "x"].value eq "y")...))) - groups bring us to max, value filter pushes over
        let mut filter_str = "emails[type eq \"x\"].value eq \"y\"".to_string(); // 1 level from []
        for _ in 0..MAX_FILTER_DEPTH {
            filter_str = format!("({})", filter_str);
        }

        let result = parse_filter(&filter_str);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.message.contains("maximum nesting depth"),
            "Error should mention maximum nesting depth: {}",
            err.message
        );
    }

    #[test]
    fn test_filter_mixed_nesting_types() {
        // Test that depth is tracked correctly across different nesting types
        // not (emails[type eq "work"].value eq "x") - should be 2 levels of nesting
        let filter = parse_filter("not (emails[type eq \"work\"].value eq \"x\")").unwrap();
        match filter {
            Filter::Not(_) => {}
            _ => panic!("Expected Not filter"),
        }
    }

    #[test]
    fn test_zero_length_filter() {
        let result = parse_filter("");
        assert!(result.is_err());
    }

    #[test]
    fn test_whitespace_only_filter() {
        let result = parse_filter("   ");
        assert!(result.is_err());
    }
}
