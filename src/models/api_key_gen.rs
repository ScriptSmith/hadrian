use base64::Engine;
use rand::Rng;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

/// Default API key prefix
pub const DEFAULT_API_KEY_PREFIX: &str = "gw_live_";

/// Generate a new API key with the given prefix.
///
/// Returns a tuple of (raw_key, key_hash) where:
/// - raw_key is the full key to show the user (only shown once on creation)
/// - key_hash is the SHA-256 hash to store in the database
pub fn generate_api_key_with_prefix(prefix: &str) -> (String, String) {
    // Generate 32 random bytes (256 bits of entropy)
    let mut rng = rand::thread_rng();
    let mut random_bytes = [0u8; 32];
    rng.fill(&mut random_bytes);

    // Encode as base64 (URL-safe, no padding)
    let random_part = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(random_bytes);

    // Create the full key with prefix
    let raw_key = format!("{}{}", prefix, random_part);

    // Hash the key for storage
    let key_hash = hash_api_key(&raw_key);

    (raw_key, key_hash)
}

/// Generate a new API key with the default prefix `gw_live_`.
///
/// Returns a tuple of (raw_key, key_hash) where:
/// - raw_key is the full key to show the user (only shown once on creation)
/// - key_hash is the SHA-256 hash to store in the database
#[allow(dead_code)] // Used in tests; public API convenience wrapper
pub fn generate_api_key() -> (String, String) {
    generate_api_key_with_prefix(DEFAULT_API_KEY_PREFIX)
}

/// Hash an API key using SHA-256
///
/// Returns the hex-encoded hash for storage in the database.
pub fn hash_api_key(key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    let result = hasher.finalize();
    hex::encode(result)
}

/// Verify that a raw key matches a stored hash using constant-time comparison.
///
/// Uses constant-time comparison to prevent timing attacks that could
/// leak information about the correct hash value.
#[allow(dead_code)] // Used in tests; public API for key verification
pub fn verify_api_key(raw_key: &str, stored_hash: &str) -> bool {
    let computed_hash = hash_api_key(raw_key);
    // Use constant-time comparison to prevent timing attacks
    computed_hash
        .as_bytes()
        .ct_eq(stored_hash.as_bytes())
        .into()
}

/// Check if an API key has a valid prefix using constant-time comparison.
///
/// This function always takes the same amount of time regardless of
/// how many characters match, preventing timing attacks.
pub fn has_valid_prefix(key: &str, expected_prefix: &str) -> bool {
    if key.len() < expected_prefix.len() {
        return false;
    }
    let key_prefix = &key[..expected_prefix.len()];
    key_prefix
        .as_bytes()
        .ct_eq(expected_prefix.as_bytes())
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_has_valid_prefix() {
        // Valid prefixes
        assert!(has_valid_prefix("gw_live_abcd123", "gw_live_"));
        assert!(has_valid_prefix("gw_test_xyz789", "gw_test_"));
        assert!(has_valid_prefix("custom_prefix_value", "custom_prefix_"));

        // Invalid prefixes
        assert!(!has_valid_prefix("invalid_key", "gw_live_"));
        assert!(!has_valid_prefix("gw_live", "gw_live_")); // Too short (missing _)
        assert!(!has_valid_prefix("", "gw_live_"));
        assert!(!has_valid_prefix("gw_test_", "gw_live_")); // Wrong prefix

        // Edge cases
        assert!(has_valid_prefix("gw_live_", "gw_live_")); // Exact match
        assert!(!has_valid_prefix("gw_", "gw_live_")); // Key shorter than prefix
    }

    #[test]
    fn test_generate_api_key() {
        let (raw_key, _hash) = generate_api_key();

        // Check format
        assert!(raw_key.starts_with(DEFAULT_API_KEY_PREFIX));

        // Check length (prefix + base64-encoded 32 bytes)
        // "gw_live_" is 8 characters
        // 32 bytes in base64 without padding is 43 characters
        assert_eq!(raw_key.len(), DEFAULT_API_KEY_PREFIX.len() + 43);
    }

    #[test]
    fn test_generate_api_key_with_custom_prefix() {
        let (raw_key, _hash) = generate_api_key_with_prefix("custom_");

        assert!(raw_key.starts_with("custom_"));
        assert_eq!(raw_key.len(), 7 + 43); // "custom_" is 7 chars
    }

    #[test]
    fn test_generate_unique_keys() {
        let (key1, _) = generate_api_key();
        let (key2, _) = generate_api_key();

        // Generated keys should be different
        assert_ne!(key1, key2);
    }

    #[test]
    fn test_hash_api_key() {
        let key = "gw_live_test123";
        let hash1 = hash_api_key(key);
        let hash2 = hash_api_key(key);

        // Same key should produce same hash
        assert_eq!(hash1, hash2);

        // Hash should be 64 characters (SHA-256 in hex)
        assert_eq!(hash1.len(), 64);

        // Hash should be hex
        assert!(hash1.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_verify_api_key() {
        let (raw_key, hash) = generate_api_key();

        // Correct key should verify
        assert!(verify_api_key(&raw_key, &hash));

        // Wrong key should not verify
        assert!(!verify_api_key("wrong_key", &hash));
    }

    #[test]
    fn test_different_keys_different_hashes() {
        let (key1, hash1) = generate_api_key();
        let (key2, hash2) = generate_api_key();

        // Different keys should produce different hashes
        assert_ne!(hash1, hash2);

        // Each key should only verify against its own hash
        assert!(verify_api_key(&key1, &hash1));
        assert!(verify_api_key(&key2, &hash2));
        assert!(!verify_api_key(&key1, &hash2));
        assert!(!verify_api_key(&key2, &hash1));
    }
}
