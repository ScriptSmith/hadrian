//! Compatibility layer for concurrency primitives and WASM routing.
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
//!
//! ## WASM routing (`AssertSend` / `WasmHandler`)
//!
//! Axum requires handler futures to be `Send`, but on wasm32 `reqwest`/`wasm-bindgen`
//! futures are `!Send`. Since WASM is single-threaded, `Send` is vacuously satisfied.
//!
//! [`AssertSend`] wraps any future with `unsafe impl Send`, and [`WasmHandler`]
//! wraps handler functions so they produce `AssertSend` futures. The [`wasm_routing`]
//! module provides drop-in replacements for `axum::routing::{get, post, ...}` that
//! automatically wrap handlers in `WasmHandler`.

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
// AssertSend / WasmHandler (wasm32 only)
// ─────────────────────────────────────────────────────────────────────────────

/// A future wrapper that asserts `Send` for `!Send` futures on wasm32.
///
/// # Safety
///
/// WASM is single-threaded, so `Send` is vacuously satisfied — there is no other
/// thread that could observe the wrapped future.
#[cfg(target_arch = "wasm32")]
pub struct AssertSend<F>(pub F);

#[cfg(target_arch = "wasm32")]
// SAFETY: wasm32 is single-threaded; Send is vacuously satisfied.
unsafe impl<F> Send for AssertSend<F> {}

#[cfg(target_arch = "wasm32")]
impl<F: std::future::Future> std::future::Future for AssertSend<F> {
    type Output = F::Output;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        // SAFETY: We only project through the newtype; pinning is preserved.
        unsafe { self.map_unchecked_mut(|s| &mut s.0) }.poll(cx)
    }
}

/// A stream wrapper that asserts `Send` for `!Send` streams on wasm32.
///
/// Like [`AssertSend`] but for `Stream` instead of `Future`. This enables
/// `Body::from_stream()` with reqwest byte streams which are `!Send` on WASM.
///
/// # Safety
///
/// WASM is single-threaded, so `Send` is vacuously satisfied.
#[cfg(target_arch = "wasm32")]
pub struct AssertSendStream<S>(pub S);

#[cfg(target_arch = "wasm32")]
// SAFETY: wasm32 is single-threaded; Send is vacuously satisfied.
unsafe impl<S> Send for AssertSendStream<S> {}

#[cfg(target_arch = "wasm32")]
impl<S: futures_util::Stream> futures_util::Stream for AssertSendStream<S> {
    type Item = S::Item;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        // SAFETY: We only project through the newtype; pinning is preserved.
        unsafe { self.map_unchecked_mut(|s| &mut s.0) }.poll_next(cx)
    }
}

/// Wraps any handler function so its return future is `Send` (via [`AssertSend`]).
///
/// This is a newtype used by the [`wasm_routing`] module's drop-in routing functions.
#[cfg(target_arch = "wasm32")]
pub struct WasmHandler<H>(pub H);

#[cfg(target_arch = "wasm32")]
impl<H: Clone> Clone for WasmHandler<H> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

// SAFETY: wasm32 is single-threaded; Sync is vacuously satisfied.
#[cfg(target_arch = "wasm32")]
unsafe impl<H> Sync for WasmHandler<H> {}

/// Implement `Handler<(T1..Tn, M), S>` for `WasmHandler<F>`.
///
/// Mirrors axum's own `Handler` impl but removes the `Send` bound on `Fut`
/// and wraps the output in [`AssertSend`].
#[cfg(target_arch = "wasm32")]
macro_rules! impl_wasm_handler {
    ( [$($ty:ident),*], $last:ident ) => {
        #[allow(non_snake_case)]
        impl<F, Fut, Res, S, M, $($ty,)* $last> axum::handler::Handler<(M, $($ty,)* $last,), S>
            for WasmHandler<F>
        where
            F: FnOnce($($ty,)* $last,) -> Fut + Clone + Send + 'static,
            Fut: std::future::Future<Output = Res> + 'static, // no Send bound
            Res: axum::response::IntoResponse,
            S: Send + Sync + Clone + 'static,
            $( $ty: axum::extract::FromRequestParts<S> + Send, )*
            $last: axum::extract::FromRequest<S, M> + Send,
        {
            type Future = AssertSend<std::pin::Pin<Box<dyn std::future::Future<Output = axum::response::Response> + 'static>>>;

            fn call(
                self,
                req: axum::http::Request<axum::body::Body>,
                state: S,
            ) -> Self::Future {
                let (mut parts, body) = req.into_parts();
                AssertSend(Box::pin(async move {
                    use axum::response::IntoResponse as _;
                    $(
                        let $ty = match $ty::from_request_parts(&mut parts, &state).await {
                            Ok(value) => value,
                            Err(rejection) => return rejection.into_response(),
                        };
                    )*
                    let req = axum::http::Request::from_parts(parts, body);
                    let $last = match $last::from_request(req, &state).await {
                        Ok(value) => value,
                        Err(rejection) => return rejection.into_response(),
                    };
                    (self.0)($($ty,)* $last,).await.into_response()
                }))
            }
        }
    };
}

// Zero-argument handler impl (mirrors axum's `FnOnce() -> Fut` impl).
#[cfg(target_arch = "wasm32")]
impl<F, Fut, Res, S> axum::handler::Handler<((),), S> for WasmHandler<F>
where
    F: FnOnce() -> Fut + Clone + Send + 'static,
    Fut: std::future::Future<Output = Res> + 'static,
    Res: axum::response::IntoResponse,
{
    type Future = AssertSend<
        std::pin::Pin<Box<dyn std::future::Future<Output = axum::response::Response> + 'static>>,
    >;

    fn call(self, _req: axum::http::Request<axum::body::Body>, _state: S) -> Self::Future {
        AssertSend(Box::pin(async move {
            axum::response::IntoResponse::into_response(self.0().await)
        }))
    }
}

// Implement for arities 1..16 (matching axum's supported handler arities).
#[cfg(target_arch = "wasm32")]
impl_wasm_handler!([], T1);
#[cfg(target_arch = "wasm32")]
impl_wasm_handler!([T1], T2);
#[cfg(target_arch = "wasm32")]
impl_wasm_handler!([T1, T2], T3);
#[cfg(target_arch = "wasm32")]
impl_wasm_handler!([T1, T2, T3], T4);
#[cfg(target_arch = "wasm32")]
impl_wasm_handler!([T1, T2, T3, T4], T5);
#[cfg(target_arch = "wasm32")]
impl_wasm_handler!([T1, T2, T3, T4, T5], T6);
#[cfg(target_arch = "wasm32")]
impl_wasm_handler!([T1, T2, T3, T4, T5, T6], T7);
#[cfg(target_arch = "wasm32")]
impl_wasm_handler!([T1, T2, T3, T4, T5, T6, T7], T8);
#[cfg(target_arch = "wasm32")]
impl_wasm_handler!([T1, T2, T3, T4, T5, T6, T7, T8], T9);
#[cfg(target_arch = "wasm32")]
impl_wasm_handler!([T1, T2, T3, T4, T5, T6, T7, T8, T9], T10);
#[cfg(target_arch = "wasm32")]
impl_wasm_handler!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10], T11);
#[cfg(target_arch = "wasm32")]
impl_wasm_handler!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11], T12);
#[cfg(target_arch = "wasm32")]
impl_wasm_handler!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12], T13);
#[cfg(target_arch = "wasm32")]
impl_wasm_handler!(
    [T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13],
    T14
);
#[cfg(target_arch = "wasm32")]
impl_wasm_handler!(
    [T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14],
    T15
);
#[cfg(target_arch = "wasm32")]
impl_wasm_handler!(
    [
        T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15
    ],
    T16
);

/// Drop-in replacements for `axum::routing::{get, post, put, patch, delete}` that
/// automatically wrap handlers in [`WasmHandler`] so `!Send` futures compile on wasm32.
///
/// Usage in route modules:
/// ```ignore
/// #[cfg(feature = "server")]
/// use axum::routing::{get, post};
/// #[cfg(feature = "wasm")]
/// use crate::compat::wasm_routing::{get, post};
/// ```
#[cfg(target_arch = "wasm32")]
pub mod wasm_routing {
    use axum::{handler::Handler, routing::MethodRouter};

    use super::WasmHandler;

    pub fn get<H, T, S>(handler: H) -> MethodRouter<S>
    where
        WasmHandler<H>: Handler<T, S>,
        T: 'static,
        S: Clone + Send + Sync + 'static,
    {
        axum::routing::get(WasmHandler(handler))
    }

    pub fn post<H, T, S>(handler: H) -> MethodRouter<S>
    where
        WasmHandler<H>: Handler<T, S>,
        T: 'static,
        S: Clone + Send + Sync + 'static,
    {
        axum::routing::post(WasmHandler(handler))
    }

    pub fn put<H, T, S>(handler: H) -> MethodRouter<S>
    where
        WasmHandler<H>: Handler<T, S>,
        T: 'static,
        S: Clone + Send + Sync + 'static,
    {
        axum::routing::put(WasmHandler(handler))
    }

    pub fn patch<H, T, S>(handler: H) -> MethodRouter<S>
    where
        WasmHandler<H>: Handler<T, S>,
        T: 'static,
        S: Clone + Send + Sync + 'static,
    {
        axum::routing::patch(WasmHandler(handler))
    }

    pub fn delete<H, T, S>(handler: H) -> MethodRouter<S>
    where
        WasmHandler<H>: Handler<T, S>,
        T: 'static,
        S: Clone + Send + Sync + 'static,
    {
        axum::routing::delete(WasmHandler(handler))
    }
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
