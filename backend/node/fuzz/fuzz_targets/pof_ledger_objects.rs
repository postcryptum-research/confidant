//! `account_objects` answers come from the ledger RPC as arbitrary JSON: counting outstanding
//! proof-of-funds credentials must never panic on any shape.
#![no_main]
use confidant_node::pof::{is_pof_credential_from, outstanding_pof_credentials};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(data) else {
        return;
    };
    let pool = "rEiHipocKiZZ9UgriMRNpMu1jg7roGWcLV";
    let objects: Vec<serde_json::Value> = match value {
        serde_json::Value::Array(items) => items,
        other => vec![other],
    };
    let n = outstanding_pof_credentials(pool, &objects);
    assert!(n <= objects.len());
    for o in &objects {
        let _ = is_pof_credential_from(pool, o);
    }
});
