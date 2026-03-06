//! astor example — middleware chains, sub-router composition, and common
//! handler patterns.
//!
//! Run:
//!   cargo run --example basic
//!
//! Try:
//!   curl http://localhost:3000/health
//!   curl http://localhost:3000/users                                 # 401 — no auth
//!   curl -H "authorization: Bearer x" http://localhost:3000/users   # 200
//!   curl -H "authorization: Bearer x" "http://localhost:3000/users?page=2&limit=5"
//!   curl -H "authorization: Bearer x" http://localhost:3000/users/42
//!   curl -H "authorization: Bearer x" -X PUT http://localhost:3000/users/42
//!   curl -H "authorization: Bearer x" -X DELETE http://localhost:3000/users/42
//!   curl -H "authorization: Bearer x" http://localhost:3000/products
//!   curl -H "authorization: Bearer x" -X POST http://localhost:3000/products \
//!        -H 'content-type: application/json' -d '{"name":"widget"}'

use astor::{Method, Middleware, Next, Request, Response, Router, Server, Status};

#[tokio::main]
async fn main() {
    // ── Sub-router: users — auth on every route, ownership check on mutations
    let users = Router::new()
        .middleware(require_auth)
        .on(Method::Get,    "/users",      list_users,   ())
        .on(Method::Get,    "/users/{id}", get_user,     ())
        .on(Method::Post,   "/users",      create_user,  ())
        .on(Method::Put,    "/users/{id}", update_user,  ownership_check)
        .on(Method::Delete, "/users/{id}", delete_user,  ownership_check);

    // ── Sub-router: products — auth on all, extra logging on POST
    let products = Router::new()
        .middleware(require_auth)
        .on(Method::Get,  "/products",  list_products,  ())
        .on(Method::Post, "/products",  create_product, log_body);

    // ── Public routes — no middleware
    let public = Router::new()
        .on(Method::Get, "/health", health, ());

    // ── Compose and serve
    //
    // merge() re-inserts each sub-router's routes with their pre-built chains.
    // The top-level Router::new() has no global middleware of its own, so
    // each sub-router's chain is unchanged.
    let app = Router::new()
        .merge(public)
        .merge(users)
        .merge(products);

    Server::bind("0.0.0.0:3000")
        .serve(app)
        .await
        .expect("server error");
}

// ── Middleware ────────────────────────────────────────────────────────────────
//
// Middleware signature: async fn(Request, Next) -> Response
// Call next.call(req).await to proceed; return directly to short-circuit.

async fn require_auth(req: Request, next: Next) -> Response {
    match req.header("authorization") {
        Some(_) => next.call(req).await,
        None    => Response::status(Status::Unauthorized),
    }
}

// Per-route middleware — only applied to PUT/DELETE /users/{id}
async fn ownership_check(req: Request, next: Next) -> Response {
    // In a real app: verify the caller owns the resource.
    // Here we just proceed to demonstrate the chain.
    next.call(req).await
}

// Demonstrates passing middleware as a value — accepts impl Middleware
async fn log_body(req: Request, next: Next) -> Response {
    let len = req.body().len();
    let res = next.call(req).await;
    // In a real app: emit a metric or trace here.
    let _ = len;
    res
}

// Helper: any async fn that matches the middleware signature is accepted.
// You can also pass closures or wrap middleware in your own newtype.
#[allow(dead_code)]
fn make_prefix_mw(prefix: &'static str) -> impl Middleware {
    move |req: Request, next: Next| async move {
        let _ = prefix;
        next.call(req).await
    }
}

// ── Handlers ─────────────────────────────────────────────────────────────────

async fn health(_req: Request) -> Response {
    Response::text("ok")
}

// GET /users?page=2&limit=10
async fn list_users(req: Request) -> Response {
    let page  = req.query("page").unwrap_or("1");
    let limit = req.query("limit").unwrap_or("20");
    Response::json(format!(r#"{{"page":{page},"limit":{limit},"users":[]}}"#).into_bytes())
}

async fn get_user(req: Request) -> Response {
    let id = req.param("id").unwrap_or("unknown");
    Response::json(format!(r#"{{"id":"{id}","name":"alice"}}"#).into_bytes())
}

async fn create_user(req: Request) -> Response {
    if req.body().is_empty() {
        return Response::status(Status::BadRequest);
    }
    Response::builder()
        .status(Status::Created)
        .header("location", "/users/99")
        .json(r#"{"id":"99","name":"new_user"}"#.to_owned().into_bytes())
}

async fn update_user(req: Request) -> Response {
    let id = req.param("id").unwrap_or("unknown");
    Response::json(format!(r#"{{"id":"{id}","name":"updated"}}"#).into_bytes())
}

// Return Status directly — astor wraps it into a response
async fn delete_user(_req: Request) -> Status {
    Status::NoContent
}

async fn list_products(_req: Request) -> Response {
    Response::json(br#"{"products":[]}"#.to_vec())
}

async fn create_product(req: Request) -> Response {
    if req.body().is_empty() {
        return Response::status(Status::BadRequest);
    }
    Response::builder()
        .status(Status::Created)
        .header("location", "/products/1")
        .json(r#"{"id":"1"}"#.to_owned().into_bytes())
}
