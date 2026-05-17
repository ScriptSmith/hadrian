//! Passthrough runtime: forward the shell tool spec to the upstream
//! provider unchanged.
//!
//! Used when admin config sets `mode = "passthrough"` and the upstream
//! provider hosts its own shell-execution environment (currently
//! OpenAI's GPT-5.2+ models with the `shell` tool type).
//!
//! The runtime never actually runs commands locally — `start_session`
//! returns `RuntimeError::Passthrough` so the orchestrator skips local
//! execution and the upstream provider's response (which contains the
//! shell call events the model produced) flows through unmodified.

use async_trait::async_trait;

use super::{
    RuntimeCapabilities, RuntimeError, RuntimeResult, SessionHandle, SessionSpec, ShellRuntime,
};

/// Pass-through runtime for upstream providers that host their own
/// shell execution environment.
#[derive(Debug, Default, Clone, Copy)]
pub struct PassthroughRuntime;

impl PassthroughRuntime {
    pub const fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ShellRuntime for PassthroughRuntime {
    fn capabilities(&self) -> RuntimeCapabilities {
        RuntimeCapabilities::passthrough()
    }

    async fn start_session(&self, _spec: SessionSpec) -> RuntimeResult<SessionHandle> {
        Err(RuntimeError::Passthrough)
    }
}
