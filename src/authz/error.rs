//! Authorization errors.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuthzError {
    #[error("Access denied: {0}")]
    AccessDenied(String),

    #[error("Policy evaluation error: {0}")]
    PolicyEvaluation(String),

    #[error("Invalid CEL expression: {0}")]
    InvalidExpression(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl AuthzError {
    pub fn access_denied(reason: impl Into<String>) -> Self {
        Self::AccessDenied(reason.into())
    }
}
