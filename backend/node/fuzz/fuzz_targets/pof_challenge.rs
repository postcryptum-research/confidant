//! The proof-of-funds challenge is caller JSON (`POST /pof/verify`): parsing, validation, the
//! recency window, the canonical bytes and the credential derivation (expiry arithmetic) must
//! never panic, in the order the node applies them.
#![no_main]
use confidant_node::pof::PofChallenge;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.len() < 24 {
        return;
    }
    let amount = u64::from_le_bytes(data[0..8].try_into().unwrap());
    let validated = u64::from_le_bytes(data[8..16].try_into().unwrap());
    let close_time = u64::from_le_bytes(data[16..24].try_into().unwrap());
    let Ok(challenge) = serde_json::from_slice::<PofChallenge>(&data[24..]) else {
        return;
    };
    if challenge.validate().is_err() {
        return;
    }
    let _ = challenge.bytes(amount);
    if challenge.check_window(validated).is_err() {
        return;
    }
    let _ = challenge.credential(amount, close_time);
});
