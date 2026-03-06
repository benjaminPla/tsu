# astor

[![Crates.io](https://img.shields.io/crates/v/astor)](https://crates.io/crates/astor)
[![docs.rs](https://img.shields.io/docsrs/astor)](https://docs.rs/astor)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![CI](https://github.com/benjaminPla/astor/actions/workflows/ci.yml/badge.svg)](https://github.com/benjaminPla/astor/actions)

> HTTP for Rust services behind a reverse proxy. Does its job. Goes home.

Two dependencies — [`matchit`] for routing, `tokio` for async I/O. No hyper. No middleware stack you didn't ask for.

---

## The idea

You're running behind nginx. nginx already handles TLS, rate limiting, slow clients, and body-size limits. Every general-purpose Rust HTTP framework re-implements all of that. astor doesn't — that's the whole point.

What changes between services is exactly what astor covers:

- **Routing** — radix tree, O(path-length), `{name}` parameter syntax
- **Typed responses** — named status codes, response shortcuts, typed builder
- **Graceful shutdown** — SIGTERM / Ctrl-C, drains in-flight requests before exit

---

## Quick start

```toml
# Cargo.toml
[dependencies]
astor = "0.3"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

```rust
use astor::{Method, Request, Response, Router, Server, Status};

#[tokio::main]
async fn main() {
    let app = Router::new()
        .on(Method::Get,    "/users",      list_users,  ())
        .on(Method::Get,    "/users/{id}", get_user,    ())
        .on(Method::Post,   "/users",      create_user, ())
        .on(Method::Delete, "/users/{id}", delete_user, ());

    Server::bind("0.0.0.0:3000").serve(app).await.unwrap();
}

// Path params: {name} syntax — req.param("id") → Option<&str>
async fn get_user(req: Request) -> Response {
    let id = req.param("id").unwrap_or("unknown");
    Response::json(format!(r#"{{"id":"{id}"}}"#).into_bytes())
}

// Query params pre-parsed — req.query("key") → Option<&str>
// GET /users?page=2&limit=10 still matches the /users route
async fn list_users(req: Request) -> Response {
    let page  = req.query("page").unwrap_or("1");
    let limit = req.query("limit").unwrap_or("20");
    Response::json(format!(r#"{{"page":{page},"limit":{limit},"users":[]}}"#).into_bytes())
}

// req.body() → &[u8] — parse with serde_json, simd-json, or anything else
async fn create_user(req: Request) -> Response {
    if req.body().is_empty() {
        return Response::status(Status::BadRequest);
    }
    Response::builder()
        .status(Status::Created)
        .header("location", "/users/99")
        .json(r#"{"id":"99"}"#.to_owned().into_bytes())
}

// Return Status directly — astor wraps it into a response
async fn delete_user(_req: Request) -> Status { Status::NoContent }
```

```sh
cargo run --example basic
curl http://localhost:3000/users/42
curl "http://localhost:3000/users?page=2&limit=10"
```

---

## Middleware

Middleware runs as an ordered chain: global (router-level) first, then per-route extras, then the handler. The chain is baked into each route at startup — no runtime composition.

```rust
use astor::{Next, Request, Response, Router, Status};

async fn require_auth(req: Request, next: Next) -> Response {
    match req.header("authorization") {
        Some(_) => next.call(req).await,           // proceed
        None    => Response::status(Status::Unauthorized), // short-circuit
    }
}

async fn ownership_check(req: Request, next: Next) -> Response {
    // verify ownership, then proceed
    next.call(req).await
}
```

**Global middleware** applies to every route registered after `.middleware()`:

```rust
let users = Router::new()
    .middleware(require_auth)                                    // applied to all routes below
    .on(Method::Get,    "/users",      list_users,   ())        // chain: [require_auth]
    .on(Method::Put,    "/users/{id}", update_user,  ownership_check) // chain: [require_auth, ownership_check]
    .on(Method::Delete, "/users/{id}", delete_user,  ownership_check);
```

**Sub-router composition** via `.merge()` — each sub-router keeps its own chain:

```rust
let public   = Router::new().on(Method::Get, "/health", health, ());
let users    = Router::new().middleware(require_auth).on(/* ... */);
let products = Router::new().middleware(require_auth).on(/* ... */);

let app = Router::new().merge(public).merge(users).merge(products);
Server::bind("0.0.0.0:3000").serve(app).await.unwrap();
```

Pass `()` as the fourth argument to `.on()` when a route needs no extra middleware.

---

## Status codes are a type, not a number

No free-form response constructor. No raw integers. Every status code is a named [`Status`] variant — tab-completable, greppable, compiler-verified.

```rust
Response::status(Status::NoContent)   // 204 — not "204", not 204
Response::status(Status::NotFound)    // 404

Response::builder()
    .status(Status::Created)
    .header("location", "/users/42")
    .json(bytes)

// Return Status directly from a handler
async fn delete_user(_req: Request) -> Status { Status::NoContent }
```

Every IANA-registered code from 100 to 511 — full list on [docs.rs/astor](https://docs.rs/astor/latest/astor/enum.Status.html).

---

Full API reference, nginx config, and Kubernetes deployment guide: **[docs.rs/astor](https://docs.rs/astor)**

---

## Contributing

Contributions are welcome. Read [CONTRIBUTING.md](CONTRIBUTING.md) before opening a PR. See [CHANGELOG.md](CHANGELOG.md) for release history.

---

## License

MIT

[matchit]: https://github.com/ibraheemdev/matchit
