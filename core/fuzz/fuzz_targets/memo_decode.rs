//! Anchor memos arrive from the ledger (any XRPL account can write a memo): decoding must never
//! panic, and a memo that decodes must re-encode to bytes that decode to the same memo.
#![no_main]
use confidant_core::memo::AnchorMemo;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(memo) = AnchorMemo::decode(data) {
        let bytes = memo.encode().expect("a decoded memo re-encodes");
        let again = AnchorMemo::decode(&bytes).expect("a re-encoded memo decodes");
        assert_eq!(again.encode().unwrap(), bytes, "encode is a fixed point");
    }
    let text = String::from_utf8_lossy(data);
    if let Ok(memo) = AnchorMemo::decode_hex(&text) {
        let _ = memo.encode_hex().expect("a decoded memo re-encodes as hex");
    }
});
