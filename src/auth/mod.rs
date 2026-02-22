mod error;
mod identity;
pub mod jwt;
#[cfg(feature = "sso")]
pub mod oidc;
mod principal;
#[cfg(feature = "sso")]
mod registry;
#[cfg(feature = "saml")]
pub mod saml;
#[cfg(feature = "saml")]
mod saml_registry;
#[cfg(feature = "sso")]
pub mod session_store;

pub use error::AuthError;
pub use identity::{ApiKeyAuth, AuthenticatedRequest, Identity, IdentityKind};
#[cfg(feature = "sso")]
pub use oidc::OidcAuthenticator;
#[cfg(feature = "sso")]
pub use registry::OidcAuthenticatorRegistry;
#[cfg(feature = "saml")]
pub use saml::derive_acs_url_from_entity_id;
#[cfg(feature = "saml")]
pub use saml_registry::SamlAuthenticatorRegistry;
#[cfg(feature = "sso")]
pub use session_store::create_session_store_with_enhanced;
