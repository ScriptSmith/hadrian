//! Shared image utilities for LLM providers.
//!
//! Provides common functionality for handling images in chat completion requests:
//! - Parsing data URLs (base64-encoded images)
//! - Fetching images from HTTP URLs and converting to base64
//! - Configuration for image fetching (timeouts, size limits)
//! - Message preprocessing to convert HTTP image URLs to data URLs

use std::time::Duration;

/// Default maximum image size in bytes (20 MB).
const DEFAULT_MAX_IMAGE_SIZE_BYTES: usize = 20 * 1024 * 1024;

/// Default timeout for fetching images from URLs (30 seconds).
const DEFAULT_IMAGE_FETCH_TIMEOUT_SECS: u64 = 30;

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use reqwest::Client;
use thiserror::Error;

use crate::api_types::chat_completion::{ContentPart, Message, MessageContent};

/// Configuration for HTTP image fetching
#[derive(Debug, Clone)]
pub struct ImageFetchConfig {
    /// Whether to enable HTTP image URL fetching (default: true)
    pub enabled: bool,
    /// Maximum image size in bytes (default: 20MB)
    pub max_size_bytes: usize,
    /// Timeout for fetching images (default: 30s)
    pub timeout: Duration,
    /// Allowed content types (empty = allow all image types)
    pub allowed_content_types: Vec<String>,
}

impl Default for ImageFetchConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_size_bytes: DEFAULT_MAX_IMAGE_SIZE_BYTES,
            timeout: Duration::from_secs(DEFAULT_IMAGE_FETCH_TIMEOUT_SECS),
            allowed_content_types: vec![
                "image/png".to_string(),
                "image/jpeg".to_string(),
                "image/gif".to_string(),
                "image/webp".to_string(),
            ],
        }
    }
}

/// Errors that can occur during image processing
#[derive(Debug, Error)]
pub enum ImageError {
    #[error("HTTP image fetching is disabled")]
    FetchingDisabled,
    #[error("Image too large: {size} bytes exceeds limit of {limit} bytes")]
    TooLarge { size: usize, limit: usize },
    #[error("Unsupported content type: {0}")]
    UnsupportedContentType(String),
    #[error("Failed to fetch image: {0}")]
    FetchError(String),
    #[error("Image URL timeout after {0:?}")]
    Timeout(Duration),
    #[error("Invalid URL: {0}")]
    InvalidUrl(String),
    #[error("Missing content type header")]
    MissingContentType,
}

/// Errors that can occur when parsing a data URL
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DataUrlParseError {
    #[error("URL does not start with 'data:' prefix")]
    MissingDataPrefix,
    #[error(
        "Missing semicolon separator between media type and encoding (expected format: data:<media_type>;base64,<data>)"
    )]
    MissingSemicolon,
    #[error(
        "Missing 'base64,' marker after semicolon (expected format: data:<media_type>;base64,<data>)"
    )]
    MissingBase64Marker,
    #[error("Empty media type in data URL")]
    EmptyMediaType,
    #[error("Empty base64 data in data URL")]
    EmptyData,
}

/// Result of parsing or fetching an image
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageData {
    /// MIME type (e.g., "image/png")
    pub media_type: String,
    /// Base64-encoded image data
    pub data: String,
}

/// Parse a data URL into media type and base64 data.
///
/// Supports format: `data:<media_type>;base64,<data>`
///
/// Returns descriptive errors for each parsing step to help diagnose malformed data URLs.
///
/// # Example
/// ```ignore
/// let result = parse_data_url("data:image/png;base64,iVBORw0KGgo=");
/// assert!(result.is_ok());
/// let data = result.unwrap();
/// assert_eq!(data.media_type, "image/png");
/// assert_eq!(data.data, "iVBORw0KGgo=");
/// ```
///
/// # Errors
/// - `MissingDataPrefix` - URL doesn't start with "data:"
/// - `MissingSemicolon` - No semicolon between media type and encoding
/// - `MissingBase64Marker` - No "base64," after the semicolon
/// - `EmptyMediaType` - Media type portion is empty
/// - `EmptyData` - Base64 data portion is empty
pub fn parse_data_url(url: &str) -> Result<ImageData, DataUrlParseError> {
    // data:image/png;base64,iVBORw0KGgo...
    let url = url
        .strip_prefix("data:")
        .ok_or(DataUrlParseError::MissingDataPrefix)?;

    let (media_type, rest) = url
        .split_once(';')
        .ok_or(DataUrlParseError::MissingSemicolon)?;

    if media_type.is_empty() {
        return Err(DataUrlParseError::EmptyMediaType);
    }

    let data = rest
        .strip_prefix("base64,")
        .ok_or(DataUrlParseError::MissingBase64Marker)?;

    if data.is_empty() {
        return Err(DataUrlParseError::EmptyData);
    }

    Ok(ImageData {
        media_type: media_type.to_string(),
        data: data.to_string(),
    })
}

/// Check if a URL is an HTTP/HTTPS URL
pub fn is_http_url(url: &str) -> bool {
    url.starts_with("http://") || url.starts_with("https://")
}

/// Fetch an image from an HTTP URL and convert to base64.
///
/// This function:
/// 1. Validates the URL is HTTP/HTTPS
/// 2. Fetches the image with configured timeout
/// 3. Validates content type and size
/// 4. Converts to base64
///
/// # Errors
/// Returns `ImageError` if fetching fails, times out, or validation fails.
#[tracing::instrument(skip(client, config), fields(url = %url))]
pub async fn fetch_image_url(
    client: &Client,
    url: &str,
    config: &ImageFetchConfig,
) -> Result<ImageData, ImageError> {
    if !config.enabled {
        return Err(ImageError::FetchingDisabled);
    }

    if !is_http_url(url) {
        return Err(ImageError::InvalidUrl(format!(
            "Expected HTTP/HTTPS URL, got: {}",
            url
        )));
    }

    // Build request with timeout
    let response = client
        .get(url)
        .timeout(config.timeout)
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                ImageError::Timeout(config.timeout)
            } else {
                ImageError::FetchError(e.to_string())
            }
        })?;

    // Check response status
    if !response.status().is_success() {
        return Err(ImageError::FetchError(format!(
            "HTTP {}: {}",
            response.status().as_u16(),
            response.status().canonical_reason().unwrap_or("Unknown")
        )));
    }

    // Get content type
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|s| {
            // Extract just the media type, ignoring charset etc.
            s.split(';').next().unwrap_or(s).trim().to_string()
        })
        .ok_or(ImageError::MissingContentType)?;

    // Validate content type if restrictions are configured
    if !config.allowed_content_types.is_empty()
        && !config.allowed_content_types.contains(&content_type)
    {
        return Err(ImageError::UnsupportedContentType(content_type));
    }

    // Check content length if available
    if let Some(content_length) = response.content_length()
        && content_length as usize > config.max_size_bytes
    {
        return Err(ImageError::TooLarge {
            size: content_length as usize,
            limit: config.max_size_bytes,
        });
    }

    // Fetch the body
    let bytes = response.bytes().await.map_err(|e| {
        if e.is_timeout() {
            ImageError::Timeout(config.timeout)
        } else {
            ImageError::FetchError(e.to_string())
        }
    })?;

    // Check actual size
    if bytes.len() > config.max_size_bytes {
        return Err(ImageError::TooLarge {
            size: bytes.len(),
            limit: config.max_size_bytes,
        });
    }

    // Convert to base64
    let data = BASE64.encode(&bytes);

    tracing::debug!(
        content_type = %content_type,
        size_bytes = bytes.len(),
        "Successfully fetched image from URL"
    );

    Ok(ImageData {
        media_type: content_type,
        data,
    })
}

/// Resolve an image URL to ImageData, handling both data URLs and HTTP URLs.
///
/// This is the main entry point for image handling in providers:
/// - Data URLs are parsed directly
/// - HTTP URLs are fetched and converted to base64
///
/// # Arguments
/// * `client` - HTTP client for fetching remote images
/// * `url` - The image URL (data URL or HTTP URL)
/// * `config` - Configuration for HTTP fetching (can be None to disable HTTP fetching)
///
/// # Returns
/// * `Ok(Some(ImageData))` - Successfully resolved image
/// * `Ok(None)` - URL type not supported (e.g., HTTP when disabled)
/// * `Err(ImageError)` - Error during fetching
pub async fn resolve_image_url(
    client: &Client,
    url: &str,
    config: Option<&ImageFetchConfig>,
) -> Result<Option<ImageData>, ImageError> {
    // Try data URL first (no network required)
    if url.starts_with("data:") {
        match parse_data_url(url) {
            Ok(data) => return Ok(Some(data)),
            Err(e) => {
                tracing::warn!(url = %url, error = %e, "Failed to parse data URL");
                return Ok(None);
            }
        }
    }

    // Try HTTP URL
    if is_http_url(url) {
        if let Some(cfg) = config {
            let data = fetch_image_url(client, url, cfg).await?;
            return Ok(Some(data));
        } else {
            // HTTP fetching disabled - return None (caller should handle/warn)
            return Ok(None);
        }
    }

    // Unknown URL format
    Ok(None)
}

/// Convert ImageData to a data URL string.
pub fn to_data_url(data: &ImageData) -> String {
    format!("data:{};base64,{}", data.media_type, data.data)
}

/// Preprocess messages to convert HTTP image URLs to data URLs.
///
/// This function modifies messages in-place, fetching any HTTP image URLs
/// and converting them to base64 data URLs. This allows synchronous conversion
/// functions to work with images that were originally HTTP URLs.
///
/// # Arguments
/// * `client` - HTTP client for fetching remote images
/// * `messages` - Messages to preprocess (modified in-place)
/// * `config` - Configuration for HTTP fetching (None = skip HTTP URLs with warning)
///
/// # Returns
/// The number of images that were successfully fetched and converted.
#[tracing::instrument(skip(client, messages, config), fields(message_count = messages.len()))]
pub async fn preprocess_messages_for_images(
    client: &Client,
    messages: &mut [Message],
    config: Option<&ImageFetchConfig>,
) -> usize {
    let mut fetched_count = 0;

    for message in messages.iter_mut() {
        let content = match message {
            Message::System { content, .. } => Some(content),
            Message::User { content, .. } => Some(content),
            Message::Assistant { content, .. } => content.as_mut(),
            Message::Tool { content, .. } => Some(content),
            Message::Developer { content, .. } => Some(content),
        };

        if let Some(content) = content {
            fetched_count += preprocess_content_for_images(client, content, config).await;
        }
    }

    if fetched_count > 0 {
        tracing::debug!(fetched_count, "Preprocessed HTTP image URLs");
    }

    fetched_count
}

/// Preprocess message content to convert HTTP image URLs to data URLs.
async fn preprocess_content_for_images(
    client: &Client,
    content: &mut MessageContent,
    config: Option<&ImageFetchConfig>,
) -> usize {
    match content {
        MessageContent::Text(_) => 0,
        MessageContent::Parts(parts) => {
            let mut fetched_count = 0;

            for part in parts.iter_mut() {
                if let ContentPart::ImageUrl { image_url, .. } = part {
                    // Skip if already a data URL
                    if image_url.url.starts_with("data:") {
                        continue;
                    }

                    // Try to fetch HTTP URL
                    if is_http_url(&image_url.url) {
                        match resolve_image_url(client, &image_url.url, config).await {
                            Ok(Some(data)) => {
                                // Convert to data URL
                                let original_url = image_url.url.clone();
                                image_url.url = to_data_url(&data);
                                fetched_count += 1;
                                tracing::debug!(
                                    original_url = %original_url,
                                    media_type = %data.media_type,
                                    "Converted HTTP image URL to data URL"
                                );
                            }
                            Ok(None) => {
                                // HTTP fetching disabled or unknown URL type
                                tracing::warn!(
                                    url = %image_url.url,
                                    "HTTP image URL fetching is disabled. Image will be skipped by provider."
                                );
                            }
                            Err(e) => {
                                tracing::warn!(
                                    url = %image_url.url,
                                    error = %e,
                                    "Failed to fetch HTTP image URL. Image will be skipped by provider."
                                );
                            }
                        }
                    }
                }
            }

            fetched_count
        }
    }
}

/// Preprocess a single image URL, returning the possibly modified URL.
///
/// This is useful for providers that need to handle images outside of the
/// standard message structure.
#[allow(dead_code)] // Will be used by Vertex provider in Sub-task 4
pub async fn preprocess_image_url(
    client: &Client,
    url: &str,
    config: Option<&ImageFetchConfig>,
) -> String {
    // Skip if already a data URL
    if url.starts_with("data:") {
        return url.to_string();
    }

    // Try to fetch HTTP URL
    if is_http_url(url) {
        match resolve_image_url(client, url, config).await {
            Ok(Some(data)) => {
                tracing::debug!(
                    original_url = %url,
                    media_type = %data.media_type,
                    "Converted HTTP image URL to data URL"
                );
                return to_data_url(&data);
            }
            Ok(None) => {
                tracing::warn!(
                    url = %url,
                    "HTTP image URL fetching is disabled. Image may be skipped by provider."
                );
            }
            Err(e) => {
                tracing::warn!(
                    url = %url,
                    error = %e,
                    "Failed to fetch HTTP image URL. Image may be skipped by provider."
                );
            }
        }
    }

    // Return original URL if we couldn't process it
    url.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_data_url_png() {
        let result = parse_data_url("data:image/png;base64,iVBORw0KGgo=");
        assert!(result.is_ok());
        let data = result.unwrap();
        assert_eq!(data.media_type, "image/png");
        assert_eq!(data.data, "iVBORw0KGgo=");
    }

    #[test]
    fn test_parse_data_url_jpeg() {
        let result = parse_data_url("data:image/jpeg;base64,/9j/4AAQ");
        assert!(result.is_ok());
        let data = result.unwrap();
        assert_eq!(data.media_type, "image/jpeg");
        assert_eq!(data.data, "/9j/4AAQ");
    }

    #[test]
    fn test_parse_data_url_gif() {
        let result = parse_data_url("data:image/gif;base64,R0lGODlh");
        assert!(result.is_ok());
        let data = result.unwrap();
        assert_eq!(data.media_type, "image/gif");
        assert_eq!(data.data, "R0lGODlh");
    }

    #[test]
    fn test_parse_data_url_webp() {
        let result = parse_data_url("data:image/webp;base64,UklGRg==");
        assert!(result.is_ok());
        let data = result.unwrap();
        assert_eq!(data.media_type, "image/webp");
        assert_eq!(data.data, "UklGRg==");
    }

    #[test]
    fn test_parse_data_url_missing_data_prefix() {
        let result = parse_data_url("image/png;base64,iVBORw0KGgo=");
        assert_eq!(result, Err(DataUrlParseError::MissingDataPrefix));
    }

    #[test]
    fn test_parse_data_url_missing_base64_marker() {
        let result = parse_data_url("data:image/png;utf8,hello");
        assert_eq!(result, Err(DataUrlParseError::MissingBase64Marker));
    }

    #[test]
    fn test_parse_data_url_http_url() {
        let result = parse_data_url("https://example.com/image.png");
        assert_eq!(result, Err(DataUrlParseError::MissingDataPrefix));
    }

    #[test]
    fn test_parse_data_url_empty() {
        let result = parse_data_url("");
        assert_eq!(result, Err(DataUrlParseError::MissingDataPrefix));
    }

    #[test]
    fn test_parse_data_url_missing_semicolon() {
        let result = parse_data_url("data:image/png,iVBORw0KGgo=");
        assert_eq!(result, Err(DataUrlParseError::MissingSemicolon));
    }

    #[test]
    fn test_parse_data_url_empty_media_type() {
        let result = parse_data_url("data:;base64,iVBORw0KGgo=");
        assert_eq!(result, Err(DataUrlParseError::EmptyMediaType));
    }

    #[test]
    fn test_parse_data_url_empty_data() {
        let result = parse_data_url("data:image/png;base64,");
        assert_eq!(result, Err(DataUrlParseError::EmptyData));
    }

    #[test]
    fn test_parse_data_url_error_messages() {
        // Verify error messages are descriptive
        let err = DataUrlParseError::MissingDataPrefix;
        assert!(err.to_string().contains("data:"));

        let err = DataUrlParseError::MissingSemicolon;
        assert!(err.to_string().contains("semicolon"));

        let err = DataUrlParseError::MissingBase64Marker;
        assert!(err.to_string().contains("base64"));

        let err = DataUrlParseError::EmptyMediaType;
        assert!(err.to_string().contains("media type"));

        let err = DataUrlParseError::EmptyData;
        assert!(err.to_string().contains("base64 data"));
    }

    #[test]
    fn test_is_http_url() {
        assert!(is_http_url("http://example.com/image.png"));
        assert!(is_http_url("https://example.com/image.png"));
        assert!(is_http_url(
            "https://example.com:8080/path/to/image.jpg?query=1"
        ));
        assert!(!is_http_url("data:image/png;base64,abc"));
        assert!(!is_http_url("ftp://example.com/image.png"));
        assert!(!is_http_url("/local/path/image.png"));
        assert!(!is_http_url(""));
    }

    #[test]
    fn test_image_fetch_config_default() {
        let config = ImageFetchConfig::default();
        assert!(config.enabled);
        assert_eq!(config.max_size_bytes, DEFAULT_MAX_IMAGE_SIZE_BYTES);
        assert_eq!(
            config.timeout,
            Duration::from_secs(DEFAULT_IMAGE_FETCH_TIMEOUT_SECS)
        );
        assert_eq!(config.allowed_content_types.len(), 4);
    }

    #[tokio::test]
    async fn test_resolve_data_url_no_http_config() {
        let client = Client::new();
        let result = resolve_image_url(&client, "data:image/png;base64,iVBORw0KGgo=", None)
            .await
            .unwrap();
        assert!(result.is_some());
        let data = result.unwrap();
        assert_eq!(data.media_type, "image/png");
        assert_eq!(data.data, "iVBORw0KGgo=");
    }

    #[tokio::test]
    async fn test_resolve_http_url_disabled() {
        let client = Client::new();
        // HTTP URL with no config = disabled
        let result = resolve_image_url(&client, "https://example.com/image.png", None)
            .await
            .unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_resolve_unknown_url() {
        let client = Client::new();
        let result = resolve_image_url(&client, "ftp://example.com/image.png", None)
            .await
            .unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_fetch_disabled_config() {
        let client = Client::new();
        let config = ImageFetchConfig {
            enabled: false,
            ..Default::default()
        };
        let result = fetch_image_url(&client, "https://example.com/image.png", &config).await;
        assert!(matches!(result, Err(ImageError::FetchingDisabled)));
    }

    #[tokio::test]
    async fn test_fetch_invalid_url() {
        let client = Client::new();
        let config = ImageFetchConfig::default();
        let result = fetch_image_url(&client, "not-a-url", &config).await;
        assert!(matches!(result, Err(ImageError::InvalidUrl(_))));
    }

    #[test]
    fn test_to_data_url() {
        let data = ImageData {
            media_type: "image/png".to_string(),
            data: "iVBORw0KGgo=".to_string(),
        };
        assert_eq!(to_data_url(&data), "data:image/png;base64,iVBORw0KGgo=");
    }

    #[tokio::test]
    async fn test_preprocess_image_url_data_url_passthrough() {
        let client = Client::new();
        let url = "data:image/png;base64,iVBORw0KGgo=";
        let result = preprocess_image_url(&client, url, None).await;
        assert_eq!(result, url);
    }

    #[tokio::test]
    async fn test_preprocess_image_url_http_disabled() {
        let client = Client::new();
        let url = "https://example.com/image.png";
        // With no config, HTTP fetching is disabled - returns original URL
        let result = preprocess_image_url(&client, url, None).await;
        assert_eq!(result, url);
    }

    #[tokio::test]
    async fn test_preprocess_messages_no_images() {
        let client = Client::new();
        let mut messages = vec![Message::User {
            content: MessageContent::Text("Hello".to_string()),
            name: None,
        }];

        let count = preprocess_messages_for_images(&client, &mut messages, None).await;
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_preprocess_messages_data_url_passthrough() {
        use crate::api_types::chat_completion::ImageUrl;

        let client = Client::new();
        let data_url = "data:image/png;base64,iVBORw0KGgo=";
        let mut messages = vec![Message::User {
            content: MessageContent::Parts(vec![ContentPart::ImageUrl {
                image_url: ImageUrl {
                    url: data_url.to_string(),
                    detail: None,
                },
                cache_control: None,
            }]),
            name: None,
        }];

        let count = preprocess_messages_for_images(&client, &mut messages, None).await;
        // Data URLs are not "fetched" - they're passed through
        assert_eq!(count, 0);

        // URL should be unchanged
        if let Message::User { content, .. } = &messages[0] {
            if let MessageContent::Parts(parts) = content {
                if let ContentPart::ImageUrl { image_url, .. } = &parts[0] {
                    assert_eq!(image_url.url, data_url);
                } else {
                    panic!("Expected ImageUrl");
                }
            } else {
                panic!("Expected Parts");
            }
        } else {
            panic!("Expected User message");
        }
    }
}
