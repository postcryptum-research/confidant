# REQ — web-ingress hardening and fuzz harnesses (node, wallet service, coordinator, nginx templates)

Sent 2026-11-24 from the implementation session to the audit session. Scope: the "Self-security" task
(the project's private ops ticket): fuzz Confidant's own input surfaces and harden what the internet can
reach. No trust-path change: the verifier, the apply paths, the sidecar rule and the credential/PoF
semantics are untouched; this is ingress discipline plus harnesses. Branch `self-security-fuzz`.

```text
== Surfaces, as reachable through nginx on the demo site ==
/api/* → coordinator (Node) → its node; /issuer/* (hardened on 2026-10-25, #100/#101); /wallet/* → the demo
wallet service (Rust, server-held demo keys; the browser WASM wallet is the default, the service is the
fallback); /node/N/pof/verify|state → the Rust node read side; static pages. Direct :8787 (plain HTTP, CORS *)
was also reachable — the templates now bind it to loopback.

== Changes ==
1. Node router (backend/node/src/http.rs): DefaultBodyLimit 1 MiB on every route (the PoF route keeps its 256 KiB),
   CatchPanicLayer (a handler panic answers 500 instead of dropping the connection; the verifier lock was already
   poison-tolerant, so a panic never took the node down — now it also never looks like a network error),
   TimeoutLayer 60 s (408), ConcurrencyLimitLayer 64. Constants BODY_LIMIT / REQUEST_TIMEOUT / MAX_IN_FLIGHT.
2. Wallet service (backend/wallet): Store capacity (DEFAULT_CAPACITY 1000, WALLET_CAP env, with_capacity ≥ 1);
   create() refuses with StoreError::Full → 503 before touching the disk; router gets DefaultBodyLimit 64 KiB,
   CatchPanic, Timeout 120 s, ConcurrencyLimit 16.
3. Coordinator (backend/quorum/coordinator.mjs): POST /fees/sweep — the one value-moving operator action — requires
   X-Operator-Token equal (timingSafeEqual) to OPERATOR_TOKEN; with no token configured the route is closed (403).
   lib.mjs passes the request to handlers and http() takes headers; render-env.mjs writes a random 32-byte
   OPERATOR_TOKEN into coordinator.env; the services e2e harness sets it, sends it, and asserts the refusal without it.
4. nginx templates (backend/deploy/nginx): confidant.conf listens on 127.0.0.1:8787 only; new
   confidant-ratelimits.conf (zones api 30r/m, pof 10r/m, wallet 20r/m, perip conn); confidant-ui-tls.conf: 403 on
   /api/fees/sweep, limit_req + limit_conn on /api, /wallet, /node (pof body 300k), server_tokens off, client
   timeouts, security headers (nosniff, DENY, Referrer-Policy, Permissions-Policy, HSTS 1 y) and a CSP in
   Report-Only mode, repeated inside `location /` (add_header is not inherited where a location sets its own).
5. Docs: deploy README (sweep with the token), NODE-OPERATOR (OPERATOR_TOKEN), API-BOUNDARY EN/RU, SPEC EN/RU, README
   and CLAUDE.md (fuzz commands).

== Tests ==
- backend/node/tests/node.rs: http_surface_survives_malformed_oversized_and_mistyped_input_without_5xx_or_state_change —
  every POST route × {empty, null, array, object, string, 1e400, truncated JSON, invalid UTF-8, 2000-deep nesting,
  wrong types, u64::MAX / i64::MIN everywhere, 10–100 KB strings, 3 MiB body} + non-JSON content type + wrong
  method; every GET route with bad query values and DELETE: never a 5xx, /state byte-identical after. Passes on main
  as well (the surface was already clean); it pins the property. Node coverage gate 95 % holds.
- backend/wallet/tests/wallet.rs: capacity_is_enforced_and_the_http_surface_survives_malformed_input — Full at the
  cap, 503 mapping, clamp to ≥ 1, the same malformed sweep over every wallet route. Wallet 3/3.
- backend/quorum npm test 28/28 (the sweep gate is exercised by the services e2e harness, which needs testnet).

== Fuzz harnesses (cargo-fuzz, libFuzzer, nightly; standalone crates excluded from the workspace) ==
core/fuzz: memo_decode (AnchorMemo::decode + decode_hex, encode fixed point), record_json (serde → bundle_from_record
→ canonical_bytes/bundle_hash → Verifier::replay on an empty verifier; state and snapshot still serializable),
snapshot_restore (serde → Verifier::restore → snapshot → restore again), sighash_ctx (OpTag::from_byte, the three
context builders, both sighashes; pins that a proof-of-funds sighash never equals an unshield sighash over the same
context).
backend/node/fuzz: address_canonical (canonical() on any string; a canonical address is a fixed point), pof_challenge
(serde → validate → bytes → check_window → credential, in the node's order), pof_ledger_objects
(outstanding_pof_credentials / is_pof_credential_from on arbitrary JSON; count ≤ objects).
Results (3 min each, -max_len 4–64 KiB, debug assertions on): memo_decode 114 k exec/s cov 274, no finding;
record_json 42 k exec/s cov 1618, no finding; snapshot_restore 13 k exec/s cov 1311, no finding; sighash_ctx 40 k
exec/s cov 140, no finding; address_canonical 13 k exec/s cov 203, no finding; pof_challenge 40 k exec/s cov 1335,
no finding; pof_ledger_objects 46 k exec/s cov 1551, no finding.
One harness false positive worth recording: the first sighash_ctx run tripped the `expect` in
`external_ctx(Unshield, _, negative)` (core/src/sighash.rs:114). That expect is the deliberate, audited precondition
from PR #3 (callers establish the sign first; `Verifier::verify` does at verifier.rs:246 before :285), so it is not
reachable from any ingress; the harness now mirrors the precondition instead of removing it, and the core is
unchanged.

== Questions ==
Q1 The limits (1 MiB / 64 KiB bodies, 60 s / 120 s, 64 / 16 in flight; nginx 30r/m api, 10r/m pof, 20r/m wallet)
   are sized from the demo's traffic; anything you want tighter or looser before they go to the host?
Q2 The operator token is a shared secret in the coordinator's env file (0640, service user). Acceptable for the
   demo pool, or do you want the sweep moved off the HTTP surface entirely (a local socket / CLI on the host)?
Q3 The wallet service with server-held demo keys stays reachable through /wallet/ as the no-WASM fallback. Keep it
   (capped, rate-limited) or recommend closing the location when the WASM path is the default?
```
