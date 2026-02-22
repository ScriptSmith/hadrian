//! Built-in regex-based PII detection provider for guardrails.
//!
//! This provider uses well-tested regex patterns to detect common PII types
//! without requiring external API calls.
//!
//! # Supported PII Types
//!
//! - Email addresses
//! - Phone numbers (US and international formats)
//! - Social Security Numbers (SSN)
//! - Credit card numbers (with Luhn validation)
//! - IP addresses (IPv4 and IPv6)
//! - Dates of birth
//!
//! # Example Configuration
//!
//! ```toml
//! [features.guardrails.input.provider]
//! type = "pii_regex"
//! # Enable specific PII types (default: all enabled)
//! email = true
//! phone = true
//! ssn = true
//! credit_card = true
//! ip_address = true
//! date_of_birth = true
//! ```

use std::time::Instant;

use async_trait::async_trait;
use regex::Regex;
use tracing::instrument;

use super::{
    Category, GuardrailsError, GuardrailsProvider, GuardrailsRequest, GuardrailsResponse,
    GuardrailsResult, Severity, Violation,
};

/// Configuration for which PII types to detect.
#[derive(Debug, Clone)]
pub struct PiiRegexConfig {
    pub email: bool,
    pub phone: bool,
    pub ssn: bool,
    pub credit_card: bool,
    pub ip_address: bool,
    pub date_of_birth: bool,
}

impl Default for PiiRegexConfig {
    fn default() -> Self {
        Self {
            email: true,
            phone: true,
            ssn: true,
            credit_card: true,
            ip_address: true,
            date_of_birth: true,
        }
    }
}

/// A compiled PII pattern with metadata.
#[derive(Debug)]
struct PiiPattern {
    regex: Regex,
    category: Category,
    message: &'static str,
    /// Optional validation function for additional checks (e.g., Luhn for credit cards).
    validator: Option<fn(&str) -> bool>,
}

/// Built-in regex-based PII detection provider.
///
/// Detects common PII types using pre-compiled regex patterns.
/// Patterns are validated at startup for optimal runtime performance.
pub struct PiiRegexProvider {
    patterns: Vec<PiiPattern>,
}

impl PiiRegexProvider {
    /// Creates a new PII regex provider with the specified configuration.
    pub fn new(config: PiiRegexConfig) -> GuardrailsResult<Self> {
        let mut patterns = Vec::new();

        if config.email {
            patterns.push(Self::email_pattern()?);
        }
        if config.phone {
            patterns.extend(Self::phone_patterns()?);
        }
        if config.ssn {
            patterns.push(Self::ssn_pattern()?);
        }
        if config.credit_card {
            patterns.push(Self::credit_card_pattern()?);
        }
        if config.ip_address {
            patterns.extend(Self::ip_patterns()?);
        }
        if config.date_of_birth {
            patterns.push(Self::dob_pattern()?);
        }

        Ok(Self { patterns })
    }

    /// Creates a provider with all PII types enabled.
    #[allow(dead_code)] // Used in tests and as a public API convenience
    pub fn all() -> GuardrailsResult<Self> {
        Self::new(PiiRegexConfig::default())
    }

    fn email_pattern() -> GuardrailsResult<PiiPattern> {
        // RFC 5322 simplified - covers most real-world email addresses
        let regex = Regex::new(
            r"(?i)[a-z0-9._%+-]+@[a-z0-9](?:[a-z0-9-]*[a-z0-9])?(?:\.[a-z0-9](?:[a-z0-9-]*[a-z0-9])?)+",
        )
        .map_err(|e| GuardrailsError::config_error(format!("Invalid email regex: {}", e)))?;

        Ok(PiiPattern {
            regex,
            category: Category::PiiEmail,
            message: "Email address detected",
            validator: None,
        })
    }

    fn phone_patterns() -> GuardrailsResult<Vec<PiiPattern>> {
        let patterns = vec![
            // US phone numbers: (555) 123-4567, 555-123-4567, 555.123.4567, 5551234567
            // Area code starts with 2-9 (NANP), but we're permissive on exchange/subscriber
            (
                r"\b(?:\+?1[-.\s]?)?\(?[2-9]\d{2}\)?[-.\s]?\d{3}[-.\s]?\d{4}\b",
                "US phone number detected",
            ),
            // International format: +44 20 7946 0958, +33 1 23 45 67 89
            (
                r"\+[1-9]\d{0,2}[-.\s]?\d{1,4}[-.\s]?\d{1,4}[-.\s]?\d{1,9}",
                "International phone number detected",
            ),
        ];

        patterns
            .into_iter()
            .map(|(pattern, message)| {
                let regex = Regex::new(pattern).map_err(|e| {
                    GuardrailsError::config_error(format!("Invalid phone regex: {}", e))
                })?;
                Ok(PiiPattern {
                    regex,
                    category: Category::PiiPhone,
                    message,
                    validator: None,
                })
            })
            .collect()
    }

    fn ssn_pattern() -> GuardrailsResult<PiiPattern> {
        // SSN format: 123-45-6789 or 123 45 6789 or 123456789
        // Validation of invalid ranges (000, 666, 900-999, etc.) is done in validate_ssn
        let regex = Regex::new(r"\b\d{3}[-\s]?\d{2}[-\s]?\d{4}\b")
            .map_err(|e| GuardrailsError::config_error(format!("Invalid SSN regex: {}", e)))?;

        Ok(PiiPattern {
            regex,
            category: Category::PiiSsn,
            message: "Social Security Number detected",
            validator: Some(validate_ssn),
        })
    }

    fn credit_card_pattern() -> GuardrailsResult<PiiPattern> {
        // Major card formats with optional separators
        // Visa: 4xxx, Mastercard: 51-55/2221-2720, Amex: 34/37, Discover: 6011/65/644-649
        let regex = Regex::new(
            r"\b(?:4\d{3}|5[1-5]\d{2}|6(?:011|5\d{2}|4[4-9]\d)|3[47]\d{2})[-\s]?\d{4}[-\s]?\d{4}[-\s]?\d{1,4}\b",
        )
        .map_err(|e| GuardrailsError::config_error(format!("Invalid credit card regex: {}", e)))?;

        Ok(PiiPattern {
            regex,
            category: Category::PiiCreditCard,
            message: "Credit card number detected",
            validator: Some(validate_luhn),
        })
    }

    fn ip_patterns() -> GuardrailsResult<Vec<PiiPattern>> {
        let patterns = vec![
            // IPv4
            (
                r"\b(?:(?:25[0-5]|2[0-4]\d|1\d{2}|[1-9]?\d)\.){3}(?:25[0-5]|2[0-4]\d|1\d{2}|[1-9]?\d)\b",
                "IPv4 address detected",
                Category::PiiOther,
            ),
            // IPv6 (simplified - covers most common formats)
            (
                r"(?i)\b(?:[0-9a-f]{1,4}:){7}[0-9a-f]{1,4}\b",
                "IPv6 address detected",
                Category::PiiOther,
            ),
        ];

        patterns
            .into_iter()
            .map(|(pattern, message, category)| {
                let regex = Regex::new(pattern).map_err(|e| {
                    GuardrailsError::config_error(format!("Invalid IP regex: {}", e))
                })?;
                Ok(PiiPattern {
                    regex,
                    category,
                    message,
                    validator: None,
                })
            })
            .collect()
    }

    fn dob_pattern() -> GuardrailsResult<PiiPattern> {
        // Common date formats that might be DOB:
        // MM/DD/YYYY, MM-DD-YYYY, YYYY-MM-DD, DD/MM/YYYY
        // Only matches dates that look like they could be birthdates (1900-2099)
        let regex = Regex::new(
            r"\b(?:(?:0?[1-9]|1[0-2])[-/](?:0?[1-9]|[12]\d|3[01])[-/](?:19|20)\d{2}|(?:19|20)\d{2}[-/](?:0?[1-9]|1[0-2])[-/](?:0?[1-9]|[12]\d|3[01]))\b",
        )
        .map_err(|e| GuardrailsError::config_error(format!("Invalid DOB regex: {}", e)))?;

        Ok(PiiPattern {
            regex,
            category: Category::PiiOther,
            message: "Potential date of birth detected",
            validator: None,
        })
    }

    /// Finds all PII matches in the content.
    fn find_matches<'a>(
        &'a self,
        content: &'a str,
    ) -> impl Iterator<Item = (&'a PiiPattern, regex::Match<'a>)> {
        self.patterns.iter().flat_map(move |pattern| {
            pattern.regex.find_iter(content).filter_map(move |m| {
                // Apply validator if present - skip matches that fail validation
                if let Some(validator) = pattern.validator
                    && !validator(m.as_str())
                {
                    return None;
                }
                Some((pattern, m))
            })
        })
    }
}

#[async_trait]
impl GuardrailsProvider for PiiRegexProvider {
    fn name(&self) -> &str {
        "pii_regex"
    }

    #[instrument(
        skip(self, request),
        fields(
            provider = "pii_regex",
            pattern_count = self.patterns.len(),
            text_length = request.text.len()
        )
    )]
    async fn evaluate(&self, request: &GuardrailsRequest) -> GuardrailsResult<GuardrailsResponse> {
        let start = Instant::now();

        let violations: Vec<Violation> = self
            .find_matches(&request.text)
            .map(|(pattern, m)| {
                Violation::new(pattern.category.clone(), Severity::High, 1.0)
                    .with_message(pattern.message)
                    .with_span(m.start(), m.end())
            })
            .collect();

        let latency_ms = start.elapsed().as_millis() as u64;

        tracing::debug!(
            violation_count = violations.len(),
            latency_ms = latency_ms,
            "PII regex evaluation complete"
        );

        Ok(GuardrailsResponse::with_violations(violations).with_latency(latency_ms))
    }

    fn supported_categories(&self) -> &[Category] {
        &[
            Category::PiiEmail,
            Category::PiiPhone,
            Category::PiiSsn,
            Category::PiiCreditCard,
            Category::PiiOther,
        ]
    }
}

/// Validates a credit card number using the Luhn algorithm.
fn validate_luhn(number: &str) -> bool {
    // Remove separators
    let digits: String = number.chars().filter(|c| c.is_ascii_digit()).collect();

    if digits.len() < 13 || digits.len() > 19 {
        return false;
    }

    let mut sum = 0;
    let mut double = false;

    for c in digits.chars().rev() {
        let mut digit = c.to_digit(10).unwrap_or(0);
        if double {
            digit *= 2;
            if digit > 9 {
                digit -= 9;
            }
        }
        sum += digit;
        double = !double;
    }

    sum % 10 == 0
}

/// Validates an SSN format (basic structural validation).
fn validate_ssn(ssn: &str) -> bool {
    // Remove separators
    let digits: String = ssn.chars().filter(|c| c.is_ascii_digit()).collect();

    if digits.len() != 9 {
        return false;
    }

    // Area number (first 3 digits) cannot be 000, 666, or 900-999
    let area: u32 = digits[0..3].parse().unwrap_or(0);
    if area == 0 || area == 666 || area >= 900 {
        return false;
    }

    // Group number (middle 2 digits) cannot be 00
    let group: u32 = digits[3..5].parse().unwrap_or(0);
    if group == 0 {
        return false;
    }

    // Serial number (last 4 digits) cannot be 0000
    let serial: u32 = digits[5..9].parse().unwrap_or(0);
    serial != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_email_detection() {
        let provider = PiiRegexProvider::new(PiiRegexConfig {
            email: true,
            ..Default::default()
        })
        .unwrap();

        let matches: Vec<_> = provider
            .find_matches("Contact me at john.doe@example.com for more info")
            .collect();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].1.as_str(), "john.doe@example.com");
        assert_eq!(matches[0].0.category, Category::PiiEmail);
    }

    #[test]
    fn test_email_variations() {
        let provider = PiiRegexProvider::new(PiiRegexConfig {
            email: true,
            phone: false,
            ssn: false,
            credit_card: false,
            ip_address: false,
            date_of_birth: false,
        })
        .unwrap();

        let test_cases = [
            ("simple@example.com", true),
            ("user.name+tag@example.co.uk", true),
            ("user@subdomain.example.com", true),
            ("invalid@", false),
            ("@invalid.com", false),
            ("no-at-sign.com", false),
        ];

        for (email, should_match) in test_cases {
            let matches: Vec<_> = provider.find_matches(email).collect();
            assert_eq!(
                !matches.is_empty(),
                should_match,
                "Email '{}' should_match={} but got {}",
                email,
                should_match,
                !matches.is_empty()
            );
        }
    }

    #[test]
    fn test_us_phone_detection() {
        let provider = PiiRegexProvider::new(PiiRegexConfig {
            phone: true,
            email: false,
            ssn: false,
            credit_card: false,
            ip_address: false,
            date_of_birth: false,
        })
        .unwrap();

        let test_cases = [
            "(555) 123-4567",
            "555-123-4567",
            "555.123.4567",
            "5551234567",
            "+1 555-123-4567",
        ];

        for phone in test_cases {
            let matches: Vec<_> = provider.find_matches(phone).collect();
            assert!(!matches.is_empty(), "Phone '{}' should be detected", phone);
            assert_eq!(matches[0].0.category, Category::PiiPhone);
        }
    }

    #[test]
    fn test_international_phone_detection() {
        let provider = PiiRegexProvider::new(PiiRegexConfig {
            phone: true,
            email: false,
            ssn: false,
            credit_card: false,
            ip_address: false,
            date_of_birth: false,
        })
        .unwrap();

        let test_cases = ["+44 20 7946 0958", "+33 1 23 45 67 89", "+1-555-123-4567"];

        for phone in test_cases {
            let matches: Vec<_> = provider.find_matches(phone).collect();
            assert!(
                !matches.is_empty(),
                "International phone '{}' should be detected",
                phone
            );
        }
    }

    #[test]
    fn test_ssn_detection() {
        let provider = PiiRegexProvider::new(PiiRegexConfig {
            ssn: true,
            email: false,
            phone: false,
            credit_card: false,
            ip_address: false,
            date_of_birth: false,
        })
        .unwrap();

        let valid_ssns = ["123-45-6789", "123 45 6789", "123456789"];

        for ssn in valid_ssns {
            let matches: Vec<_> = provider.find_matches(ssn).collect();
            assert!(!matches.is_empty(), "SSN '{}' should be detected", ssn);
            assert_eq!(matches[0].0.category, Category::PiiSsn);
        }
    }

    #[test]
    fn test_ssn_invalid_formats() {
        let provider = PiiRegexProvider::new(PiiRegexConfig {
            ssn: true,
            email: false,
            phone: false,
            credit_card: false,
            ip_address: false,
            date_of_birth: false,
        })
        .unwrap();

        let invalid_ssns = [
            "000-12-3456", // Area 000 invalid
            "666-12-3456", // Area 666 invalid
            "900-12-3456", // Area 900+ invalid
            "123-00-3456", // Group 00 invalid
            "123-45-0000", // Serial 0000 invalid
        ];

        for ssn in invalid_ssns {
            let matches: Vec<_> = provider.find_matches(ssn).collect();
            assert!(
                matches.is_empty(),
                "Invalid SSN '{}' should not be detected",
                ssn
            );
        }
    }

    #[test]
    fn test_credit_card_detection() {
        let provider = PiiRegexProvider::new(PiiRegexConfig {
            credit_card: true,
            email: false,
            phone: false,
            ssn: false,
            ip_address: false,
            date_of_birth: false,
        })
        .unwrap();

        // Valid test card numbers (pass Luhn check)
        let valid_cards = [
            "4111111111111111",    // Visa
            "4111-1111-1111-1111", // Visa with dashes
            "4111 1111 1111 1111", // Visa with spaces
            "5500000000000004",    // Mastercard
            "340000000000009",     // Amex
            "6011000000000004",    // Discover
        ];

        for card in valid_cards {
            let matches: Vec<_> = provider.find_matches(card).collect();
            assert!(
                !matches.is_empty(),
                "Valid card '{}' should be detected",
                card
            );
            assert_eq!(matches[0].0.category, Category::PiiCreditCard);
        }
    }

    #[test]
    fn test_credit_card_luhn_validation() {
        // Test Luhn algorithm directly
        assert!(validate_luhn("4111111111111111")); // Valid Visa test number
        assert!(validate_luhn("5500000000000004")); // Valid Mastercard test number
        assert!(!validate_luhn("4111111111111112")); // Invalid (wrong check digit)
        assert!(!validate_luhn("1234567890123456")); // Invalid (fails Luhn)
    }

    #[test]
    fn test_ipv4_detection() {
        let provider = PiiRegexProvider::new(PiiRegexConfig {
            ip_address: true,
            email: false,
            phone: false,
            ssn: false,
            credit_card: false,
            date_of_birth: false,
        })
        .unwrap();

        let valid_ips = ["192.168.1.1", "10.0.0.1", "255.255.255.255", "8.8.8.8"];

        for ip in valid_ips {
            let matches: Vec<_> = provider.find_matches(ip).collect();
            assert!(!matches.is_empty(), "IPv4 '{}' should be detected", ip);
        }
    }

    #[test]
    fn test_ipv6_detection() {
        let provider = PiiRegexProvider::new(PiiRegexConfig {
            ip_address: true,
            email: false,
            phone: false,
            ssn: false,
            credit_card: false,
            date_of_birth: false,
        })
        .unwrap();

        let matches: Vec<_> = provider
            .find_matches("2001:0db8:85a3:0000:0000:8a2e:0370:7334")
            .collect();
        assert!(!matches.is_empty(), "IPv6 should be detected");
    }

    #[test]
    fn test_dob_detection() {
        let provider = PiiRegexProvider::new(PiiRegexConfig {
            date_of_birth: true,
            email: false,
            phone: false,
            ssn: false,
            credit_card: false,
            ip_address: false,
        })
        .unwrap();

        let dates = ["01/15/1990", "12-25-1985", "1990-01-15", "2000-12-31"];

        for date in dates {
            let matches: Vec<_> = provider.find_matches(date).collect();
            assert!(!matches.is_empty(), "Date '{}' should be detected", date);
        }
    }

    #[tokio::test]
    async fn test_evaluate_returns_violations() {
        let provider = PiiRegexProvider::all().unwrap();

        let request =
            GuardrailsRequest::user_input("Contact john@example.com or call 555-123-4567");
        let response = provider.evaluate(&request).await.unwrap();

        assert!(!response.passed);
        assert!(response.violations.len() >= 2); // At least email and phone

        let categories: Vec<_> = response.violations.iter().map(|v| &v.category).collect();
        assert!(categories.contains(&&Category::PiiEmail));
        assert!(categories.contains(&&Category::PiiPhone));
    }

    #[tokio::test]
    async fn test_evaluate_no_violations() {
        let provider = PiiRegexProvider::all().unwrap();

        let request = GuardrailsRequest::user_input("Hello, how are you today?");
        let response = provider.evaluate(&request).await.unwrap();

        assert!(response.passed);
        assert!(response.violations.is_empty());
    }

    #[test]
    fn test_provider_name() {
        let provider = PiiRegexProvider::all().unwrap();
        assert_eq!(provider.name(), "pii_regex");
    }

    #[test]
    fn test_selective_detection() {
        // Only enable email detection
        let provider = PiiRegexProvider::new(PiiRegexConfig {
            email: true,
            phone: false,
            ssn: false,
            credit_card: false,
            ip_address: false,
            date_of_birth: false,
        })
        .unwrap();

        let text = "Email: test@example.com, Phone: 555-123-4567, SSN: 123-45-6789";
        let matches: Vec<_> = provider.find_matches(text).collect();

        // Should only find email
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].0.category, Category::PiiEmail);
    }

    #[test]
    fn test_span_accuracy() {
        let provider = PiiRegexProvider::new(PiiRegexConfig {
            email: true,
            phone: false,
            ssn: false,
            credit_card: false,
            ip_address: false,
            date_of_birth: false,
        })
        .unwrap();

        let text = "Contact: user@test.com here";
        let matches: Vec<_> = provider.find_matches(text).collect();

        assert_eq!(matches.len(), 1);
        let (_, m) = &matches[0];
        assert_eq!(&text[m.start()..m.end()], "user@test.com");
    }

    #[test]
    fn test_multiple_pii_in_same_text() {
        let provider = PiiRegexProvider::all().unwrap();

        let text = "User info: email john@test.com, phone 555-123-4567, SSN 123-45-6789";
        let matches: Vec<_> = provider.find_matches(text).collect();

        assert!(matches.len() >= 3, "Should find at least 3 PII items");
    }
}
