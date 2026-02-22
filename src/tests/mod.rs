//! Consolidated test modules.
//!
//! This module contains end-to-end tests that are parameterized across providers.

#[cfg(all(test, feature = "database-sqlite"))]
mod provider_e2e;
