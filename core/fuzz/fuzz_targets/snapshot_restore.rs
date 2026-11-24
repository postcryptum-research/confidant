//! A key-free snapshot is read from disk at node start and from `GET /log` mirrors: untrusted
//! JSON. `restore` must refuse malformed input with an error, never panic.
#![no_main]
use confidant_core::{Verifier, VerifierSnapshot};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(snapshot) = serde_json::from_slice::<VerifierSnapshot>(data) else {
        return;
    };
    if let Ok(v) = Verifier::restore(&snapshot) {
        let again = Verifier::restore(&v.snapshot()).expect("a restored verifier's snapshot restores");
        assert_eq!(again.state().root, v.state().root);
    }
});
