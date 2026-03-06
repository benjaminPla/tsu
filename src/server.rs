//! HTTP server and graceful shutdown.
//!
//! # Why not hyper?
//!
//! Because nginx already validated every HTTP quirk from the untrusted client
//! before forwarding. The nginx → astor link is a clean, trusted HTTP/1.1
//! stream. A line-oriented parser over a tokio `BufReader` is enough.
//! Pulling in hyper for that would be like hiring a bouncer for your living room.
//!
//! # nginx requirements — not optional
//!
//! astor trusts nginx to have already done the work described here before
//! forwarding. See `docs/nginx.md` in the repo for the full config reference.
//!
//! ## `proxy_buffering on`
//!
//! astor reads `Content-Length`-framed bodies only. `proxy_buffering on`
//! (the nginx default) ensures the full body arrives with a `Content-Length`
//! header. Set it to `off` and you get chunked bodies astor cannot parse.
//! Don't do it.
//!
//! ## Method filtering
//!
//! nginx does not validate HTTP methods by default — `ANYTHING /path HTTP/1.1`
//! gets forwarded. Configure a whitelist so unknown methods never reach astor:
//!
//! ```nginx
//! if ($request_method !~ ^(GET|HEAD|POST|PUT|PATCH|DELETE|OPTIONS)$) {
//!     return 405;
//! }
//! ```
//!
//! The regex is case-sensitive (`~`, not `~*`). astor's method parser is
//! case-sensitive per RFC 9110 §9.1 and assumes nginx already enforces
//! uppercase — it does not normalise case itself.
//!
//! # Keep-alive — nginx's business, not ours
//!
//! nginx reuses connections to astor. astor loops until nginx closes them (EOF).
//! We never inspect the `Connection` header. nginx handles it. Let it.
//!
//! # Graceful shutdown
//!
//! On SIGTERM / Ctrl-C: accept loop stops, in-flight tasks drain, then exit.
//! Set `terminationGracePeriodSeconds` longer than your slowest request or k8s
//! SIGKILLs the pod before drain finishes. That is not graceful shutdown.

use std::net::SocketAddr;
use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::net::{TcpListener, TcpStream};

use crate::error::Error;
use crate::method::Method;
use crate::middleware::Next;
use crate::request::Request;
use crate::response::Response;
use crate::router::Router;
use crate::status::Status; // used in dispatch fallback (404)

/// The HTTP server.
///
/// Binds a TCP port, accepts connections, and dispatches requests through a
/// [`Router`]. Handles graceful shutdown automatically — no setup required.
///
/// ```rust,no_run
/// use astor::{Method, Request, Response, Router, Server};
///
/// #[tokio::main]
/// async fn main() {
///     let app = Router::new()
///         .on(Method::Get, "/", |_req: Request| async { Response::text("ok") });
///
///     Server::bind("0.0.0.0:3000").serve(app).await.unwrap();
/// }
/// ```
pub struct Server {
    addr: SocketAddr,
}

impl Server {
    /// Configures the server to bind on `addr`.
    ///
    /// `addr` must be a valid `host:port` string:
    /// - `"0.0.0.0:3000"` — all interfaces (standard for containerised services)
    /// - `"127.0.0.1:3000"` — loopback only (useful when nginx is on the same host)
    ///
    /// The port is not actually bound until [`serve`][Server::serve] is called.
    ///
    /// # Panics
    ///
    /// Panics if `addr` is not a valid socket address string.
    pub fn bind(addr: &str) -> Self {
        let addr: SocketAddr = addr.parse().expect("invalid socket address");
        Self { addr }
    }

    /// Binds the port, starts accepting connections, and dispatches requests
    /// through `router`.
    ///
    /// Blocks until the process receives `SIGTERM` or `Ctrl-C`. On shutdown:
    /// 1. The accept loop stops — no new connections are accepted.
    /// 2. In-flight requests are allowed to finish.
    /// 3. The function returns.
    ///
    /// For Kubernetes: set `terminationGracePeriodSeconds` in your pod spec to
    /// a value longer than your slowest request. If k8s sends `SIGKILL` before
    /// the drain completes, in-flight requests are dropped — that is not
    /// graceful shutdown.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] if the TCP listener cannot bind to the address, or if
    /// a fatal accept error occurs.
    pub async fn serve(self, router: Router) -> Result<(), Error> {
        let listener = TcpListener::bind(self.addr).await?;
        let router = Arc::new(router);

        let mut tasks = tokio::task::JoinSet::new();
        let shutdown = shutdown_signal();
        tokio::pin!(shutdown);

        loop {
            tokio::select! {
                biased;

                () = &mut shutdown => {
                    break;
                }

                res = listener.accept() => {
                    let (stream, _remote_addr) = match res {
                        Ok(v) => v,
                        Err(_) => continue,
                    };
                    let router = Arc::clone(&router);
                    tasks.spawn(async move {
                        let _ = serve_connection(stream, router).await;
                    });
                }

                Some(_) = tasks.join_next(), if !tasks.is_empty() => {}
            }
        }

        while tasks.join_next().await.is_some() {}
        Ok(())
    }
}

// ── Connection handler ────────────────────────────────────────────────────────

/// Serves all requests on one TCP connection.
///
/// Loops until nginx closes the connection (EOF). nginx controls connection
/// lifetime via `keepalive_timeout` and `keepalive_requests` in the upstream
/// block — astor never inspects the `Connection` header.
async fn serve_connection(stream: TcpStream, router: Arc<Router>) -> Result<(), Error> {
    let (read_half, mut write_half) = stream.into_split();
    let mut reader = BufReader::new(read_half);

    loop {
        // ── Request line ──────────────────────────────────────────────────────
        let mut line = String::new();
        if reader.read_line(&mut line).await? == 0 {
            break; // peer closed connection
        }
        let line = line.trim_end();
        let mut parts = line.splitn(3, ' ');
        let method_str = parts.next().unwrap_or("");
        let raw = parts.next().unwrap_or("/");
        let (path, query) = match raw.find('?') {
            Some(i) => (&raw[..i], &raw[i + 1..]),
            None    => (raw, ""),
        };
        let path = path.to_owned();
        let query = query.to_owned();
        let Ok(method) = method_str.parse::<Method>() else { break };
        // HTTP version field ignored — nginx guarantees HTTP/1.1

        // ── Headers ───────────────────────────────────────────────────────────
        let mut headers: Vec<(String, String)> = Vec::new();
        loop {
            let mut hline = String::new();
            reader.read_line(&mut hline).await?;
            let hline = hline.trim_end();
            if hline.is_empty() { break; }
            if let Some((name, value)) = hline.split_once(": ") {
                headers.push((name.to_owned(), value.to_owned()));
            }
        }

        // ── Body ──────────────────────────────────────────────────────────────
        let body = read_body(&mut reader, &headers).await?;

        // ── Dispatch ──────────────────────────────────────────────────────────
        let response = match router.lookup(method, &path) {
            Some((handler, middleware, params)) => {
                let req = Request::new(body, headers, method, params, path, query);
                Next::new(middleware, handler).call(req).await
            }
            None => Response::status(Status::NotFound),
        };

        response.write_to(&mut write_half).await?;
    }

    Ok(())
}

// ── Body readers ─────────────────────────────────────────────────────────────

async fn read_body<R: AsyncBufReadExt + Unpin>(
    reader: &mut R,
    headers: &[(String, String)],
) -> Result<Vec<u8>, Error> {
    if let Some(len) = headers.iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, v)| v.trim().parse::<usize>().ok())
    {
        let mut buf = vec![0u8; len];
        reader.read_exact(&mut buf).await?;
        return Ok(buf);
    }

    Ok(Vec::new())
}

// ── Shutdown signal ───────────────────────────────────────────────────────────

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl-C handler");
    };

    #[cfg(unix)]
    let sigterm = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let sigterm = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c  => {}
        () = sigterm => {}
    }
}
