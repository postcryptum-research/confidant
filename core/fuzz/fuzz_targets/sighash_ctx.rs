//! The signature-domain helpers take caller strings (destinations, external context): any input
//! must give bytes, never a panic, and the proof-of-funds domain never collides with a pool sighash.
#![no_main]
use confidant_core::sighash::{external_ctx, pof_ctx, pof_sighash, sighash, unshield_ctx, OpTag};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.len() < 18 {
        return;
    }
    let op = OpTag::from_byte(data[0]);
    let amount = u64::from_le_bytes(data[1..9].try_into().unwrap());
    let balance = i64::from_le_bytes(data[9..17].try_into().unwrap());
    let text = String::from_utf8_lossy(&data[17..]);
    let commitment = [0x11u8; 32];
    let a = unshield_ctx(&text, amount);
    let b = pof_ctx(text.as_bytes(), amount);
    let _ = pof_sighash(&commitment, &b);
    if let Some(op) = op {
        // `external_ctx` owes its caller the sign check (audit ruling on PR #3: a sighash is
        // computed correctly or not at all, and `Verifier::verify` refuses a wrong-signed bundle
        // before it gets here). The harness mirrors that precondition instead of removing it:
        // an unshield context is only asked for a non-negative balance.
        if op != OpTag::Unshield || balance >= 0 {
            let c = external_ctx(op, &text, balance);
            let _ = sighash(&commitment, op, &c);
        }
        if op == OpTag::Unshield {
            let _ = sighash(&commitment, op, &a);
        }
    }
    // The two context encodings share one shape on purpose (len ‖ bytes ‖ amount); what keeps the
    // domains apart is the sighash personalization and the op tag. Pin that: for the same
    // commitment and the same bytes the two digests never coincide.
    if a == b {
        assert_ne!(
            pof_sighash(&commitment, &b),
            sighash(&commitment, OpTag::Unshield, &a),
            "a proof-of-funds sighash never equals an unshield sighash over the same context"
        );
    }
});
