# RESP — web-ingress hardening and fuzz harnesses (PR #118) — IMPL-PASS

Received 2026-11-24 from the audit session, committed verbatim. Ruling: **IMPL-PASS at b448537**, merge-clear on the audit axis, no changes required.

```text
IMPL-PASS for PR #118 at b448537 — verified firsthand, merge-clear on my side, no changes required on the security axes.

**The load-bearing claim — no trust-path change — confirmed three ways:**
1. The 40-file changed list contains no verifier/apply/credential/PoF core source.
2. backend/node/src/http.rs is purely additive — the 4 ingress layers (DefaultBodyLimit, CatchPanic, Timeout, ConcurrencyLimit) and 3 constants, zero deletions, the submit/verify/endorse/anchored/reveal handler bodies byte-unchanged. The layers wrap the handlers; they don't alter them.
3. The fuzz crates are standalone (workspace-excluded), and sighash_ctx MIRRORS the core's audited precondition (`op != Unshield || balance >= 0`, the PR #3 ruling that Verifier::verify establishes the sign before external_ctx) rather than deleting the core's expect. That's the correct handling — you respected the caller-contract instead of weakening the core to satisfy the fuzzer. Core untouched.

**Sweep gate — the one value-moving action, solid:**
- requireOperator(req) is invoked at the route top (not a dead gate), fail-closed (no OPERATOR_TOKEN → 403), constant-time (length check then timingSafeEqual).
- And the fund-safety doesn't even rest on the token: Destination is pinned to the node's configured treasury (fees.treasury from /fees, never the request body), Amount is bounded to fees.available, and each quorum signer re-verifies the fee-sweep against its own fee ledger. So a leaked token can at worst force a *legitimate* sweep to the treasury — it can never redirect funds. The token is a nuisance/DoS gate on top of real fund-safety.

**The rest:** ingress limits fail safe (413/408/503); CatchPanic→500 is fail-closed for the verifier (poison-tolerant lock; your node.rs test asserts every POST route survives malformed/oversized/mistyped input with no 5xx and /state byte-identical after — that's the property that matters); wallet create() checks capacity and returns Full→503 before touching disk, clamp ≥1. Fuzz coverage is non-hollow per your reported exec/cov.

**Your three questions:**

**Q1 (limits):** fine for the demo, and all fail-safe — nothing to tighten (no DoS from looseness at 1MiB/60s/64). One forward note, not a blocker: the 256 KiB PoF cap is sized from the typical two-action record (~19 KB). Before it's load-bearing for real holders, re-derive it from the *maximum* legitimate PoF record (a many-action holder), not the typical one, with margin — else a valid large proof gets 413'd. It fails safe (a rejected proof isn't a security hole), so it's a liveness note.

**Q2 (operator token on HTTP vs local socket):** acceptable as-is — and the reason is the pinned destination. Because the sweep can only pay the configured treasury, bounded and quorum-verified, the token's blast radius is zero theft; it's not what stands between an attacker and the funds. So I'm not asking you to move it. If the pool ever holds real value, moving the sweep to a local socket/CLI on the host is a defense-in-depth nicety (shrinks the surface, removes the bearer-token-over-HTTP replay/leak concern) — and if you keep it on HTTP then, add a nonce/expiry to kill replay. For the demo, leave it.

**Q3 (server-key wallet fallback):** keep it for the demo — demo keys, capped, rate-limited, behind TLS — but label it demo-only in the UI/docs (server-held keys, no real value) so no one reads the fallback as a custody model. Close or gate /wallet/ for any real-value deployment: a server holding wallet keys is a single-point custody surface, and the WASM client-side wallet is the default precisely to keep keys off the server. Keep for demo, close for production.

So: PASS, and the slice is sound ops discipline — the trust path is genuinely untouched, the one value action is gated and destination-pinned, and the fuzz story is honest (including how you recorded the sighash_ctx false positive). The merge is yours/the lead's; on the audit axis it's clear at b448537.
```

Follow-ups recorded from the ruling (not blockers): re-derive the 256 KiB proof-of-funds body cap from the maximum legitimate record before real holders depend on it; label the `/wallet/` fallback demo-only in the UI and docs (server-held demo keys, no real value) and close or gate it for any real-value deployment; if the sweep stays on HTTP with real value, add a nonce/expiry.
