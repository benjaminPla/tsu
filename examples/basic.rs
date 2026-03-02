//! astor example — covers every Response variant and common handler patterns.
//!
//! Run:
//!   cargo run --example basic
//!
//! Try:
//!   curl http://localhost:3000/healthz
//!   curl http://localhost:3000/readyz
//!   curl http://localhost:3000/redirect
//!   curl "http://localhost:3000/users?page=2&limit=10"
//!   curl http://localhost:3000/users/42
//!   curl http://localhost:3000/xml
//!   curl -X DELETE http://localhost:3000/users/42
//!   curl -X PATCH  http://localhost:3000/users/42 \
//!        -H 'content-type: application/json' -d '{"name":"bob"}'
//!   curl -X POST   http://localhost:3000/users \
//!        -H 'content-type: application/json' -d '{"name":"alice"}'

use astor::{ContentType, Method, Request, Response, Router, Server, Status};

#[tokio::main]
async fn main() {
    let app = Router::new()
        .on(Method::Delete, "/users/{id}", delete_user)
        .on(Method::Get,    "/healthz",    liveness)
        .on(Method::Get,    "/readyz",     readiness)
        .on(Method::Get,    "/redirect",   redirect)
        .on(Method::Get,    "/users",      list_users)
        .on(Method::Get,    "/users/{id}", get_user)
        .on(Method::Get,    "/xml",        xml_response)
        .on(Method::Patch,  "/users/{id}", update_user)
        .on(Method::Post,   "/users",      create_user);

    Server::bind("0.0.0.0:3000").serve(app).await.expect("server error");
}

// ── Health ────────────────────────────────────────────────────────────────────
//
// Health endpoints are regular handlers. No magic, no built-in module.
// Gate readiness on dependency health (db pools, downstream services, etc.)
// if your app needs a warm-up period before serving traffic.
async fn liveness(_req: Request) -> Response { Response::text("ok") }
async fn readiness(_req: Request) -> Response { Response::text("ready") }

// ── GET /users ────────────────────────────────────────────────────────────────
//
// req.query() returns the raw query string without the leading '?'.
// Parse it with serde_qs, form_urlencoded, or split manually — astor
// does not interpret query parameters, and the route matches regardless
// of whether a query string is present.
async fn list_users(req: Request) -> Response {
    // "page=2&limit=10" or "" if no query string
    let qs = req.query();
    Response::json(format!(r#"{{"query":"{qs}","users":[]}}"#).into_bytes())
}

// ── GET /users/{id} ───────────────────────────────────────────────────────────
//
// Response::json takes Vec<u8> — pass bytes from your serialiser directly.
//   serde_json:  Response::json(serde_json::to_vec(&user).unwrap())
//   hand-built:  format!(...).into_bytes()  ← zero-cost, no copy
async fn get_user(req: Request) -> Response {
    let id = req.param("id").unwrap_or("unknown");
    Response::json(format!(r#"{{"id":"{id}","name":"alice"}}"#).into_bytes())
}

// ── POST /users ───────────────────────────────────────────────────────────────
//
// req.body() is &[u8]. Parse with serde_json::from_slice, simd-json, etc.
// 201 Created + Location header.
async fn create_user(req: Request) -> Response {
    if req.body().is_empty() {
        return Response::status(Status::BadRequest);
    }
    Response::builder()
        .status(Status::Created)
        .header("location", "/users/99")
        .json(r#"{"id":"99","name":"new_user"}"#.to_owned().into_bytes())
}

// ── PATCH /users/{id} ────────────────────────────────────────────────────────
async fn update_user(req: Request) -> Response {
    let id = req.param("id").unwrap_or("unknown");
    Response::json(format!(r#"{{"id":"{id}","name":"updated"}}"#).into_bytes())
}

// ── DELETE /users/{id} ───────────────────────────────────────────────────────
//
// Return Status directly from a handler — astor wraps it into a response.
async fn delete_user(_req: Request) -> Status {
    Status::NoContent
}

// ── GET /xml ──────────────────────────────────────────────────────────────────
//
// Non-JSON body via ContentType enum.
// Same pattern works for Html, Csv, Pdf, OctetStream, MsgPack, EventStream.
async fn xml_response(_req: Request) -> Response {
    Response::builder()
        .status(Status::Ok)
        .bytes(ContentType::Xml, b"<users><user id=\"1\"/></users>".to_vec())
}

// ── GET /redirect ─────────────────────────────────────────────────────────────
//
// 301 redirect — custom status + header, no body.
async fn redirect(_req: Request) -> Response {
    Response::builder()
        .status(Status::MovedPermanently)
        .header("location", "/users/1")
        .no_body()
}
