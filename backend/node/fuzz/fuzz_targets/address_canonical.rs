//! Every XRPL address the node sees comes from a caller or the ledger (payout destinations,
//! credential subjects, screening lists): the base58 decoder and X-address unpacking must
//! never panic on any string.
#![no_main]
use confidant_node::canonical;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let text = String::from_utf8_lossy(data);
    if let Some(classic) = canonical(&text) {
        // A canonical address is a fixed point.
        assert_eq!(canonical(&classic).as_deref(), Some(classic.as_str()));
    }
});
