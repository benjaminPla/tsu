//! Radix-tree request router.
//!
//! One tree per HTTP method. O(path-length) lookup. Middleware chains are
//! baked in at registration time — zero runtime composition per request.
//!
//! # Building the router
//!
//! ```rust,no_run
//! # use astor::{Method, Request, Response, Router};
//! # async fn get_user(_: Request) -> Response { Response::text("") }
//! # async fn create_user(_: Request) -> Response { Response::text("") }
//! # async fn delete_user(_: Request) -> Response { Response::text("") }
//! let app = Router::new()
//!     .on(Method::Delete, "/users/{id}", delete_user, ())
//!     .on(Method::Get,    "/users/{id}", get_user,    ())
//!     .on(Method::Post,   "/users",      create_user, ());
//! ```

use std::collections::HashMap;
use std::sync::Arc;

use matchit::Router as MatchitRouter;

use crate::handler::{BoxedHandler, Handler};
use crate::method::Method;
use crate::middleware::{BoxedMiddleware, IntoMiddlewares, Middleware};

/// The application router.
///
/// One radix tree per HTTP method — O(path-length) lookup, no allocations on
/// the hot path beyond Arc clones. Build it once at startup; pass it to
/// [`Server::serve`][crate::Server::serve].
///
/// Each [`Router::on`] call bakes the current global middleware chain plus any
/// per-route extras into an `Arc<[BoxedMiddleware]>` stored alongside the
/// handler. Sub-routers can be composed with [`Router::merge`].
///
/// Unmatched routes return `404 Not Found` automatically.
pub struct Router {
    routes: HashMap<Method, MatchitRouter<(BoxedHandler, Arc<[BoxedMiddleware]>)>>,
    /// Kept solely for [`merge`][Router::merge] — matchit 0.8 has no iteration API.
    raw: Vec<(Method, Box<str>, BoxedHandler, Arc<[BoxedMiddleware]>)>,
    /// Accumulated by [`middleware`][Router::middleware], consumed at each [`on`][Router::on] call.
    middleware: Vec<BoxedMiddleware>,
}

impl Router {
    /// Creates an empty router with no registered routes or middleware.
    pub fn new() -> Self {
        Self { routes: HashMap::new(), raw: Vec::new(), middleware: Vec::new() }
    }

    /// Append a global middleware that applies to every route registered on
    /// this router **after** this call. Call before [`on`][Router::on].
    ///
    /// ```rust,no_run
    /// # use astor::{Method, Next, Request, Response, Router, Status};
    /// # async fn list_users(_: Request) -> Response { Response::text("") }
    /// # async fn require_auth(req: Request, next: Next) -> Response { next.call(req).await }
    /// let users = Router::new()
    ///     .middleware(require_auth)
    ///     .on(Method::Get, "/users", list_users, ());
    /// ```
    pub fn middleware(mut self, mw: impl Middleware) -> Self {
        self.middleware.push(mw.into_boxed_middleware());
        self
    }

    /// Register a handler for a `method + path` pair. Returns `self` for chaining.
    ///
    /// `extra` adds per-route middleware appended **after** the global chain.
    /// Pass `()` for no extra middleware.
    ///
    /// Path parameters use `{name}` syntax — retrieve them with
    /// [`Request::param`][crate::Request::param]:
    ///
    /// ```rust,no_run
    /// # use astor::{Method, Next, Request, Response, Router, Status};
    /// # async fn get_user(_: Request) -> Response { Response::text("") }
    /// # async fn create_user(_: Request) -> Response { Response::text("") }
    /// # async fn ownership_check(req: Request, next: Next) -> Response { next.call(req).await }
    /// Router::new()
    ///     .on(Method::Get,  "/users/{id}", get_user,    ())
    ///     .on(Method::Post, "/users",      create_user, ownership_check);
    /// ```
    ///
    /// # Panics
    ///
    /// Panics at startup if two routes with the same method and path are
    /// registered, or if the path is not a valid matchit pattern.
    pub fn on(
        self,
        method: Method,
        path: &str,
        handler: impl Handler,
        extra: impl IntoMiddlewares,
    ) -> Self {
        let mut chain = self.middleware.clone();
        chain.extend(extra.into_middlewares());
        let chain: Arc<[BoxedMiddleware]> = chain.into();
        self.add_route(method, path, handler.into_boxed_handler(), chain)
    }

    /// Merge all routes from `other` into this router.
    ///
    /// Each route keeps its pre-built middleware chain unchanged — `self`'s
    /// global middleware does **not** retroactively apply to merged routes.
    /// Each sub-router owns its own chain.
    ///
    /// ```rust,no_run
    /// # use astor::{Method, Next, Request, Response, Router, Status};
    /// # async fn health(_: Request) -> Response { Response::text("ok") }
    /// # async fn list_users(_: Request) -> Response { Response::text("") }
    /// # async fn require_auth(req: Request, next: Next) -> Response { next.call(req).await }
    /// let public = Router::new().on(Method::Get, "/health", health, ());
    /// let users  = Router::new()
    ///     .middleware(require_auth)
    ///     .on(Method::Get, "/users", list_users, ());
    ///
    /// let app = Router::new().merge(public).merge(users);
    /// ```
    pub fn merge(mut self, other: Router) -> Self {
        for (method, path, handler, chain) in other.raw {
            self = self.add_route(method, &path, handler, chain);
        }
        self
    }

    fn add_route(
        mut self,
        method: Method,
        path: &str,
        handler: BoxedHandler,
        chain: Arc<[BoxedMiddleware]>,
    ) -> Self {
        self.routes
            .entry(method)
            .or_default()
            .insert(path, (Arc::clone(&handler), Arc::clone(&chain)))
            .unwrap_or_else(|e| panic!("invalid route `{path}`: {e}"));
        self.raw.push((method, path.into(), handler, chain));
        self
    }

    pub(crate) fn lookup(
        &self,
        method: Method,
        path: &str,
    ) -> Option<(BoxedHandler, Arc<[BoxedMiddleware]>, HashMap<String, String>)> {
        let tree = self.routes.get(&method)?;
        let matched = tree.at(path).ok()?;
        let handler = Arc::clone(&matched.value.0);
        let chain = Arc::clone(&matched.value.1);
        let params = matched
            .params
            .iter()
            .map(|(k, v)| (k.to_owned(), v.to_owned()))
            .collect();
        Some((handler, chain, params))
    }
}

impl Default for Router {
    fn default() -> Self {
        Self::new()
    }
}
