# REQ — demo-pool re-provision runbook + gate-aware prepopulate

Sent 2026-11-27 from the implementation session to the audit session. Scope: operational recovery after a
public XRPL testnet reset (which wipes every account, so the demo pool must be rebuilt fresh). Off-chain
artifacts only — a deploy/ops script and the prepopulate helper. No trust-path change: the verifier, the
apply paths, the sidecar rule, the credential/PoF/fee semantics and every service's code are untouched.

```text
== Change ==
1. backend/deploy/reprovision.sh (new) — a host runbook: back up data+env; run the existing setup-pool.mjs
   (new pool/sink/treasury, SignerListSet k-of-N, master disabled); faucet a new KYC issuer; install the
   keys 0600 at the canonical paths; clear node verifier state; rewrite common.env (POOL/SINK/SIGNERS/
   treasury/issuer, keeping SCREENING_ARGS/POF_ARGS/QUORUM/RPC verbatim); restart the services; verify the
   pool is on the ledger. It calls only existing tools (setup-pool.mjs, systemctl); it creates no new
   signing path and changes no service code. It is a fresh start, not a restore (old bundles are gone with
   their accounts). Paths are env-overridable; defaults are the demo host's layout.
2. backend/quorum/prepopulate.mjs — gate-aware: when /config advertises a credential gate, each faucet
   funder now obtains the entry credential from the demo issuer service (POST ISSUER_URL/issue) and accepts
   it (CredentialAccept, funder's own key) BEFORE its deposit. Without this, prepopulate's deposits are
   refused 403 under the gate (the gate was added after prepopulate was written). ISSUER_URL defaults to
   the local issuer. Only the depositor-eligibility step is added; the shield/transfer flow is unchanged.
3. docs/NODE-OPERATOR.md — a "Recovering after a public XRPL testnet reset" section documenting the above.

== Why this is not a trust-path change ==
reprovision.sh orchestrates existing tools and systemd; it writes keys and env the way the original
deployment did. prepopulate.mjs is a demo convenience (synthetic, faucet-funded anonymity set, labelled as
such); the credential it obtains is the SAME gate every account uses, via the SAME issuer service — no new
authority, no pool key. Nothing in core/ or backend/node, no verifier/signer/coordinator code changed.

== Tests ==
reprovision.sh: `bash -n` clean; exercised end-to-end on the host 2026-11-27 (new pool live, custody
verified on the ledger, a credentialed shield anchored 2-of-3). prepopulate.mjs: `node --check` clean; the
gate-aware path was exercised by the same-shaped seed run on the host (5 shields + 3 transfers under the
gate). No unit test is added (both need a live pool + faucet; they are covered by the services e2e harness).

== Questions ==
Q1 reprovision.sh keeps SCREENING_ARGS/POF_ARGS verbatim from the prior common.env. Acceptable, or should a
   reset be the moment to re-confirm those (e.g. POF revealing flag) rather than carry them forward silently?
Q2 prepopulate obtains the credential through the issuer SERVICE (no issuer key on the prepopulate host).
   Fine, or do you want it to accept an issuer key directly for air-gapped prepopulation?
```
