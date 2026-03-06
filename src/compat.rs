//! Compatibility layer for concurrency primitives.
//!
//! On native builds with the `concurrency` feature, this re-exports high-performance
//! types from `parking_lot` and `dashmap`. On WASM or builds without `concurrency`,
//! it provides std-based fallbacks that are safe on single-threaded runtimes.
//!
//! ## Async trait Send bounds
//!
//! WASM is single-threaded, so `Send` bounds on async trait futures are unnecessary
//! and impossible to satisfy (reqwest/wasm-bindgen futures are `!Send`).
//!
//! All `#[async_trait]` usages are replaced with conditional `cfg_attr`:
//! ```ignore
//! #[cfg_attr(not(target_arch = "wasm32"), async_trait)]
//! #[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
//! ```
//! This is behavior-identical on native, and uses `?Send` on wasm32.

// ─────────────────────────────────────────────────────────────────────────────
// Spawn (fire-and-forget)
// ─────────────────────────────────────────────────────────────────────────────

/// Spawn a fire-and-forget async task.
///
/// On native builds this delegates to `tokio::spawn` (requires `Send`).
/// On WASM builds this uses `wasm_bindgen_futures::spawn_local` (no `Send` required).
#[cfg(not(target_arch = "wasm32"))]
pub fn spawn_detached<F>(future: F)
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    tokio::spawn(future);
}

/// Spawn a fire-and-forget async task (WASM version, no `Send` bound).
#[cfg(target_arch = "wasm32")]
pub fn spawn_detached<F>(future: F)
where
    F: std::future::Future<Output = ()> + 'static,
{
    wasm_bindgen_futures::spawn_local(future);
}

// ─────────────────────────────────────────────────────────────────────────────
// Mutex
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(feature = "concurrency")]
pub use parking_lot::Mutex;

/// A Mutex wrapper around `std::sync::Mutex` that panics-on-poison (matching
/// `parking_lot::Mutex` semantics). Safe on single-threaded WASM.
#[cfg(not(feature = "concurrency"))]
#[derive(Debug)]
pub struct Mutex<T>(std::sync::Mutex<T>);

#[cfg(not(feature = "concurrency"))]
impl<T> Mutex<T> {
    pub fn new(value: T) -> Self {
        Self(std::sync::Mutex::new(value))
    }

    pub fn lock(&self) -> std::sync::MutexGuard<'_, T> {
        self.0.lock().expect("mutex poisoned")
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// RwLock
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(feature = "concurrency")]
pub use parking_lot::RwLock;

/// An RwLock wrapper around `std::sync::RwLock` that panics-on-poison.
#[cfg(not(feature = "concurrency"))]
#[derive(Debug, Default)]
pub struct RwLock<T>(std::sync::RwLock<T>);

#[cfg(not(feature = "concurrency"))]
impl<T> RwLock<T> {
    pub fn new(value: T) -> Self {
        Self(std::sync::RwLock::new(value))
    }

    pub fn read(&self) -> std::sync::RwLockReadGuard<'_, T> {
        self.0.read().expect("rwlock poisoned")
    }

    pub fn write(&self) -> std::sync::RwLockWriteGuard<'_, T> {
        self.0.write().expect("rwlock poisoned")
    }
}
