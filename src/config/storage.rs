//! File storage configuration for RAG and Files API.
//!
//! Supports multiple storage backends for file content:
//! - **Database**: Store files directly in the database (default, simplest)
//! - **Filesystem**: Store files on the local filesystem
//! - **S3**: Store files in S3-compatible object storage
//!
//! # Example Configuration
//!
//! ```toml
//! [storage.files]
//! backend = "s3"
//!
//! [storage.files.s3]
//! bucket = "hadrian-files"
//! region = "us-east-1"
//! # Credentials via env vars AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY
//! # or IAM role
//!
//! [storage.files.filesystem]
//! path = "/var/hadrian/files"
//! ```

use serde::{Deserialize, Serialize};

/// Storage configuration for files and other binary data.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct StorageConfig {
    /// File storage configuration.
    #[serde(default)]
    pub files: FileStorageConfig,
}

/// File storage backend configuration for the Files API.
///
/// Determines where file content is stored. The database always stores
/// file metadata; this config only affects where the actual file bytes live.
///
/// Note: For chat upload storage, see `UploadStorageConfig` in `ui.rs`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FileStorageConfig {
    /// Storage backend to use.
    #[serde(default)]
    pub backend: FileStorageBackend,

    /// S3 configuration (required when backend = "s3").
    #[serde(default)]
    pub s3: Option<S3StorageConfig>,

    /// Filesystem configuration (required when backend = "filesystem").
    #[serde(default)]
    pub filesystem: Option<FilesystemStorageConfig>,
}

impl Default for FileStorageConfig {
    fn default() -> Self {
        Self {
            backend: FileStorageBackend::Database,
            s3: None,
            filesystem: None,
        }
    }
}

impl FileStorageConfig {
    /// Validate the storage configuration.
    pub fn validate(&self) -> Result<(), String> {
        match self.backend {
            FileStorageBackend::Database => Ok(()),
            FileStorageBackend::S3 => {
                if self.s3.is_none() {
                    return Err(
                        "S3 storage backend requires [storage.files.s3] configuration".to_string(),
                    );
                }
                self.s3.as_ref().unwrap().validate()
            }
            FileStorageBackend::Filesystem => {
                if self.filesystem.is_none() {
                    return Err(
                        "Filesystem storage backend requires [storage.files.filesystem] configuration".to_string(),
                    );
                }
                self.filesystem.as_ref().unwrap().validate()
            }
        }
    }
}

/// Storage backend type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum FileStorageBackend {
    /// Store file content directly in the database.
    /// Simplest option, good for small deployments.
    /// Files stored in the `file_data` column.
    #[default]
    Database,

    /// Store file content on the local filesystem.
    /// Good for single-node deployments with large files.
    /// Database stores the file path reference.
    Filesystem,

    /// Store file content in S3-compatible object storage.
    /// Best for production, multi-node deployments.
    /// Supports AWS S3, MinIO, R2, DigitalOcean Spaces, etc.
    S3,
}

/// S3-compatible object storage configuration.
#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct S3StorageConfig {
    /// S3 bucket name.
    pub bucket: String,

    /// AWS region (e.g., "us-east-1").
    /// For non-AWS S3-compatible services, use their region name.
    #[serde(default)]
    pub region: Option<String>,

    /// Custom endpoint URL for S3-compatible services.
    /// Examples:
    /// - MinIO: "http://localhost:9000"
    /// - R2: "https://<account-id>.r2.cloudflarestorage.com"
    /// - DigitalOcean Spaces: "https://<region>.digitaloceanspaces.com"
    #[serde(default)]
    pub endpoint: Option<String>,

    /// AWS access key ID.
    /// If not specified, uses environment variables or IAM role.
    #[serde(default)]
    pub access_key_id: Option<String>,

    /// AWS secret access key.
    /// If not specified, uses environment variables or IAM role.
    #[serde(default)]
    pub secret_access_key: Option<String>,

    /// Use path-style URLs instead of virtual-hosted style.
    /// Required for MinIO and some S3-compatible services.
    /// Default: false (use virtual-hosted style)
    #[serde(default)]
    pub force_path_style: bool,

    /// Key prefix for all stored files.
    /// Useful for organizing files in a shared bucket.
    /// Example: "hadrian/files/" would store files as "hadrian/files/<file-id>"
    #[serde(default)]
    pub key_prefix: Option<String>,

    /// Storage class for new objects.
    /// AWS: STANDARD, REDUCED_REDUNDANCY, STANDARD_IA, ONEZONE_IA, INTELLIGENT_TIERING, GLACIER, DEEP_ARCHIVE
    #[serde(default)]
    pub storage_class: Option<String>,

    /// Enable server-side encryption.
    #[serde(default)]
    pub server_side_encryption: Option<S3ServerSideEncryption>,
}

impl std::fmt::Debug for S3StorageConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("S3StorageConfig")
            .field("bucket", &self.bucket)
            .field("region", &self.region)
            .field("endpoint", &self.endpoint)
            .field(
                "access_key_id",
                &self.access_key_id.as_ref().map(|_| "****"),
            )
            .field(
                "secret_access_key",
                &self.secret_access_key.as_ref().map(|_| "****"),
            )
            .field("force_path_style", &self.force_path_style)
            .field("key_prefix", &self.key_prefix)
            .field("storage_class", &self.storage_class)
            .field("server_side_encryption", &self.server_side_encryption)
            .finish()
    }
}

impl S3StorageConfig {
    /// Validate S3 configuration.
    pub fn validate(&self) -> Result<(), String> {
        if self.bucket.is_empty() {
            return Err("S3 bucket name cannot be empty".to_string());
        }
        // Region is required unless using a custom endpoint
        if self.region.is_none() && self.endpoint.is_none() {
            return Err("S3 requires either 'region' or 'endpoint' to be specified".to_string());
        }
        Ok(())
    }

    /// Generate the S3 key for a file.
    pub fn file_key(&self, file_id: &str) -> String {
        match &self.key_prefix {
            Some(prefix) => {
                let prefix = prefix.trim_end_matches('/');
                format!("{}/{}", prefix, file_id)
            }
            None => file_id.to_string(),
        }
    }
}

/// S3 server-side encryption configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "type", rename_all = "snake_case")]
#[serde(deny_unknown_fields)]
pub enum S3ServerSideEncryption {
    /// Server-side encryption with Amazon S3-managed keys (SSE-S3).
    Aes256,

    /// Server-side encryption with AWS KMS keys (SSE-KMS).
    Kms {
        /// KMS key ID or ARN.
        key_id: String,
    },
}

/// Local filesystem storage configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FilesystemStorageConfig {
    /// Base directory for file storage.
    /// Files are stored as `{path}/{file-id}`.
    pub path: String,

    /// Create the directory if it doesn't exist.
    /// Default: true
    #[serde(default = "default_true")]
    pub create_dir: bool,

    /// File permissions (Unix mode) for new files.
    /// Default: 0o600 (owner read/write only)
    #[serde(default = "default_file_mode")]
    pub file_mode: u32,

    /// Directory permissions (Unix mode) for new directories.
    /// Default: 0o700 (owner read/write/execute only)
    #[serde(default = "default_dir_mode")]
    pub dir_mode: u32,
}

impl FilesystemStorageConfig {
    /// Validate filesystem configuration.
    pub fn validate(&self) -> Result<(), String> {
        if self.path.is_empty() {
            return Err("Filesystem storage path cannot be empty".to_string());
        }
        Ok(())
    }

    /// Get the full path for a file.
    pub fn file_path(&self, file_id: &str) -> std::path::PathBuf {
        std::path::PathBuf::from(&self.path).join(file_id)
    }
}

impl Default for FilesystemStorageConfig {
    fn default() -> Self {
        Self {
            path: "/var/hadrian/files".to_string(),
            create_dir: true,
            file_mode: default_file_mode(),
            dir_mode: default_dir_mode(),
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_file_mode() -> u32 {
    0o600
}

fn default_dir_mode() -> u32 {
    0o700
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_storage_config() {
        let config: StorageConfig = toml::from_str("").unwrap();
        assert!(matches!(config.files.backend, FileStorageBackend::Database));
        assert!(config.files.s3.is_none());
        assert!(config.files.filesystem.is_none());
    }

    #[test]
    fn test_database_backend() {
        let config: FileStorageConfig = toml::from_str(
            r#"
            backend = "database"
            "#,
        )
        .unwrap();

        assert!(matches!(config.backend, FileStorageBackend::Database));
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_s3_backend() {
        let config: FileStorageConfig = toml::from_str(
            r#"
            backend = "s3"

            [s3]
            bucket = "my-bucket"
            region = "us-east-1"
            "#,
        )
        .unwrap();

        assert!(matches!(config.backend, FileStorageBackend::S3));
        assert!(config.validate().is_ok());

        let s3 = config.s3.unwrap();
        assert_eq!(s3.bucket, "my-bucket");
        assert_eq!(s3.region, Some("us-east-1".to_string()));
    }

    #[test]
    fn test_s3_with_custom_endpoint() {
        let config: FileStorageConfig = toml::from_str(
            r#"
            backend = "s3"

            [s3]
            bucket = "my-bucket"
            endpoint = "http://localhost:9000"
            force_path_style = true
            "#,
        )
        .unwrap();

        assert!(config.validate().is_ok());

        let s3 = config.s3.unwrap();
        assert_eq!(s3.endpoint, Some("http://localhost:9000".to_string()));
        assert!(s3.force_path_style);
    }

    #[test]
    fn test_s3_with_encryption() {
        let config: FileStorageConfig = toml::from_str(
            r#"
            backend = "s3"

            [s3]
            bucket = "my-bucket"
            region = "us-east-1"

            [s3.server_side_encryption]
            type = "kms"
            key_id = "arn:aws:kms:us-east-1:123456789:key/abc123"
            "#,
        )
        .unwrap();

        let s3 = config.s3.unwrap();
        match s3.server_side_encryption.unwrap() {
            S3ServerSideEncryption::Kms { key_id } => {
                assert!(key_id.starts_with("arn:aws:kms"));
            }
            _ => panic!("Expected KMS encryption"),
        }
    }

    #[test]
    fn test_s3_missing_region_and_endpoint() {
        let config: FileStorageConfig = toml::from_str(
            r#"
            backend = "s3"

            [s3]
            bucket = "my-bucket"
            "#,
        )
        .unwrap();

        assert!(config.validate().is_err());
    }

    #[test]
    fn test_filesystem_backend() {
        let config: FileStorageConfig = toml::from_str(
            r#"
            backend = "filesystem"

            [filesystem]
            path = "/var/data/files"
            "#,
        )
        .unwrap();

        assert!(matches!(config.backend, FileStorageBackend::Filesystem));
        assert!(config.validate().is_ok());

        let fs = config.filesystem.unwrap();
        assert_eq!(fs.path, "/var/data/files");
        assert!(fs.create_dir); // default
        assert_eq!(fs.file_mode, 0o600); // default
    }

    #[test]
    fn test_filesystem_custom_permissions() {
        let config: FileStorageConfig = toml::from_str(
            r#"
            backend = "filesystem"

            [filesystem]
            path = "/var/data/files"
            create_dir = false
            file_mode = 420  # 0o644
            dir_mode = 493   # 0o755
            "#,
        )
        .unwrap();

        let fs = config.filesystem.unwrap();
        assert!(!fs.create_dir);
        assert_eq!(fs.file_mode, 420);
        assert_eq!(fs.dir_mode, 493);
    }

    #[test]
    fn test_filesystem_missing_config() {
        let config: FileStorageConfig = toml::from_str(
            r#"
            backend = "filesystem"
            "#,
        )
        .unwrap();

        assert!(config.validate().is_err());
    }

    #[test]
    fn test_s3_file_key_generation() {
        let config = S3StorageConfig {
            bucket: "test".to_string(),
            region: Some("us-east-1".to_string()),
            endpoint: None,
            access_key_id: None,
            secret_access_key: None,
            force_path_style: false,
            key_prefix: Some("hadrian/files/".to_string()),
            storage_class: None,
            server_side_encryption: None,
        };

        assert_eq!(config.file_key("abc-123"), "hadrian/files/abc-123");

        let config_no_prefix = S3StorageConfig {
            key_prefix: None,
            ..config
        };
        assert_eq!(config_no_prefix.file_key("abc-123"), "abc-123");
    }

    #[test]
    fn test_filesystem_file_path_generation() {
        let config = FilesystemStorageConfig {
            path: "/var/hadrian/files".to_string(),
            create_dir: true,
            file_mode: 0o600,
            dir_mode: 0o700,
        };

        assert_eq!(
            config.file_path("abc-123"),
            std::path::PathBuf::from("/var/hadrian/files/abc-123")
        );
    }

    #[test]
    fn test_full_storage_config() {
        let config: StorageConfig = toml::from_str(
            r#"
            [files]
            backend = "s3"

            [files.s3]
            bucket = "hadrian-files"
            region = "us-west-2"
            key_prefix = "prod/files/"
            storage_class = "STANDARD_IA"
            "#,
        )
        .unwrap();

        assert!(matches!(config.files.backend, FileStorageBackend::S3));
        let s3 = config.files.s3.unwrap();
        assert_eq!(s3.bucket, "hadrian-files");
        assert_eq!(s3.storage_class, Some("STANDARD_IA".to_string()));
    }

    #[test]
    fn test_s3_storage_config_debug_redacts_credentials() {
        let config = S3StorageConfig {
            bucket: "my-bucket".to_string(),
            region: Some("us-east-1".to_string()),
            endpoint: None,
            access_key_id: Some("AKIAIOSFODNN7EXAMPLE".to_string()),
            secret_access_key: Some("wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".to_string()),
            force_path_style: false,
            key_prefix: Some("prefix/".to_string()),
            storage_class: None,
            server_side_encryption: None,
        };

        let debug_output = format!("{:?}", config);
        assert!(
            debug_output.contains("****"),
            "Debug output should contain redacted marker"
        );
        assert!(
            !debug_output.contains("AKIAIOSFODNN7EXAMPLE"),
            "Debug output must NOT contain access key ID"
        );
        assert!(
            !debug_output.contains("wJalrXUtnFEMI"),
            "Debug output must NOT contain secret access key"
        );
        // Non-sensitive fields should still be visible
        assert!(
            debug_output.contains("my-bucket"),
            "Bucket should be visible"
        );
        assert!(
            debug_output.contains("us-east-1"),
            "Region should be visible"
        );
    }

    #[test]
    fn test_s3_storage_config_debug_no_credentials() {
        // When credentials are None, should show None not ****
        let config = S3StorageConfig {
            bucket: "my-bucket".to_string(),
            region: Some("us-east-1".to_string()),
            endpoint: None,
            access_key_id: None,
            secret_access_key: None,
            force_path_style: false,
            key_prefix: None,
            storage_class: None,
            server_side_encryption: None,
        };

        let debug_output = format!("{:?}", config);
        assert!(
            debug_output.contains("access_key_id: None"),
            "Debug output should show None for missing access_key_id"
        );
        assert!(
            debug_output.contains("secret_access_key: None"),
            "Debug output should show None for missing secret_access_key"
        );
    }
}
