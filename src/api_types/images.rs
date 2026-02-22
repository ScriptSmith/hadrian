#![allow(dead_code)]

//! OpenAI-compatible image generation API types.
//!
//! Types for the image generation endpoints:
//! - POST /v1/images/generations - Generate images from text
//! - POST /v1/images/edits - Edit images with text instructions
//! - POST /v1/images/variations - Create image variations

use serde::{Deserialize, Serialize};
use validator::Validate;

/// Image generation model
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "kebab-case")]
pub enum ImageModel {
    #[serde(rename = "dall-e-2")]
    DallE2,
    #[serde(rename = "dall-e-3")]
    DallE3,
    #[serde(rename = "gpt-image-1")]
    GptImage1,
    #[serde(rename = "gpt-image-1-mini")]
    GptImage1Mini,
    #[serde(rename = "gpt-image-1.5")]
    GptImage15,
}

/// Image quality setting
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum ImageQuality {
    /// Standard quality (dall-e-2, dall-e-3)
    Standard,
    /// HD quality (dall-e-3)
    Hd,
    /// Low quality (GPT image models)
    Low,
    /// Medium quality (GPT image models)
    Medium,
    /// High quality (GPT image models)
    High,
    /// Auto-select best quality for model (default)
    #[default]
    Auto,
}

/// Image response format
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum ImageResponseFormat {
    /// Return URL (valid for 60 minutes)
    #[default]
    Url,
    /// Return base64-encoded JSON
    B64Json,
}

/// Image output format (GPT image models only)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum ImageOutputFormat {
    #[default]
    Png,
    Jpeg,
    Webp,
}

/// Image size for generation
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub enum ImageSize {
    /// Auto-select (GPT image models)
    #[serde(rename = "auto")]
    Auto,
    /// 256x256 (dall-e-2)
    #[serde(rename = "256x256")]
    Size256,
    /// 512x512 (dall-e-2)
    #[serde(rename = "512x512")]
    Size512,
    /// 1024x1024 (all models)
    #[default]
    #[serde(rename = "1024x1024")]
    Size1024,
    /// 1536x1024 landscape (GPT image models)
    #[serde(rename = "1536x1024")]
    Size1536x1024,
    /// 1024x1536 portrait (GPT image models)
    #[serde(rename = "1024x1536")]
    Size1024x1536,
    /// 1792x1024 landscape (dall-e-3)
    #[serde(rename = "1792x1024")]
    Size1792x1024,
    /// 1024x1792 portrait (dall-e-3)
    #[serde(rename = "1024x1792")]
    Size1024x1792,
}

/// Image style (dall-e-3 only)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum ImageStyle {
    /// Hyper-real and dramatic images
    #[default]
    Vivid,
    /// More natural, less hyper-real images
    Natural,
}

/// Image background transparency (GPT image models only)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum ImageBackground {
    /// Transparent background (requires png or webp output)
    Transparent,
    /// Opaque background
    Opaque,
    /// Auto-detect best background
    #[default]
    Auto,
}

/// Content moderation level (GPT image models only)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum ImageModeration {
    /// Less restrictive filtering
    Low,
    /// Default moderation
    #[default]
    Auto,
}

/// Partial images configuration for streaming
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct PartialImages {
    /// Number of partial images to generate during streaming
    #[serde(skip_serializing_if = "Option::is_none")]
    pub partial_image_count: Option<i32>,
}

/// Create image generation request (POST /v1/images/generations)
#[derive(Debug, Clone, Validate, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CreateImageRequest {
    /// A text description of the desired image(s).
    /// Max 32000 chars for GPT image models, 1000 for dall-e-2, 4000 for dall-e-3.
    pub prompt: String,

    /// The model to use for image generation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    /// Number of images to generate (1-10). For dall-e-3, only n=1 is supported.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[validate(range(min = 1, max = 10))]
    pub n: Option<i32>,

    /// The quality of the generated image.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quality: Option<ImageQuality>,

    /// Format for returning generated images (url or b64_json).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<ImageResponseFormat>,

    /// Output format for GPT image models (png, jpeg, webp).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_format: Option<ImageOutputFormat>,

    /// Compression level (0-100%) for webp/jpeg output. GPT image models only.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[validate(range(min = 0, max = 100))]
    pub output_compression: Option<i32>,

    /// Generate image in streaming mode. GPT image models only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,

    /// Partial images configuration for streaming.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub partial_images: Option<PartialImages>,

    /// Size of the generated images.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<ImageSize>,

    /// Content moderation level. GPT image models only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub moderation: Option<ImageModeration>,

    /// Background transparency setting. GPT image models only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<ImageBackground>,

    /// Image style. dall-e-3 only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<ImageStyle>,

    /// Unique identifier for the end-user for abuse detection.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
}

/// Create image edit request (POST /v1/images/edits)
///
/// Note: This endpoint accepts multipart/form-data. The `image` and `mask`
/// fields are file uploads, not JSON fields.
#[derive(Debug, Clone, Validate, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CreateImageEditRequest {
    /// A text description of the desired edit.
    /// Max 1000 chars for dall-e-2, 32000 for GPT image models.
    pub prompt: String,

    /// The model to use for image editing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    /// Number of images to generate (1-10).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[validate(range(min = 1, max = 10))]
    pub n: Option<i32>,

    /// Size of the generated images.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<ImageSize>,

    /// Format for returning generated images (url or b64_json).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<ImageResponseFormat>,

    /// Output format for GPT image models (png, jpeg, webp).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_format: Option<ImageOutputFormat>,

    /// Compression level (0-100%) for webp/jpeg output.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[validate(range(min = 0, max = 100))]
    pub output_compression: Option<i32>,

    /// Background transparency setting. GPT image models only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<ImageBackground>,

    /// The quality of the generated image.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quality: Option<ImageQuality>,

    /// Edit image in streaming mode. GPT image models only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,

    /// Partial images configuration for streaming.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub partial_images: Option<PartialImages>,

    /// Unique identifier for the end-user for abuse detection.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
}

/// Create image variation request (POST /v1/images/variations)
///
/// Note: This endpoint accepts multipart/form-data. The `image` field
/// is a file upload, not a JSON field.
#[derive(Debug, Clone, Validate, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CreateImageVariationRequest {
    /// The model to use. Only dall-e-2 is supported for variations.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    /// Number of images to generate (1-10).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[validate(range(min = 1, max = 10))]
    pub n: Option<i32>,

    /// Format for returning generated images (url or b64_json).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<ImageResponseFormat>,

    /// Size of the generated images.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<ImageSize>,

    /// Unique identifier for the end-user for abuse detection.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
}

/// Individual image data in response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct Image {
    /// Base64-encoded image data (when response_format is b64_json)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub b64_json: Option<String>,

    /// URL of the generated image (when response_format is url)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,

    /// The revised prompt used for generation (dall-e-3 only)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revised_prompt: Option<String>,
}

/// Token usage details for image input
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ImageInputTokensDetails {
    /// Number of text tokens in input
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_tokens: Option<i64>,

    /// Number of image tokens in input
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_tokens: Option<i64>,
}

/// Token usage for image generation (GPT image models only)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ImageUsage {
    /// Total tokens used
    pub total_tokens: i64,

    /// Input tokens used
    pub input_tokens: i64,

    /// Output tokens used
    pub output_tokens: i64,

    /// Breakdown of input token types
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_tokens_details: Option<ImageInputTokensDetails>,
}

/// Response from image generation endpoints
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ImagesResponse {
    /// Unix timestamp (seconds) when images were created
    pub created: i64,

    /// List of generated images
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Image>>,

    /// Background used (transparent or opaque)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,

    /// Output format used (png, webp, jpeg)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_format: Option<String>,

    /// Size of generated images
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,

    /// Quality of generated images
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quality: Option<String>,

    /// Token usage (GPT image models only)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<ImageUsage>,
}

impl CreateImageRequest {
    /// Strip parameters unsupported by the target model family.
    ///
    /// GPT-image models reject `response_format` and `style`;
    /// DALL-E models reject `output_format`, `output_compression`, `background`,
    /// `moderation`, `partial_images`, and `stream`.
    /// Unknown or missing families pass through unchanged.
    pub fn normalize_for_family(&mut self, family: Option<&str>) {
        match family {
            Some("gpt-image") => {
                self.response_format = None;
                self.style = None;
            }
            Some("dall-e") => {
                self.output_format = None;
                self.output_compression = None;
                self.background = None;
                self.moderation = None;
                self.partial_images = None;
                self.stream = None;
            }
            _ => {}
        }
    }
}

impl CreateImageEditRequest {
    /// Strip parameters unsupported by the target model family.
    ///
    /// See [`CreateImageRequest::normalize_for_family`] for details.
    pub fn normalize_for_family(&mut self, family: Option<&str>) {
        match family {
            Some("gpt-image") => {
                self.response_format = None;
            }
            Some("dall-e") => {
                self.output_format = None;
                self.output_compression = None;
                self.background = None;
                self.partial_images = None;
                self.stream = None;
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_image_request_serialization() {
        let request = CreateImageRequest {
            prompt: "A cute baby sea otter".to_string(),
            model: Some("dall-e-3".to_string()),
            n: Some(1),
            quality: Some(ImageQuality::Hd),
            response_format: Some(ImageResponseFormat::Url),
            output_format: None,
            output_compression: None,
            stream: None,
            partial_images: None,
            size: Some(ImageSize::Size1024),
            moderation: None,
            background: None,
            style: Some(ImageStyle::Vivid),
            user: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"prompt\":\"A cute baby sea otter\""));
        assert!(json.contains("\"model\":\"dall-e-3\""));
        assert!(json.contains("\"quality\":\"hd\""));
        assert!(json.contains("\"size\":\"1024x1024\""));
        assert!(json.contains("\"style\":\"vivid\""));
    }

    #[test]
    fn test_create_image_request_deserialization() {
        let json = r#"{
            "prompt": "A painting of a sunset",
            "model": "dall-e-2",
            "n": 2,
            "size": "512x512",
            "response_format": "b64_json"
        }"#;

        let request: CreateImageRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.prompt, "A painting of a sunset");
        assert_eq!(request.model, Some("dall-e-2".to_string()));
        assert_eq!(request.n, Some(2));
        assert_eq!(request.size, Some(ImageSize::Size512));
        assert_eq!(request.response_format, Some(ImageResponseFormat::B64Json));
    }

    #[test]
    fn test_images_response_serialization() {
        let response = ImagesResponse {
            created: 1713833628,
            data: Some(vec![Image {
                b64_json: Some("base64data...".to_string()),
                url: None,
                revised_prompt: Some("A cute otter swimming".to_string()),
            }]),
            background: Some("transparent".to_string()),
            output_format: Some("png".to_string()),
            size: Some("1024x1024".to_string()),
            quality: Some("high".to_string()),
            usage: Some(ImageUsage {
                total_tokens: 100,
                input_tokens: 50,
                output_tokens: 50,
                input_tokens_details: Some(ImageInputTokensDetails {
                    text_tokens: Some(10),
                    image_tokens: Some(40),
                }),
            }),
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"created\":1713833628"));
        assert!(json.contains("\"b64_json\":\"base64data...\""));
        assert!(json.contains("\"total_tokens\":100"));
    }

    #[test]
    fn test_image_quality_enum() {
        assert_eq!(serde_json::to_string(&ImageQuality::Hd).unwrap(), "\"hd\"");
        assert_eq!(
            serde_json::to_string(&ImageQuality::Standard).unwrap(),
            "\"standard\""
        );
        assert_eq!(
            serde_json::to_string(&ImageQuality::Auto).unwrap(),
            "\"auto\""
        );
    }

    #[test]
    fn test_image_size_enum() {
        assert_eq!(
            serde_json::to_string(&ImageSize::Size256).unwrap(),
            "\"256x256\""
        );
        assert_eq!(
            serde_json::to_string(&ImageSize::Size1792x1024).unwrap(),
            "\"1792x1024\""
        );
        assert_eq!(serde_json::to_string(&ImageSize::Auto).unwrap(), "\"auto\"");
    }

    #[test]
    fn test_image_model_enum() {
        assert_eq!(
            serde_json::to_string(&ImageModel::DallE2).unwrap(),
            "\"dall-e-2\""
        );
        assert_eq!(
            serde_json::to_string(&ImageModel::DallE3).unwrap(),
            "\"dall-e-3\""
        );
        assert_eq!(
            serde_json::to_string(&ImageModel::GptImage15).unwrap(),
            "\"gpt-image-1.5\""
        );
    }

    #[test]
    fn test_minimal_create_image_request() {
        let json = r#"{"prompt": "A red apple"}"#;
        let request: CreateImageRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.prompt, "A red apple");
        assert!(request.model.is_none());
        assert!(request.n.is_none());
    }

    fn full_create_image_request() -> CreateImageRequest {
        CreateImageRequest {
            prompt: "test".to_string(),
            model: Some("gpt-image-1".to_string()),
            n: Some(1),
            quality: Some(ImageQuality::High),
            response_format: Some(ImageResponseFormat::B64Json),
            output_format: Some(ImageOutputFormat::Png),
            output_compression: Some(80),
            stream: Some(true),
            partial_images: Some(PartialImages {
                partial_image_count: Some(2),
            }),
            size: Some(ImageSize::Size1024),
            moderation: Some(ImageModeration::Low),
            background: Some(ImageBackground::Transparent),
            style: Some(ImageStyle::Vivid),
            user: Some("user-1".to_string()),
        }
    }

    #[test]
    fn test_normalize_gpt_image_strips_dalle_only_fields() {
        let mut req = full_create_image_request();
        req.normalize_for_family(Some("gpt-image"));

        // Stripped for gpt-image
        assert!(req.response_format.is_none());
        assert!(req.style.is_none());

        // Preserved
        assert!(req.output_format.is_some());
        assert!(req.output_compression.is_some());
        assert!(req.background.is_some());
        assert!(req.moderation.is_some());
        assert!(req.partial_images.is_some());
        assert!(req.stream.is_some());
        assert!(req.quality.is_some());
    }

    #[test]
    fn test_normalize_dalle_strips_gpt_image_only_fields() {
        let mut req = full_create_image_request();
        req.normalize_for_family(Some("dall-e"));

        // Stripped for dall-e
        assert!(req.output_format.is_none());
        assert!(req.output_compression.is_none());
        assert!(req.background.is_none());
        assert!(req.moderation.is_none());
        assert!(req.partial_images.is_none());
        assert!(req.stream.is_none());

        // Preserved
        assert!(req.response_format.is_some());
        assert!(req.style.is_some());
        assert!(req.quality.is_some());
    }

    #[test]
    fn test_normalize_unknown_family_preserves_all() {
        let mut req = full_create_image_request();
        req.normalize_for_family(Some("custom-model"));

        assert!(req.response_format.is_some());
        assert!(req.style.is_some());
        assert!(req.output_format.is_some());
        assert!(req.output_compression.is_some());
        assert!(req.background.is_some());
        assert!(req.moderation.is_some());
        assert!(req.partial_images.is_some());
        assert!(req.stream.is_some());
    }

    #[test]
    fn test_normalize_none_family_preserves_all() {
        let mut req = full_create_image_request();
        req.normalize_for_family(None);

        assert!(req.response_format.is_some());
        assert!(req.style.is_some());
        assert!(req.output_format.is_some());
    }

    #[test]
    fn test_normalize_gpt_image_serializes_without_stripped_fields() {
        let mut req = full_create_image_request();
        req.normalize_for_family(Some("gpt-image"));

        let json = serde_json::to_string(&req).unwrap();
        assert!(!json.contains("response_format"));
        assert!(!json.contains("style"));
        assert!(json.contains("output_format"));
        assert!(json.contains("background"));
    }

    #[test]
    fn test_normalize_edit_request_gpt_image() {
        let mut req = CreateImageEditRequest {
            prompt: "test".to_string(),
            model: Some("gpt-image-1".to_string()),
            n: Some(1),
            size: Some(ImageSize::Size1024),
            response_format: Some(ImageResponseFormat::B64Json),
            output_format: Some(ImageOutputFormat::Png),
            output_compression: Some(80),
            background: Some(ImageBackground::Transparent),
            quality: Some(ImageQuality::High),
            stream: Some(true),
            partial_images: Some(PartialImages {
                partial_image_count: Some(2),
            }),
            user: None,
        };
        req.normalize_for_family(Some("gpt-image"));

        assert!(req.response_format.is_none());
        assert!(req.output_format.is_some());
        assert!(req.background.is_some());
    }

    #[test]
    fn test_normalize_edit_request_dalle() {
        let mut req = CreateImageEditRequest {
            prompt: "test".to_string(),
            model: Some("dall-e-2".to_string()),
            n: Some(1),
            size: Some(ImageSize::Size512),
            response_format: Some(ImageResponseFormat::B64Json),
            output_format: Some(ImageOutputFormat::Png),
            output_compression: Some(80),
            background: Some(ImageBackground::Transparent),
            quality: Some(ImageQuality::Standard),
            stream: Some(true),
            partial_images: Some(PartialImages {
                partial_image_count: Some(2),
            }),
            user: None,
        };
        req.normalize_for_family(Some("dall-e"));

        assert!(req.response_format.is_some());
        assert!(req.output_format.is_none());
        assert!(req.output_compression.is_none());
        assert!(req.background.is_none());
        assert!(req.partial_images.is_none());
        assert!(req.stream.is_none());
        assert!(req.quality.is_some());
    }
}
