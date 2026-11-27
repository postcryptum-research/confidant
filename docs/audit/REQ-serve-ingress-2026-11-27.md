# REQ — shared HTTP helper: defensive request-target and body parsing

Sent 2026-11-27 from the implementation session to the audit session. Scope: a follow-on slice of the
"Self-security" task (fuzz Confidant's own input surfaces and harden what the internet can reach, after
PR #118). This slice hardens the one shared HTTP helper the Node services run on. No trust-path change:
the verifier, the apply paths, the sidecar rule, the fee/credential/PoF semantics and every handler body
are untouched; this is ingress discipline in `serve()` only. Branch `self-security-ingress-hardening`.

```text
== Surface ==
backend/quorum/lib.mjs serve(port, routes) is the HTTP front of the coordinator (reachable through nginx
at /api/) and of the signer sidecars (loopback only). The verifier nodes (Rust/axum) and the demo issuer
(issuer.mjs, its own server) do not use serve() and are out of scope here.

== Change (serve() only) ==
1. Request target: serve() parsed req.url with new URL() BEFORE its try/catch. The WHATWG URL parser
   throws on some malformed targets; the parse now runs inside a guard and a malformed target is
   answered 400 ("malformed request target"). The handler is reached only with a parsed URL.
2. Request body: a POST body that is not valid JSON, or that parses to something other than a JSON
   object (null, an array, a scalar), is now a 400 with a fixed message ("body is not valid JSON" /
   "body must be a JSON object") — not a 500 that surfaces the JSON parser's own text, and not a body a
   handler would fail to destructure. The 1 MiB cap (413) and the empty-body-as {} convention are
   unchanged; a well-formed object body reaches the handler exactly as before.

Net: every handler still receives (body, url, req) with body a parsed object (or null for GET); the only
behavioural change is that malformed input is a 4xx from serve() instead of reaching a handler or the
generic 500 path. No route, no handler, no signing rule, no ledger interaction changed.

== Also in this branch (not serve()) ==
- CI: the coverage job's Rust toolchain is pinned (dtolnay/rust-toolchain@1.95.0 instead of @stable).
  llvm-cov line counting shifts between rustc releases; the 100%-core-lines gate flapped red on the
  stable that shipped after 1.95, with the core itself unchanged. Locally the gate is 100% on 1.94 and
  1.95. This is CI config only; no source, no coverage-threshold change.

== Tests ==
backend/quorum/test/serve.test.mjs: starts serve() on an ephemeral port and, over raw sockets, sends
malformed request targets (//, ///, /\, /\\, http://[, //[, http://) and malformed bodies ({{{ , not
JSON, truncated, null, [], a bare number, "x", true). Asserts: each is 400; the server keeps answering
200 on a good route after the whole battery (it did not drop the connection or stop); malformed bodies
never reach the handler and never echo parser internals; a well-formed object body does reach it.
Full quorum suite: 29/29 (28 prior + this).

== Questions ==
Q1 serve() now rejects a non-object JSON body (array/scalar/null) with 400. Every current handler expects
   an object, so this is strictly tighter; confirm no caller relies on a non-object body.
Q2 The 400 messages are fixed strings (no request echo). Enough, or do you want a request id for
   operator correlation in logs?
```
