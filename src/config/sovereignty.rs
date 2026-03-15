//! Sovereignty and compliance metadata for providers and models.
//!
//! Organizations using Hadrian as an AI gateway need to understand and control
//! where their data is processed, by whom, and under what legal/regulatory
//! frameworks. This module provides structured metadata at the provider level
//! (where data goes) with model-level overrides.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Definition of a custom sovereignty metadata field.
///
/// Custom fields are defined globally in `[sovereignty.custom_fields]` and
/// can be set per-provider or per-model via `sovereignty.custom.<key>`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CustomSovereigntyFieldDef {
    /// Unique key used in config and API responses.
    pub key: String,
    /// Human-readable title for display in the UI.
    pub title: String,
    /// Optional description shown as help text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Top-level sovereignty configuration.
///
/// Defines custom metadata fields that providers and models can set values for.
/// ```toml
/// [[sovereignty.custom_fields]]
/// key = "data_residency"
/// title = "Data Residency"
/// description = "Where customer data is physically stored"
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct SovereigntyConfig {
    /// Custom sovereignty field definitions available to all providers.
    #[serde(default)]
    pub custom_fields: Vec<CustomSovereigntyFieldDef>,
}

/// Sovereignty and compliance metadata for providers and models.
///
/// When set at the provider level, these values apply to all models
/// from that provider. Per-model overrides take precedence.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct SovereigntyMetadata {
    /// ISO 3166-1 alpha-2 country code of the provider's headquarters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hq_country: Option<String>,

    /// ISO 3166-1 alpha-2 country codes where model inference runs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inference_countries: Vec<String>,

    /// Compliance certifications and regulatory frameworks.
    /// Well-known values: "gdpr", "hipaa", "soc2", "soc2-type2", "iso27001",
    /// "fedramp", "fedramp-high", "pci-dss", "c5", "ismap", "hipaa-baa", "ccpa", "dpa"
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub certifications: Vec<String>,

    /// Whether this provider/model runs on infrastructure operated by the org.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_prem: Option<bool>,

    /// Whether the provider trains on customer data.
    /// None = unknown, true = yes, false = no (or opt-out available).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trains_on_data: Option<bool>,

    /// Provider's data retention policy for request/response data.
    /// Well-known values: "none", "30d", "90d", "1y", "indefinite"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_retention: Option<String>,

    /// Model license identifier.
    /// Well-known values: "proprietary", "apache-2.0", "mit", "llama-3.1",
    /// "gemma", "mistral", "cc-by-4.0", "cc-by-nc-4.0"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,

    /// Free-form notes for additional context.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,

    /// Custom key-value metadata. Keys should match a defined custom field.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub custom: HashMap<String, String>,
}

impl SovereigntyMetadata {
    /// Merge model-level overrides onto provider-level defaults.
    ///
    /// Model values take precedence: `Some` wins over `None`, non-empty `Vec` wins.
    pub fn merge(provider: Option<&Self>, model: Option<&Self>) -> Option<Self> {
        match (provider, model) {
            (None, None) => None,
            (Some(p), None) => Some(p.clone()),
            (None, Some(m)) => Some(m.clone()),
            (Some(p), Some(m)) => Some(Self {
                hq_country: m.hq_country.clone().or_else(|| p.hq_country.clone()),
                inference_countries: if m.inference_countries.is_empty() {
                    p.inference_countries.clone()
                } else {
                    m.inference_countries.clone()
                },
                certifications: if m.certifications.is_empty() {
                    p.certifications.clone()
                } else {
                    m.certifications.clone()
                },
                on_prem: m.on_prem.or(p.on_prem),
                trains_on_data: m.trains_on_data.or(p.trains_on_data),
                data_retention: m
                    .data_retention
                    .clone()
                    .or_else(|| p.data_retention.clone()),
                license: m.license.clone().or_else(|| p.license.clone()),
                notes: m.notes.clone().or_else(|| p.notes.clone()),
                custom: if m.custom.is_empty() {
                    p.custom.clone()
                } else {
                    let mut merged = p.custom.clone();
                    merged.extend(m.custom.clone());
                    merged
                },
            }),
        }
    }

    /// Returns true if all fields are empty/None.
    pub fn is_empty(&self) -> bool {
        self.hq_country.is_none()
            && self.inference_countries.is_empty()
            && self.certifications.is_empty()
            && self.on_prem.is_none()
            && self.trains_on_data.is_none()
            && self.data_retention.is_none()
            && self.license.is_none()
            && self.notes.is_none()
            && self.custom.is_empty()
    }
}

/// Sovereignty requirements for API key restrictions.
///
/// When set on an API key, only models whose resolved sovereignty metadata
/// satisfies all specified requirements can be used.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct SovereigntyRequirements {
    /// Only allow models with inference in these countries.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_inference_countries: Option<Vec<String>>,

    /// Only allow on-prem providers (sovereignty.on_prem == true).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub require_on_prem: Option<bool>,

    /// Provider must have ALL of these certifications.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required_certifications: Option<Vec<String>>,

    /// Only allow open-weight models (open_weights == true).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub require_open_weights: Option<bool>,

    /// Exclude models from providers headquartered in these countries.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocked_hq_countries: Option<Vec<String>>,

    /// Only allow models with these licenses.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_licenses: Option<Vec<String>>,
}

impl SovereigntyRequirements {
    /// Merge two sets of requirements (e.g., API key + per-request).
    ///
    /// The result is the intersection of allowed values: if both specify
    /// `allowed_inference_countries`, only countries in both lists survive.
    /// Boolean requirements are OR'd (either source requiring it wins).
    pub fn merge(a: Option<&Self>, b: Option<&Self>) -> Option<Self> {
        match (a, b) {
            (None, None) => None,
            (Some(x), None) | (None, Some(x)) => Some(x.clone()),
            (Some(a), Some(b)) => Some(Self {
                allowed_inference_countries: merge_allowed_lists(
                    &a.allowed_inference_countries,
                    &b.allowed_inference_countries,
                ),
                require_on_prem: merge_bool_require(a.require_on_prem, b.require_on_prem),
                required_certifications: merge_required_lists(
                    &a.required_certifications,
                    &b.required_certifications,
                ),
                require_open_weights: merge_bool_require(
                    a.require_open_weights,
                    b.require_open_weights,
                ),
                blocked_hq_countries: merge_blocked_lists(
                    &a.blocked_hq_countries,
                    &b.blocked_hq_countries,
                ),
                allowed_licenses: merge_allowed_lists(&a.allowed_licenses, &b.allowed_licenses),
            }),
        }
    }

    /// Check whether the given sovereignty metadata satisfies these requirements.
    ///
    /// Returns `Ok(())` if all requirements are met, or `Err` with a description
    /// of the first violation.
    pub fn check(
        &self,
        sovereignty: &SovereigntyMetadata,
        model_open_weights: bool,
    ) -> Result<(), String> {
        // Check inference countries
        if let Some(allowed) = &self.allowed_inference_countries
            && allowed.is_empty()
        {
            return Err(
                "allowed inference countries list is empty (conflicting requirements)".into(),
            );
        }
        if let Some(allowed) = &self.allowed_inference_countries
            && !sovereignty
                .inference_countries
                .iter()
                .any(|c| allowed.iter().any(|a| a.eq_ignore_ascii_case(c)))
        {
            return Err(format!(
                "model inference countries {:?} not in allowed list {:?}",
                sovereignty.inference_countries, allowed
            ));
        }

        // Check on-prem
        if self.require_on_prem == Some(true) && sovereignty.on_prem != Some(true) {
            return Err("model is not on-prem".to_string());
        }

        // Check certifications (must have ALL required)
        if let Some(required) = &self.required_certifications {
            for cert in required {
                if !sovereignty
                    .certifications
                    .iter()
                    .any(|c| c.eq_ignore_ascii_case(cert))
                {
                    return Err(format!("missing required certification '{cert}'"));
                }
            }
        }

        // Check open weights
        if self.require_open_weights == Some(true) && !model_open_weights {
            return Err("model does not have open weights".to_string());
        }

        // Check blocked HQ countries
        if let Some(blocked) = &self.blocked_hq_countries {
            match &sovereignty.hq_country {
                None => {
                    return Err(
                        "provider HQ country is unknown; cannot verify against blocked list".into(),
                    );
                }
                Some(hq) if blocked.iter().any(|b| b.eq_ignore_ascii_case(hq)) => {
                    return Err(format!("provider HQ country '{hq}' is blocked"));
                }
                _ => {}
            }
        }

        // Check allowed licenses
        if let Some(allowed) = &self.allowed_licenses
            && allowed.is_empty()
        {
            return Err("allowed licenses list is empty (conflicting requirements)".into());
        }
        if let Some(allowed) = &self.allowed_licenses {
            match &sovereignty.license {
                Some(license) if allowed.iter().any(|a| a.eq_ignore_ascii_case(license)) => {}
                _ => {
                    return Err(format!(
                        "model license {:?} not in allowed list {allowed:?}",
                        sovereignty.license
                    ));
                }
            }
        }

        Ok(())
    }
}

/// For "allowed" lists, take the intersection if both are set, or whichever is set.
/// Uses case-insensitive comparison to match `check()` behavior.
fn merge_allowed_lists(a: &Option<Vec<String>>, b: &Option<Vec<String>>) -> Option<Vec<String>> {
    match (a, b) {
        (None, None) => None,
        (Some(x), None) | (None, Some(x)) => Some(x.clone()),
        (Some(a), Some(b)) => {
            let intersection: Vec<String> = a
                .iter()
                .filter(|v| b.iter().any(|bv| bv.eq_ignore_ascii_case(v)))
                .cloned()
                .collect();
            Some(intersection)
        }
    }
}

/// For "required" lists, take the union — both sources' requirements must be met.
/// Uses case-insensitive comparison to match `check()` behavior.
fn merge_required_lists(a: &Option<Vec<String>>, b: &Option<Vec<String>>) -> Option<Vec<String>> {
    match (a, b) {
        (None, None) => None,
        (Some(x), None) | (None, Some(x)) => Some(x.clone()),
        (Some(a), Some(b)) => {
            let mut merged = a.clone();
            for v in b {
                if !merged.iter().any(|m| m.eq_ignore_ascii_case(v)) {
                    merged.push(v.clone());
                }
            }
            Some(merged)
        }
    }
}

/// For "blocked" lists, take the union — anything blocked by either source stays blocked.
fn merge_blocked_lists(a: &Option<Vec<String>>, b: &Option<Vec<String>>) -> Option<Vec<String>> {
    merge_required_lists(a, b) // Same semantics: union
}

/// Boolean require: if either source requires it, the result requires it.
fn merge_bool_require(a: Option<bool>, b: Option<bool>) -> Option<bool> {
    match (a, b) {
        (Some(true), _) | (_, Some(true)) => Some(true),
        (Some(false), _) | (_, Some(false)) => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_none_none() {
        assert_eq!(SovereigntyMetadata::merge(None, None), None);
    }

    #[test]
    fn test_merge_provider_only() {
        let provider = SovereigntyMetadata {
            hq_country: Some("US".into()),
            inference_countries: vec!["US".into()],
            certifications: vec!["soc2".into()],
            ..Default::default()
        };
        let result = SovereigntyMetadata::merge(Some(&provider), None).unwrap();
        assert_eq!(result.hq_country, Some("US".into()));
        assert_eq!(result.inference_countries, vec!["US"]);
    }

    #[test]
    fn test_merge_model_overrides_provider() {
        let provider = SovereigntyMetadata {
            hq_country: Some("US".into()),
            inference_countries: vec!["US".into()],
            certifications: vec!["soc2".into()],
            license: Some("proprietary".into()),
            ..Default::default()
        };
        let model = SovereigntyMetadata {
            license: Some("apache-2.0".into()),
            inference_countries: vec!["DE".into()],
            ..Default::default()
        };
        let result = SovereigntyMetadata::merge(Some(&provider), Some(&model)).unwrap();
        assert_eq!(result.hq_country, Some("US".into())); // inherited
        assert_eq!(result.inference_countries, vec!["DE"]); // overridden
        assert_eq!(result.certifications, vec!["soc2"]); // inherited (model empty)
        assert_eq!(result.license, Some("apache-2.0".into())); // overridden
    }

    #[test]
    fn test_requirements_check_pass() {
        let reqs = SovereigntyRequirements {
            allowed_inference_countries: Some(vec!["US".into(), "EU".into()]),
            required_certifications: Some(vec!["soc2".into()]),
            ..Default::default()
        };
        let meta = SovereigntyMetadata {
            inference_countries: vec!["US".into()],
            certifications: vec!["soc2".into(), "gdpr".into()],
            ..Default::default()
        };
        assert!(reqs.check(&meta, false).is_ok());
    }

    #[test]
    fn test_requirements_check_inference_country_fail() {
        let reqs = SovereigntyRequirements {
            allowed_inference_countries: Some(vec!["DE".into()]),
            ..Default::default()
        };
        let meta = SovereigntyMetadata {
            inference_countries: vec!["US".into()],
            ..Default::default()
        };
        assert!(reqs.check(&meta, false).is_err());
    }

    #[test]
    fn test_requirements_check_on_prem_fail() {
        let reqs = SovereigntyRequirements {
            require_on_prem: Some(true),
            ..Default::default()
        };
        let meta = SovereigntyMetadata::default();
        assert!(reqs.check(&meta, false).is_err());
    }

    #[test]
    fn test_requirements_check_certification_fail() {
        let reqs = SovereigntyRequirements {
            required_certifications: Some(vec!["hipaa-baa".into()]),
            ..Default::default()
        };
        let meta = SovereigntyMetadata {
            certifications: vec!["soc2".into()],
            ..Default::default()
        };
        assert!(reqs.check(&meta, false).is_err());
    }

    #[test]
    fn test_requirements_check_open_weights_fail() {
        let reqs = SovereigntyRequirements {
            require_open_weights: Some(true),
            ..Default::default()
        };
        let meta = SovereigntyMetadata::default();
        assert!(reqs.check(&meta, false).is_err());
    }

    #[test]
    fn test_requirements_check_blocked_hq_fail() {
        let reqs = SovereigntyRequirements {
            blocked_hq_countries: Some(vec!["US".into()]),
            ..Default::default()
        };
        let meta = SovereigntyMetadata {
            hq_country: Some("US".into()),
            ..Default::default()
        };
        assert!(reqs.check(&meta, false).is_err());
    }

    #[test]
    fn test_requirements_check_license_fail() {
        let reqs = SovereigntyRequirements {
            allowed_licenses: Some(vec!["apache-2.0".into()]),
            ..Default::default()
        };
        let meta = SovereigntyMetadata {
            license: Some("proprietary".into()),
            ..Default::default()
        };
        assert!(reqs.check(&meta, false).is_err());
    }

    #[test]
    fn test_is_empty() {
        assert!(SovereigntyMetadata::default().is_empty());
        assert!(
            !SovereigntyMetadata {
                hq_country: Some("US".into()),
                ..Default::default()
            }
            .is_empty()
        );
    }

    #[test]
    fn test_merge_custom_fields() {
        let provider = SovereigntyMetadata {
            custom: HashMap::from([
                ("region".into(), "EU".into()),
                ("tier".into(), "enterprise".into()),
            ]),
            ..Default::default()
        };
        let model = SovereigntyMetadata {
            custom: HashMap::from([("region".into(), "US".into())]),
            ..Default::default()
        };
        let result = SovereigntyMetadata::merge(Some(&provider), Some(&model)).unwrap();
        assert_eq!(result.custom.get("region"), Some(&"US".to_string())); // overridden
        assert_eq!(result.custom.get("tier"), Some(&"enterprise".to_string())); // inherited
    }

    #[test]
    fn test_is_empty_with_custom() {
        let mut meta = SovereigntyMetadata::default();
        assert!(meta.is_empty());
        meta.custom.insert("key".into(), "val".into());
        assert!(!meta.is_empty());
    }

    #[test]
    fn test_merge_requirements_none_none() {
        assert!(SovereigntyRequirements::merge(None, None).is_none());
    }

    #[test]
    fn test_merge_requirements_one_side() {
        let reqs = SovereigntyRequirements {
            allowed_inference_countries: Some(vec!["US".into()]),
            ..Default::default()
        };
        let merged = SovereigntyRequirements::merge(Some(&reqs), None).unwrap();
        assert_eq!(merged.allowed_inference_countries, Some(vec!["US".into()]));
    }

    #[test]
    fn test_merge_requirements_intersection_allowed() {
        let a = SovereigntyRequirements {
            allowed_inference_countries: Some(vec!["US".into(), "DE".into(), "FR".into()]),
            ..Default::default()
        };
        let b = SovereigntyRequirements {
            allowed_inference_countries: Some(vec!["DE".into(), "FR".into(), "NL".into()]),
            ..Default::default()
        };
        let merged = SovereigntyRequirements::merge(Some(&a), Some(&b)).unwrap();
        assert_eq!(
            merged.allowed_inference_countries,
            Some(vec!["DE".into(), "FR".into()])
        );
    }

    #[test]
    fn test_merge_requirements_union_required_certs() {
        let a = SovereigntyRequirements {
            required_certifications: Some(vec!["soc2".into()]),
            ..Default::default()
        };
        let b = SovereigntyRequirements {
            required_certifications: Some(vec!["gdpr".into()]),
            ..Default::default()
        };
        let merged = SovereigntyRequirements::merge(Some(&a), Some(&b)).unwrap();
        let certs = merged.required_certifications.unwrap();
        assert!(certs.contains(&"soc2".into()));
        assert!(certs.contains(&"gdpr".into()));
    }

    #[test]
    fn test_merge_requirements_union_blocked() {
        let a = SovereigntyRequirements {
            blocked_hq_countries: Some(vec!["CN".into()]),
            ..Default::default()
        };
        let b = SovereigntyRequirements {
            blocked_hq_countries: Some(vec!["RU".into()]),
            ..Default::default()
        };
        let merged = SovereigntyRequirements::merge(Some(&a), Some(&b)).unwrap();
        let blocked = merged.blocked_hq_countries.unwrap();
        assert!(blocked.contains(&"CN".into()));
        assert!(blocked.contains(&"RU".into()));
    }

    #[test]
    fn test_merge_requirements_bool_or() {
        let a = SovereigntyRequirements {
            require_on_prem: Some(true),
            ..Default::default()
        };
        let b = SovereigntyRequirements {
            require_on_prem: Some(false),
            ..Default::default()
        };
        let merged = SovereigntyRequirements::merge(Some(&a), Some(&b)).unwrap();
        assert_eq!(merged.require_on_prem, Some(true));
    }

    #[test]
    fn test_requirements_check_no_license_vs_allowed_licenses() {
        let reqs = SovereigntyRequirements {
            allowed_licenses: Some(vec!["apache-2.0".into()]),
            ..Default::default()
        };
        let meta = SovereigntyMetadata::default(); // license: None
        let err = reqs.check(&meta, false).unwrap_err();
        assert!(err.contains("not in allowed list"), "{err}");
    }

    #[test]
    fn test_requirements_check_no_hq_vs_blocked_countries() {
        let reqs = SovereigntyRequirements {
            blocked_hq_countries: Some(vec!["CN".into()]),
            ..Default::default()
        };
        let meta = SovereigntyMetadata::default(); // hq_country: None
        let err = reqs.check(&meta, false).unwrap_err();
        assert!(err.contains("unknown"), "{err}");
    }

    #[test]
    fn test_requirements_check_empty_intersection_blocks() {
        let a = SovereigntyRequirements {
            allowed_inference_countries: Some(vec!["US".into()]),
            ..Default::default()
        };
        let b = SovereigntyRequirements {
            allowed_inference_countries: Some(vec!["DE".into()]),
            ..Default::default()
        };
        let merged = SovereigntyRequirements::merge(Some(&a), Some(&b)).unwrap();
        // Intersection is empty
        assert_eq!(merged.allowed_inference_countries, Some(vec![]));
        let meta = SovereigntyMetadata {
            inference_countries: vec!["US".into()],
            ..Default::default()
        };
        assert!(merged.check(&meta, false).is_err());
    }

    #[test]
    fn test_requirements_check_empty_inference_countries_error_message() {
        let reqs = SovereigntyRequirements {
            allowed_inference_countries: Some(vec![]),
            ..Default::default()
        };
        let meta = SovereigntyMetadata {
            inference_countries: vec!["US".into()],
            ..Default::default()
        };
        let err = reqs.check(&meta, false).unwrap_err();
        assert!(err.contains("conflicting requirements"), "{err}");
    }

    #[test]
    fn test_requirements_check_empty_licenses_error_message() {
        let reqs = SovereigntyRequirements {
            allowed_licenses: Some(vec![]),
            ..Default::default()
        };
        let meta = SovereigntyMetadata {
            license: Some("mit".into()),
            ..Default::default()
        };
        let err = reqs.check(&meta, false).unwrap_err();
        assert!(err.contains("conflicting requirements"), "{err}");
    }

    #[test]
    fn test_requirements_check_case_insensitive_country() {
        let reqs = SovereigntyRequirements {
            allowed_inference_countries: Some(vec!["US".into()]),
            ..Default::default()
        };
        let meta = SovereigntyMetadata {
            inference_countries: vec!["us".into()],
            ..Default::default()
        };
        assert!(reqs.check(&meta, false).is_ok());
    }

    #[test]
    fn test_requirements_check_case_insensitive_certification() {
        let reqs = SovereigntyRequirements {
            required_certifications: Some(vec!["SOC2".into()]),
            ..Default::default()
        };
        let meta = SovereigntyMetadata {
            certifications: vec!["soc2".into()],
            ..Default::default()
        };
        assert!(reqs.check(&meta, false).is_ok());
    }

    #[test]
    fn test_requirements_check_case_insensitive_license() {
        let reqs = SovereigntyRequirements {
            allowed_licenses: Some(vec!["Apache-2.0".into()]),
            ..Default::default()
        };
        let meta = SovereigntyMetadata {
            license: Some("apache-2.0".into()),
            ..Default::default()
        };
        assert!(reqs.check(&meta, false).is_ok());
    }

    #[test]
    fn test_requirements_check_no_inference_countries_vs_allowed() {
        let reqs = SovereigntyRequirements {
            allowed_inference_countries: Some(vec!["US".into()]),
            ..Default::default()
        };
        let meta = SovereigntyMetadata::default(); // inference_countries: []
        assert!(reqs.check(&meta, false).is_err());
    }

    #[test]
    fn test_requirements_check_license_passes_when_in_list() {
        let reqs = SovereigntyRequirements {
            allowed_licenses: Some(vec!["apache-2.0".into(), "mit".into()]),
            ..Default::default()
        };
        let meta = SovereigntyMetadata {
            license: Some("mit".into()),
            ..Default::default()
        };
        assert!(reqs.check(&meta, false).is_ok());
    }

    #[test]
    fn test_merge_allowed_lists_case_insensitive() {
        let a = SovereigntyRequirements {
            allowed_inference_countries: Some(vec!["US".into(), "DE".into()]),
            ..Default::default()
        };
        let b = SovereigntyRequirements {
            allowed_inference_countries: Some(vec!["us".into(), "fr".into()]),
            ..Default::default()
        };
        let merged = SovereigntyRequirements::merge(Some(&a), Some(&b)).unwrap();
        assert_eq!(merged.allowed_inference_countries, Some(vec!["US".into()]));
    }

    #[test]
    fn test_merge_required_lists_case_insensitive() {
        let a = SovereigntyRequirements {
            required_certifications: Some(vec!["SOC2".into()]),
            ..Default::default()
        };
        let b = SovereigntyRequirements {
            required_certifications: Some(vec!["soc2".into(), "gdpr".into()]),
            ..Default::default()
        };
        let merged = SovereigntyRequirements::merge(Some(&a), Some(&b)).unwrap();
        let certs = merged.required_certifications.unwrap();
        // "soc2" should not be added as a duplicate of "SOC2"
        assert_eq!(certs.len(), 2);
        assert!(certs.contains(&"SOC2".into()));
        assert!(certs.contains(&"gdpr".into()));
    }

    #[test]
    fn test_merge_blocked_lists_case_insensitive() {
        let a = SovereigntyRequirements {
            blocked_hq_countries: Some(vec!["CN".into()]),
            ..Default::default()
        };
        let b = SovereigntyRequirements {
            blocked_hq_countries: Some(vec!["cn".into(), "RU".into()]),
            ..Default::default()
        };
        let merged = SovereigntyRequirements::merge(Some(&a), Some(&b)).unwrap();
        let blocked = merged.blocked_hq_countries.unwrap();
        // "cn" should not be added as a duplicate of "CN"
        assert_eq!(blocked.len(), 2);
        assert!(blocked.contains(&"CN".into()));
        assert!(blocked.contains(&"RU".into()));
    }
}
