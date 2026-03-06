# Changelog

All notable changes to astor are documented here.

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
astor adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

---

## [0.3.0] — 2026-03-06

### Added

- `Middleware` trait — sealed blanket impl for `async fn(Request, Next) -> Response`. Any function with that signature is a middleware.
- `Next` struct — drives the middleware chain. Call `next.call(req).await` to proceed; return a `Response` directly to short-circuit.
- `Router::middleware(mw)` — register global middleware applied to every route declared after the call.
- `Router::merge(other)` — compose sub-routers. Each sub-router's middleware chain is pre-built at startup; `merge` does not retroactively apply the parent router's global middleware to merged routes.
- `Request::query()` — returns the raw query string (without `?`), empty string if absent. Query strings are now stripped from the path before router lookup, so `GET /users/42?page=1` correctly matches `/users/{id}`.

### Changed

- **Breaking:** `Router::on` now takes four parameters: `(method, path, handler, extra_middleware)`. Pass `()` as the fourth argument for routes with no extra middleware.
- README trimmed to philosophy + quick start; nginx and Kubernetes config moved to [docs.rs/astor](https://docs.rs/astor).

---

## [0.2.1] — 2026-03-02

### Changed

- Comprehensive `///` doc pass across all public APIs — every type, variant, and method now has rustdoc coverage.
- README rewritten with philosophy-first approach, type safety section, and nginx delegation examples.

---

## [0.2.0] — 2026-02-25

### Added

- `Method` enum — all RFC 9110 standard methods (`Connect`, `Delete`, `Get`, `Head`, `Options`, `Patch`, `Post`, `Put`, `Trace`), WebDAV extensions (`Copy`, `Lock`, `Mkcalendar`, `Mkcol`, `Move`, `Propfind`, `Proppatch`, `Report`, `Search`, `Unlock`), and `Purge` (nginx / Varnish cache invalidation).

### Changed

- `Router::get`, `post`, `put`, `patch`, `delete`, `route` replaced by a single `Router::on(Method, path, handler)` — uniform API, no shorthand exceptions.
- `Request::method()` now returns `Method` instead of `&str`.
- Path parameters now use matchit's native `{param}` syntax instead of `:param`. Update routes accordingly: `/users/:id` → `/users/{id}`.

### Removed

- `health` module (`health::liveness`, `health::readiness`) — health endpoints are regular handlers; no built-in module needed. See README for the two-liner pattern.

---

## [0.1.1] — 2026-02-25

### Fixed

- Path parameters now match correctly. matchit 0.8 switched from `:param` to `{param}` syntax; astor translated at registration time so the user-facing API was unchanged.

### Removed

- `tracing` dependency — astor is a library; consumers bring their own logging. Errors surface via `Result`.

---

## [0.1.0] — 2026-02-25

First release. The foundation is here. Radix-tree routing, raw HTTP/1.1 parsing, graceful shutdown — and nothing the reverse proxy already handles.

### Added

- `ContentType` enum — `Csv`, `EventStream`, `FormData`, `Html`, `Json`, `MsgPack`, `OctetStream`, `Pdf`, `Text`, `Xml`.
- `health` module — built-in `health::liveness` (`/healthz`) and `health::readiness` (`/readyz`) for Kubernetes probes.
- `IntoResponse` trait — return your own types directly from handlers.
- `Request` — path parameters (`req.param`), method, URI, headers, raw body bytes (`req.body() -> &[u8]`).
- `Response` — shortcut constructors (`Response::json`, `Response::text`, `Response::status`) and a typed builder (`Response::builder().status(...).header(...).json(...)`).
- `Router` — radix-tree routing via `matchit`. `GET`, `POST`, `PUT`, `PATCH`, `DELETE`, and arbitrary methods.
- `Server::bind` — graceful shutdown on `SIGTERM` / `Ctrl-C`. Waits for in-flight requests to drain.
- `Status` enum — every IANA-registered HTTP status code as a named variant.
- nginx and Kubernetes deployment configuration documented in `README.md`.
- Raw tokio HTTP/1.1 parsing — no hyper, no http crate.

[Unreleased]: https://github.com/benjaminPla/astor/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/benjaminPla/astor/compare/v0.2.1...v0.3.0
[0.2.1]: https://github.com/benjaminPla/astor/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/benjaminPla/astor/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/benjaminPla/astor/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/benjaminPla/astor/releases/tag/v0.1.0
