//! A bundle record is what `POST /submit/*`, `/endorse` and the public log carry: untrusted JSON.
//! Parsing, re-building the Orchard bundle, hashing it and replaying it into an empty verifier
//! must never panic — every malformed input is a typed error.
#![no_main]
use confidant_core::record::{bundle_from_record, bundle_hash, canonical_bytes};
use confidant_core::verifier::BUNDLE_VERSION;
use confidant_core::{BundleRecord, Verifier};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(record) = serde_json::from_slice::<BundleRecord>(data) else {
        return;
    };
    let _ = canonical_bytes(&record);
    let _ = bundle_hash(&record);
    let _ = bundle_from_record(&record, BUNDLE_VERSION);
    let mut v = Verifier::new();
    let _ = v.replay(&record);
    // Whatever happened, the verifier is still consistent and serializable.
    let _ = v.state();
    let _ = serde_json::to_string(&v.snapshot()).expect("a verifier snapshot serializes");
});
