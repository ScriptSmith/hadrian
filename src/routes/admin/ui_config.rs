use axum::{Json, extract::State};
use serde::Serialize;

use crate::{
    AppState,
    config::{
        AdminConfig, AuthMode, BrandingConfig, ChatConfig, ColorPalette, CustomFont, FontsConfig,
        LoginConfig, UiConfig,
    },
};

/// UI configuration response for frontend applications.
#[derive(Debug, Serialize)]
pub struct UiConfigResponse {
    pub branding: BrandingResponse,
    pub chat: ChatResponse,
    pub admin: AdminResponse,
    pub auth: AuthResponse,
}

#[derive(Debug, Serialize)]
pub struct BrandingResponse {
    pub title: String,
    pub tagline: Option<String>,
    pub logo_url: Option<String>,
    pub logo_dark_url: Option<String>,
    pub favicon_url: Option<String>,
    pub colors: ColorPaletteResponse,
    pub colors_dark: Option<ColorPaletteResponse>,
    pub fonts: Option<FontsResponse>,
    pub footer_text: Option<String>,
    pub footer_links: Vec<FooterLinkResponse>,
    pub show_version: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub login: Option<LoginResponse>,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background_image: Option<String>,
    pub show_logo: bool,
}

#[derive(Debug, Serialize, Default)]
pub struct ColorPaletteResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secondary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub foreground: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub muted: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct FooterLinkResponse {
    pub label: String,
    pub url: String,
}

#[derive(Debug, Serialize)]
pub struct FontsResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub heading: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mono: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub custom: Vec<CustomFontResponse>,
}

#[derive(Debug, Serialize)]
pub struct CustomFontResponse {
    pub name: String,
    pub url: String,
    pub weight: String,
    pub style: String,
}

impl From<&FontsConfig> for FontsResponse {
    fn from(config: &FontsConfig) -> Self {
        Self {
            heading: config.heading.clone(),
            body: config.body.clone(),
            mono: config.mono.clone(),
            custom: config.custom.iter().map(CustomFontResponse::from).collect(),
        }
    }
}

impl From<&CustomFont> for CustomFontResponse {
    fn from(font: &CustomFont) -> Self {
        Self {
            name: font.name.clone(),
            url: font.url.clone(),
            weight: font.weight.clone(),
            style: font.style.clone(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ChatResponse {
    pub enabled: bool,
    pub default_model: Option<String>,
    pub available_models: Vec<String>,
    pub file_uploads_enabled: bool,
    pub max_file_size_bytes: usize,
    pub allowed_file_types: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct AdminResponse {
    pub enabled: bool,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub methods: Vec<String>,
    pub oidc: Option<OidcResponse>,
}

#[derive(Debug, Serialize)]
pub struct OidcResponse {
    pub provider: String,
    pub authorization_url: String,
    pub client_id: String,
}

impl From<&UiConfig> for UiConfigResponse {
    fn from(config: &UiConfig) -> Self {
        Self {
            branding: BrandingResponse::from(&config.branding),
            chat: ChatResponse::from(&config.chat),
            admin: AdminResponse::from(&config.admin),
            auth: AuthResponse::default(),
        }
    }
}

impl From<&BrandingConfig> for BrandingResponse {
    fn from(config: &BrandingConfig) -> Self {
        Self {
            title: config
                .title
                .clone()
                .unwrap_or_else(|| "Hadrian Gateway".to_string()),
            tagline: config.tagline.clone(),
            logo_url: config.logo_url.clone(),
            logo_dark_url: config.logo_dark_url.clone(),
            favicon_url: config.favicon_url.clone(),
            colors: config
                .colors
                .as_ref()
                .map(ColorPaletteResponse::from)
                .unwrap_or_default(),
            colors_dark: config.colors_dark.as_ref().map(ColorPaletteResponse::from),
            fonts: config.fonts.as_ref().map(FontsResponse::from),
            footer_text: config.footer_text.clone(),
            footer_links: config
                .footer_links
                .iter()
                .map(|l| FooterLinkResponse {
                    label: l.label.clone(),
                    url: l.url.clone(),
                })
                .collect(),
            show_version: config.show_version,
            version: if config.show_version {
                Some(env!("CARGO_PKG_VERSION").to_string())
            } else {
                None
            },
            login: config.login.as_ref().map(LoginResponse::from),
        }
    }
}

impl From<&LoginConfig> for LoginResponse {
    fn from(config: &LoginConfig) -> Self {
        Self {
            title: config.title.clone(),
            subtitle: config.subtitle.clone(),
            background_image: config.background_image.clone(),
            show_logo: config.show_logo,
        }
    }
}

impl From<&ColorPalette> for ColorPaletteResponse {
    fn from(config: &ColorPalette) -> Self {
        Self {
            primary: config.primary.clone(),
            secondary: config.secondary.clone(),
            accent: config.accent.clone(),
            background: config.background.clone(),
            foreground: config.foreground.clone(),
            muted: config.muted.clone(),
            border: config.border.clone(),
        }
    }
}

impl From<&ChatConfig> for ChatResponse {
    fn from(config: &ChatConfig) -> Self {
        Self {
            enabled: config.enabled,
            default_model: config.default_model.clone(),
            available_models: config.available_models.clone(),
            file_uploads_enabled: config.file_uploads.enabled,
            max_file_size_bytes: config.file_uploads.max_size_bytes,
            allowed_file_types: config.file_uploads.allowed_types.clone(),
        }
    }
}

impl From<&AdminConfig> for AdminResponse {
    fn from(config: &AdminConfig) -> Self {
        Self {
            enabled: config.enabled,
        }
    }
}

impl Default for AuthResponse {
    fn default() -> Self {
        Self {
            methods: vec!["api_key".to_string()],
            oidc: None,
        }
    }
}

/// Get UI configuration for frontend applications.
/// This endpoint is unauthenticated so the UI can fetch it before login.
pub async fn get_ui_config(State(state): State<AppState>) -> Json<UiConfigResponse> {
    let ui_config = &state.config.ui;
    let mut response = UiConfigResponse::from(ui_config);

    // Add auth methods based on configuration
    let mut auth_methods = Vec::new();

    // Add auth methods based on the configured auth mode
    match &state.config.auth.mode {
        AuthMode::None => {
            // No auth - fall through to "none" below
        }
        AuthMode::ApiKey => {
            // API key mode - offer API key login for admin panel
            auth_methods.push("api_key".to_string());
        }
        #[cfg(feature = "sso")]
        AuthMode::Idp => {
            // IdP mode - users authenticate via per-org SSO
            // The frontend should show email discovery to determine which org's IdP to use
            auth_methods.push("session".to_string());
        }
        AuthMode::Iap(_) => {
            // IAP mode - reverse proxy handles auth
            auth_methods.push("header".to_string());
        }
    }

    // Check if any per-org SSO configurations exist (for SAML or per-org OIDC)
    // This enables email discovery on the login page even when no global OIDC is configured
    #[cfg(feature = "sso")]
    if let Some(ref services) = state.services
        && services
            .org_sso_configs
            .any_enabled()
            .await
            .unwrap_or(false)
    {
        auth_methods.push("per_org_sso".to_string());
    }

    // If no auth is configured at all, allow unauthenticated access
    if auth_methods.is_empty() {
        auth_methods.push("none".to_string());
    }

    response.auth.methods = auth_methods;

    Json(response)
}
