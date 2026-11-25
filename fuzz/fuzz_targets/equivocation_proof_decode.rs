#![no_main]
//! The equivocation proof decode path.
//!
//! Any node may report two blocks sealed by one validator at one slot. The
//! target decodes the report and runs the three conditions the check runs.

use cerulea_primitives::{verify_equivocation, EquivocationProof};
use codec::Decode;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(proof) = EquivocationProof::decode(&mut &data[..]) {
        let _ = proof.first.payload();
        let _ = proof.second.payload();
        let _ = verify_equivocation(&proof);
    }
});
