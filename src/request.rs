//! Incoming HTTP request type.
//!
//! Parsed from the raw TCP stream by the server. By the time your handler
//! receives a [`Request`], the proxy has already validated and buffered the
//! input. You get a clean struct — no ceremony, no streaming.
//!
//! # Accessing request data
//!
//! ```rust,no_run
//! use astor::{Request, Response, Status};
//!
//! async fn handler(req: Request) -> Response {
//!     // Path parameter — registered as {id} in the route
//!     let id = req.param("id").unwrap_or("unknown");
//!
//!     // Single header — case-insensitive
//!     let auth = req.header("authorization");
//!
//!     // Raw body bytes — parse with whatever you want
//!     if req.body().is_empty() {
//!         return Response::status(Status::BadRequest);
//!     }
//!
//!     Response::text(id)
//! }
//! ```

use std::collections::HashMap;

use crate::method::Method;

/// An incoming HTTP request, parsed from the raw TCP stream.
///
/// Constructed by the server before dispatch. Fields are read-only — handlers
/// receive the request and return a [`Response`][crate::Response].
pub struct Request {
    pub(crate) body: Vec<u8>,
    pub(crate) headers: Vec<(String, String)>,
    pub(crate) method: Method,
    pub(crate) params: HashMap<String, String>,
    pub(crate) path: String,
    pub(crate) query: String,
}

impl Request {
    pub(crate) fn new(
        body: Vec<u8>,
        headers: Vec<(String, String)>,
        method: Method,
        params: HashMap<String, String>,
        path: String,
        query: String,
    ) -> Self {
        Self { body, headers, method, params, path, query }
    }

    /// Returns the HTTP method.
    pub fn method(&self) -> Method { self.method }

    /// Returns the request path, without the query string.
    ///
    /// For a request URI of `/users/42?page=1` this returns `/users/42`.
    pub fn path(&self) -> &str { &self.path }

    /// Returns the raw query string, without the leading `?`.
    ///
    /// Empty string if the request had no query string.
    /// Parse it with `serde_qs`, `form_urlencoded`, or a manual split —
    /// astor does not interpret query parameters.
    ///
    /// ```rust,no_run
    /// # use astor::{Request, Response};
    /// async fn handler(req: Request) -> Response {
    ///     // e.g. GET /search?q=rust&page=2
    ///     let qs = req.query(); // "q=rust&page=2"
    ///     Response::text(qs)
    /// }
    /// ```
    pub fn query(&self) -> &str { &self.query }

    /// Returns all request headers as name-value pairs.
    ///
    /// Header names are lowercased by nginx before reaching astor.
    /// For single-header lookup, prefer [`header`][Request::header].
    pub fn headers(&self) -> &[(String, String)] { &self.headers }

    /// Returns the raw request body as bytes.
    ///
    /// astor never interprets the bytes — parse them with whatever fits your
    /// use case:
    /// - `serde_json::from_slice(req.body())`
    /// - `simd_json::from_slice(req.body())`
    /// - hand-rolled parsing for simple formats
    ///
    /// An empty body returns an empty slice. Check `is_empty()` before
    /// attempting to parse.
    ///
    /// Body size is constrained by `client_max_body_size` in your nginx config,
    /// not by astor. Gate on `body.len()` inside the handler if you need
    /// per-route limits tighter than the global nginx setting.
    pub fn body(&self) -> &[u8] { &self.body }

    /// Case-insensitive lookup for a single header by name.
    ///
    /// Returns `None` if the header is absent. Header names are lowercased by
    /// nginx, so `"Authorization"` and `"authorization"` both match.
    ///
    /// ```rust,no_run
    /// # use astor::{Request, Response, Status};
    /// async fn handler(req: Request) -> Response {
    ///     match req.header("authorization") {
    ///         Some(token) => Response::text(token),
    ///         None        => Response::status(Status::Unauthorized),
    ///     }
    /// }
    /// ```
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    /// Returns a named path parameter extracted by the router.
    ///
    /// For a route `/users/{id}`, `req.param("id")` on `/users/42` returns
    /// `Some("42")`. Returns `None` if the key is not in the route pattern.
    ///
    /// ```rust,no_run
    /// # use astor::{Request, Response};
    /// // Route: /orgs/{org}/repos/{repo}
    /// async fn get_repo(req: Request) -> Response {
    ///     let org  = req.param("org").unwrap();
    ///     let repo = req.param("repo").unwrap();
    ///     Response::text(format!("{org}/{repo}"))
    /// }
    /// ```
    pub fn param(&self, key: &str) -> Option<&str> {
        self.params.get(key).map(String::as_str)
    }
}
