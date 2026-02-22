use serde::{Deserialize, Serialize};

/// UI configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct UiConfig {
    /// Enable the UI.
    #[serde(default)]
    pub enabled: bool,

    /// Path to serve the UI from (default: /).
    #[serde(default = "default_ui_path")]
    pub path: String,

    /// Static assets configuration.
    #[serde(default)]
    pub assets: AssetsConfig,

    /// Chat interface configuration.
    #[serde(default)]
    pub chat: ChatConfig,

    /// Admin panel configuration.
    #[serde(default)]
    pub admin: AdminConfig,

    /// Branding customization.
    #[serde(default)]
    pub branding: BrandingConfig,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            path: default_ui_path(),
            assets: AssetsConfig::default(),
            chat: ChatConfig::default(),
            admin: AdminConfig::default(),
            branding: BrandingConfig::default(),
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_ui_path() -> String {
    "/".to_string()
}

/// Static assets configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct AssetsConfig {
    /// Source of static assets.
    #[serde(default)]
    pub source: AssetSource,

    /// Cache control header for static assets.
    #[serde(default = "default_cache_control")]
    pub cache_control: String,

    /// Enable gzip compression.
    #[serde(default = "default_true")]
    pub gzip: bool,

    /// Enable brotli compression.
    #[serde(default = "default_true")]
    pub brotli: bool,
}

impl Default for AssetsConfig {
    fn default() -> Self {
        Self {
            source: AssetSource::default(),
            cache_control: default_cache_control(),
            gzip: true,
            brotli: true,
        }
    }
}

fn default_cache_control() -> String {
    "public, max-age=31536000, immutable".to_string()
}

/// Source for static assets.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "type", rename_all = "snake_case")]
#[serde(deny_unknown_fields)]
pub enum AssetSource {
    /// Assets embedded in the binary.
    #[default]
    Embedded,

    /// Assets served from the filesystem.
    Filesystem { path: String },

    /// Assets served from a CDN (UI makes requests directly).
    Cdn { base_url: String },
}

/// Chat interface configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ChatConfig {
    /// Enable chat interface.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Default model for new chats.
    #[serde(default)]
    pub default_model: Option<String>,

    /// Available models in the UI (if empty, all models are shown).
    #[serde(default)]
    pub available_models: Vec<String>,

    /// Enable conversation history.
    #[serde(default = "default_true")]
    pub history_enabled: bool,

    /// Maximum conversations to store per user.
    #[serde(default = "default_max_conversations")]
    pub max_conversations: usize,

    /// Enable file uploads.
    #[serde(default)]
    pub file_uploads: FileUploadConfig,

    /// Enable code execution in chat.
    #[serde(default)]
    pub code_execution: bool,

    /// Enable web search in chat.
    #[serde(default)]
    pub web_search: bool,
}

impl Default for ChatConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            default_model: None,
            available_models: vec![],
            history_enabled: true,
            max_conversations: default_max_conversations(),
            file_uploads: FileUploadConfig::default(),
            code_execution: false,
            web_search: false,
        }
    }
}

fn default_max_conversations() -> usize {
    100
}

/// File upload configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FileUploadConfig {
    /// Enable file uploads.
    #[serde(default)]
    pub enabled: bool,

    /// Maximum file size in bytes.
    #[serde(default = "default_max_file_size")]
    pub max_size_bytes: usize,

    /// Allowed MIME types.
    #[serde(default = "default_allowed_types")]
    pub allowed_types: Vec<String>,

    /// Storage backend for uploaded files.
    #[serde(default)]
    pub storage: UploadStorageConfig,
}

impl Default for FileUploadConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_size_bytes: default_max_file_size(),
            allowed_types: default_allowed_types(),
            storage: UploadStorageConfig::default(),
        }
    }
}

fn default_max_file_size() -> usize {
    10 * 1024 * 1024 // 10 MB
}

fn default_allowed_types() -> Vec<String> {
    vec![
        "image/png".into(),
        "image/jpeg".into(),
        "image/gif".into(),
        "image/webp".into(),
        "application/pdf".into(),
        "text/plain".into(),
        "text/markdown".into(),
    ]
}

/// Storage backend for chat file uploads.
///
/// Note: For the Files API storage backend, see `FileStorageConfig` in `storage.rs`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "type", rename_all = "snake_case")]
#[serde(deny_unknown_fields)]
pub enum UploadStorageConfig {
    /// Store in database (for small files).
    #[default]
    Database,

    /// Store on local filesystem.
    Filesystem { path: String },

    /// Store in S3-compatible storage.
    S3 {
        bucket: String,
        #[serde(default)]
        region: Option<String>,
        #[serde(default)]
        endpoint: Option<String>,
        #[serde(default)]
        prefix: Option<String>,
    },
}

/// Admin panel configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct AdminConfig {
    /// Enable admin panel.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Path for admin panel.
    #[serde(default = "default_admin_path")]
    pub path: String,

    /// Roles that can access the admin panel.
    /// If empty, any authenticated user with admin flag can access.
    #[serde(default)]
    pub allowed_roles: Vec<String>,
}

impl Default for AdminConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            path: default_admin_path(),
            allowed_roles: vec![],
        }
    }
}

fn default_admin_path() -> String {
    "/admin".to_string()
}

/// Branding customization.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct BrandingConfig {
    /// Application title.
    #[serde(default)]
    pub title: Option<String>,

    /// Tagline shown below the title (e.g., "Powering research with AI").
    #[serde(default)]
    pub tagline: Option<String>,

    /// Logo URL.
    #[serde(default)]
    pub logo_url: Option<String>,

    /// Logo URL for dark mode. Falls back to logo_url if not specified.
    #[serde(default)]
    pub logo_dark_url: Option<String>,

    /// Favicon URL.
    #[serde(default)]
    pub favicon_url: Option<String>,

    /// Color palette for light mode.
    #[serde(default)]
    pub colors: Option<ColorPalette>,

    /// Color palette overrides for dark mode.
    #[serde(default)]
    pub colors_dark: Option<ColorPalette>,

    /// Typography configuration.
    #[serde(default)]
    pub fonts: Option<FontsConfig>,

    /// Custom CSS URL.
    #[serde(default)]
    pub custom_css_url: Option<String>,

    /// Footer text.
    #[serde(default)]
    pub footer_text: Option<String>,

    /// Footer links.
    #[serde(default)]
    pub footer_links: Vec<FooterLink>,

    /// Show version in footer.
    #[serde(default)]
    pub show_version: bool,

    /// Login page customization.
    #[serde(default)]
    pub login: Option<LoginConfig>,
}

/// Color palette for branding customization.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ColorPalette {
    /// Primary brand color (hex, e.g., "#3b82f6").
    #[serde(default)]
    pub primary: Option<String>,

    /// Text color on primary backgrounds (hex, e.g., "#ffffff").
    /// Used for text on primary buttons like "New Chat".
    #[serde(default)]
    pub primary_foreground: Option<String>,

    /// Secondary color for secondary actions (hex).
    #[serde(default)]
    pub secondary: Option<String>,

    /// Text color on secondary backgrounds (hex).
    #[serde(default)]
    pub secondary_foreground: Option<String>,

    /// Accent color for highlights and CTAs (hex).
    #[serde(default)]
    pub accent: Option<String>,

    /// Background color (hex).
    #[serde(default)]
    pub background: Option<String>,

    /// Foreground/text color (hex).
    #[serde(default)]
    pub foreground: Option<String>,

    /// Muted color for subtle backgrounds (hex).
    #[serde(default)]
    pub muted: Option<String>,

    /// Border color (hex).
    #[serde(default)]
    pub border: Option<String>,
}

/// Typography/font configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FontsConfig {
    /// Font family for headings (e.g., "Inter", "Roboto").
    #[serde(default)]
    pub heading: Option<String>,

    /// Font family for body text (e.g., "Inter", "Roboto").
    #[serde(default)]
    pub body: Option<String>,

    /// Font family for monospace/code text (e.g., "JetBrains Mono", "Fira Code").
    #[serde(default)]
    pub mono: Option<String>,

    /// Custom fonts to load via @font-face.
    #[serde(default)]
    pub custom: Vec<CustomFont>,
}

/// Custom font definition for loading external fonts.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CustomFont {
    /// Font family name to use in CSS.
    pub name: String,

    /// URL to the font file (woff2, woff, ttf, otf).
    pub url: String,

    /// Font weight (e.g., "400", "700", "100 900" for variable fonts).
    #[serde(default = "default_font_weight")]
    pub weight: String,

    /// Font style ("normal" or "italic").
    #[serde(default = "default_font_style")]
    pub style: String,
}

fn default_font_weight() -> String {
    "400".to_string()
}

fn default_font_style() -> String {
    "normal".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FooterLink {
    pub label: String,
    pub url: String,
}

/// Login page customization.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct LoginConfig {
    /// Custom title for the login page (e.g., "Sign in to AI Gateway").
    #[serde(default)]
    pub title: Option<String>,

    /// Subtitle shown below the title (e.g., "Use your university credentials").
    #[serde(default)]
    pub subtitle: Option<String>,

    /// Background image URL for the login page.
    #[serde(default)]
    pub background_image: Option<String>,

    /// Whether to show the logo on the login page (defaults to true).
    #[serde(default = "default_true")]
    pub show_logo: bool,
}
