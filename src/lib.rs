//! # astor
//!
//! A minimal HTTP framework for Rust services behind a reverse proxy.
//!
//! Two dependencies — [`matchit`](https://docs.rs/matchit) for routing,
//! [`tokio`](https://docs.rs/tokio) for async I/O. No hyper. No http crate.
//! No middleware stack you didn't ask for.
//!
//! ## The contract
//!
//! nginx handles TLS, rate limiting, slow clients, and body-size limits.
//! astor does not — by design. The proxy does proxy things. The framework
//! does framework things. Every feature astor skips is one nginx already
//! ships, tested at scale, at no cost to you.
//!
//! | nginx / ingress-nginx handles | astor's take |
//! |---|---|
//! | Body-size limits | `client_max_body_size` — done. |
//! | Header-size limits | `large_client_header_buffers` — done. |
//! | Rate limiting | `limit_req_zone` / ingress annotations — done. |
//! | Slow-client protection | nginx timeouts and buffers — done. |
//! | TLS termination | nginx SSL / k8s ingress — obviously. |
//! | HTTP/2 + HTTP/3 to clients | nginx negotiates; astor speaks HTTP/1.1 upstream. |
//!
//! What astor covers — the only part that changes between applications:
//!
//! - **Routing** — [`Router`] + [`matchit`](https://docs.rs/matchit), O(path-length) lookup
//! - **Async I/O** — raw tokio, no hyper
//! - **Graceful shutdown** — SIGTERM / Ctrl-C, drains in-flight requests
//!
//! ## Quick start
//!
//! ```rust,no_run
//! use astor::{Method, Request, Response, Router, Server, Status};
//!
//! #[tokio::main]
//! async fn main() {
//!     let app = Router::new()
//!         .on(Method::Delete, "/users/{id}", delete_user, ())
//!         .on(Method::Get,    "/users/{id}", get_user,    ())
//!         .on(Method::Post,   "/users",      create_user, ());
//!
//!     Server::bind("0.0.0.0:3000").serve(app).await.unwrap();
//! }
//!
//! // req.param("id") → Option<&str>
//! async fn get_user(req: Request) -> Response {
//!     let id = req.param("id").unwrap_or("unknown");
//!     // astor sends bytes — build them however you like:
//!     //   serde_json::to_vec(&user).unwrap()
//!     //   format!(r#"{{"id":"{id}"}}"#).into_bytes()
//!     # let bytes: Vec<u8> = vec![];
//!     Response::json(bytes)
//! }
//!
//! // req.body() → &[u8] — parse with serde_json, simd-json, or anything
//! async fn create_user(req: Request) -> Response {
//!     if req.body().is_empty() {
//!         return Response::status(Status::BadRequest);
//!     }
//!     # let bytes: Vec<u8> = vec![];
//!     Response::builder()
//!         .status(Status::Created)
//!         .header("location", "/users/99")
//!         .json(bytes)
//! }
//!
//! // Return Status directly — astor wraps it into a response
//! async fn delete_user(_req: Request) -> Status { Status::NoContent }
//! ```
//!
//! ## Status codes are a type, not a number
//!
//! Every IANA-registered status code is a named [`Status`] variant.
//! You cannot pass a raw integer where a status code goes — the compiler
//! stops you. There are no magic numbers, no typos that silently send the
//! wrong status, no `response(2040, bytes)` when you meant `204`.
//!
//! ```rust
//! use astor::{Response, Status};
//!
//! // Named. The compiler knows these are correct.
//! Response::status(Status::NoContent);   // 204
//! Response::status(Status::NotFound);    // 404
//!
//! # let bytes: Vec<u8> = vec![];
//! // The builder is the same discipline — explicit at every step.
//! Response::builder()
//!     .status(Status::Created)
//!     .header("location", "/users/42")
//!     .json(bytes);
//! ```
//!
//! ## nginx
//!
//! astor assumes nginx (or any reverse proxy) has already handled the items
//! below. It does not re-implement any of them.
//!
//! | Required nginx setting | What breaks without it |
//! |---|---|
//! | `proxy_buffering on` | astor only reads `Content-Length`-framed bodies — chunked bodies are silently dropped |
//! | `proxy_http_version 1.1` + `proxy_set_header Connection ""` | keep-alive pool collapses to one request per TCP connection |
//! | `client_max_body_size` | clients can stream unlimited body bytes |
//! | `client_header_buffer_size` / `large_client_header_buffers` | oversized headers are not rejected before reaching astor |
//! | `client_body_timeout` / `client_header_timeout` | slow clients are not dropped; astor has no timeout logic |
//! | method whitelist | nginx forwards any method string — `ANYTHING /path HTTP/1.1` reaches your handlers |
//!
//! Minimal example (two required lines shown — not a full config):
//!
//! ```nginx
//! # body size — can be set per location block for per-route limits
//! client_max_body_size 10m;
//!
//! # Example method whitelist — adjust to your service.
//! # Case-sensitive (~, not ~*): RFC 9110 requires uppercase; astor does not normalise.
//! if ($request_method !~ ^(GET|HEAD|POST|PUT|PATCH|DELETE|OPTIONS)$) {
//!     return 405;
//! }
//! ```
//!
//! Full config, Kubernetes ingress, and all required settings:
//! **[`docs/nginx.md`](https://github.com/benjaminPla/astor/blob/master/docs/nginx.md)**
//!
//! ## Key types
//!
//! | Type | Purpose |
//! |---|---|
//! | [`Router`] | Register routes — `Router::new().on(method, path, handler, extra_mw)` |
//! | [`Server`] | Bind a port and serve — `Server::bind(addr).serve(router)` |
//! | [`Request`] | Incoming request — method, path, headers, body, params |
//! | [`Response`] | Outgoing response — shortcuts + typed builder |
//! | [`Status`] | Every IANA status code as a named variant |
//! | [`Method`] | Every HTTP method — RFC 9110 + WebDAV + PURGE |
//! | [`ContentType`] | Common content-type values for [`Response::builder`] |
//! | [`IntoResponse`] | Implement on your own types to return them from handlers |

mod error;
mod handler;
mod method;
mod request;
mod response;
mod router;
mod server;
mod status;

pub mod middleware;

pub use error::Error;
pub use handler::Handler;
pub use method::Method;
pub use middleware::{Middleware, Next};
pub use request::Request;
pub use response::{ContentType, IntoResponse, Response};
pub use router::Router;
pub use server::Server;
pub use status::Status;
