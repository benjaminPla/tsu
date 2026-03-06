//! Middleware layer — intercept and short-circuit requests.
//!
//! Middleware runs in a chain: each function can inspect the request, call
//! [`Next::call`] to proceed, or return a [`Response`][crate::Response]
//! immediately to short-circuit.
//!
//! Register global middleware on a router with
//! [`Router::middleware`][crate::Router::middleware]; per-route middleware is
//! the fourth argument to [`Router::on`][crate::Router::on].
//!
//! # No built-in middleware
//!
//! astor ships no middleware of its own — by design. Everything that typically
//! lives in a middleware layer (CORS headers, rate limiting, request-ID
//! injection, timeouts) is already handled by nginx before a request reaches
//! astor. Duplicating that work here contradicts the whole point of the
//! framework.
//!
//! Write your own for anything genuinely application-specific: ownership
//! checks, feature flags, audit logging. Everything else: configure nginx.
//!
//! # Execution order
//!
//! ```text
//! global middleware (registration order) → per-route middleware (left-to-right) → handler
//! ```
//!
//! The chain is baked into an `Arc<[BoxedMiddleware]>` at startup — zero
//! runtime composition per request.
//!
//! # Example
//!
//! ```rust,no_run
//! use astor::{Next, Request, Response, Status};
//!
//! async fn require_auth(req: Request, next: Next) -> Response {
//!     match req.header("authorization") {
//!         Some(_) => next.call(req).await,
//!         None    => Response::status(Status::Unauthorized),
//!     }
//! }
//! ```

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::handler::BoxedHandler;
use crate::request::Request;
use crate::response::Response;

// ── Internal types ────────────────────────────────────────────────────────────

type BoxFuture = Pin<Box<dyn Future<Output = Response> + Send + 'static>>;

#[doc(hidden)]
pub trait ErasedMiddleware {
    fn call(&self, req: Request, next: Next) -> BoxFuture;
}

/// A heap-allocated, type-erased middleware.
///
/// `#[doc(hidden)] pub` — appears in the bounds of public APIs but is not
/// meant for direct use outside the crate.
#[doc(hidden)]
pub type BoxedMiddleware = Arc<dyn ErasedMiddleware + Send + Sync + 'static>;

// ── Next ──────────────────────────────────────────────────────────────────────

/// The next step in the middleware chain.
///
/// Call [`Next::call`] inside a middleware to pass the request to the next
/// step. Drop it and return your own [`Response`][crate::Response] to
/// short-circuit the chain.
pub struct Next {
    middleware: Arc<[BoxedMiddleware]>,
    index: usize,
    handler: BoxedHandler,
}

impl Next {
    pub(crate) fn new(middleware: Arc<[BoxedMiddleware]>, handler: BoxedHandler) -> Self {
        Self { middleware, index: 0, handler }
    }

    /// Advance the chain. Runs the next middleware, or the handler when the
    /// chain is exhausted. Returns a future that resolves to a
    /// [`Response`][crate::Response].
    pub fn call(self, req: Request) -> Pin<Box<dyn Future<Output = Response> + Send + 'static>> {
        if self.index < self.middleware.len() {
            let mw = Arc::clone(&self.middleware[self.index]);
            let next = Next {
                middleware: self.middleware,
                index: self.index + 1,
                handler: self.handler,
            };
            mw.call(req, next)
        } else {
            self.handler.call(req)
        }
    }
}

// ── Middleware trait ──────────────────────────────────────────────────────────

mod private {
    pub trait Sealed {}
    pub trait IntoSeal {}
}

/// Implemented for every valid middleware function.
///
/// Automatically satisfied for any `async fn` with the signature:
///
/// ```text
/// async fn name(req: Request, next: Next) -> Response
/// ```
///
/// The trait is **sealed** — only the blanket impl can satisfy it. This keeps
/// the API stable and prevents accidental misuse.
pub trait Middleware: private::Sealed + Send + Sync + 'static {
    #[doc(hidden)]
    fn into_boxed_middleware(self) -> BoxedMiddleware;
}

impl<F, Fut> private::Sealed for F
where
    F: Fn(Request, Next) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Response> + Send + 'static,
{
}

impl<F, Fut> Middleware for F
where
    F: Fn(Request, Next) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Response> + Send + 'static,
{
    fn into_boxed_middleware(self) -> BoxedMiddleware {
        Arc::new(FnMiddleware(self))
    }
}

struct FnMiddleware<F>(F);

impl<F, Fut> ErasedMiddleware for FnMiddleware<F>
where
    F: Fn(Request, Next) -> Fut + Send + Sync,
    Fut: Future<Output = Response> + Send + 'static,
{
    fn call(&self, req: Request, next: Next) -> BoxFuture {
        Box::pin((self.0)(req, next))
    }
}

// ── IntoMiddlewares trait ─────────────────────────────────────────────────────

/// Converts a value into a list of boxed middleware. Implemented for:
/// - `()` — no extra middleware
/// - any single `M: Middleware`
/// - tuples `(M1, M2)` through `(M1, M2, M3, M4, M5)`
///
/// Pass `()` to [`Router::on`][crate::Router::on] when a route needs no extra
/// middleware beyond the router-level global chain.
///
/// The trait is **sealed** — no external implementations.
pub trait IntoMiddlewares: private::IntoSeal {
    #[doc(hidden)]
    fn into_middlewares(self) -> Vec<BoxedMiddleware>;
}

impl private::IntoSeal for () {}
impl IntoMiddlewares for () {
    fn into_middlewares(self) -> Vec<BoxedMiddleware> {
        vec![]
    }
}

impl<M: Middleware> private::IntoSeal for M {}
impl<M: Middleware> IntoMiddlewares for M {
    fn into_middlewares(self) -> Vec<BoxedMiddleware> {
        vec![self.into_boxed_middleware()]
    }
}

impl<M1: Middleware, M2: Middleware> private::IntoSeal for (M1, M2) {}
impl<M1: Middleware, M2: Middleware> IntoMiddlewares for (M1, M2) {
    fn into_middlewares(self) -> Vec<BoxedMiddleware> {
        vec![self.0.into_boxed_middleware(), self.1.into_boxed_middleware()]
    }
}

impl<M1: Middleware, M2: Middleware, M3: Middleware> private::IntoSeal for (M1, M2, M3) {}
impl<M1: Middleware, M2: Middleware, M3: Middleware> IntoMiddlewares for (M1, M2, M3) {
    fn into_middlewares(self) -> Vec<BoxedMiddleware> {
        vec![
            self.0.into_boxed_middleware(),
            self.1.into_boxed_middleware(),
            self.2.into_boxed_middleware(),
        ]
    }
}

impl<M1: Middleware, M2: Middleware, M3: Middleware, M4: Middleware> private::IntoSeal
    for (M1, M2, M3, M4)
{
}
impl<M1: Middleware, M2: Middleware, M3: Middleware, M4: Middleware> IntoMiddlewares
    for (M1, M2, M3, M4)
{
    fn into_middlewares(self) -> Vec<BoxedMiddleware> {
        vec![
            self.0.into_boxed_middleware(),
            self.1.into_boxed_middleware(),
            self.2.into_boxed_middleware(),
            self.3.into_boxed_middleware(),
        ]
    }
}

impl<M1: Middleware, M2: Middleware, M3: Middleware, M4: Middleware, M5: Middleware>
    private::IntoSeal for (M1, M2, M3, M4, M5)
{
}
impl<M1: Middleware, M2: Middleware, M3: Middleware, M4: Middleware, M5: Middleware>
    IntoMiddlewares for (M1, M2, M3, M4, M5)
{
    fn into_middlewares(self) -> Vec<BoxedMiddleware> {
        vec![
            self.0.into_boxed_middleware(),
            self.1.into_boxed_middleware(),
            self.2.into_boxed_middleware(),
            self.3.into_boxed_middleware(),
            self.4.into_boxed_middleware(),
        ]
    }
}
