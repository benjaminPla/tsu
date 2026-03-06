# todo

## backlog

- [ ] set up `cargo publish` pipeline
- [ ] remove all `unwrap()` and others similar, standarize an error
- [ ] chunked transfer encoding as opt-in feature (currently requires `proxy_buffering on`)

## ai suggestions

- **Named middleware groups** — a `MiddlewareStack` type that wraps a `Vec<BoxedMiddleware>` and implements `IntoMiddlewares` would let users define reusable groups (e.g., `let auth_stack = MiddlewareStack::new().push(require_auth).push(rate_limit)`) and pass them to multiple routes without repeating the tuple syntax.
- **Middleware error recovery** — a pattern/recipe in docs showing how a middleware can catch panics or map errors to HTTP 500 responses, since the current chain has no built-in error boundary.
- **`Router::middleware` ordering guard** — a debug-mode assertion (or at least a doc warning) that `.middleware()` has no effect on routes registered *before* the call, which is a common gotcha.
- **Async-fn middleware type alias** — exporting a `type MiddlewareFn = fn(Request, Next) -> Pin<Box<dyn Future<Output = Response> + Send>>` convenience alias so users don't have to write the boxed future signature in function pointer contexts.
- [x] double check if lowecase methods can bypass ngnix and handle it (better in the ngnix conf, not astor)
- [x] bug getting more than one query param, I think I need to store them in a hasmap again, because it can be multiple, also Option maybe
- [x] remove hyper
- [x] remove http
- [x] remove logs/tracing — that's the consumer's responsibility
- [x] clean README.md file (philosophy, reverse-proxy, etc) and refer to the docs - mainteining both is a mess
- [x] ensure `examples/` is excluded from the published crate
