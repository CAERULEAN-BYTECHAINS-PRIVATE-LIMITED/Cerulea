#![no_main]
//! The Access Manifest decode path.
//!
//! Every transaction declares the state it reads and writes before execution.
//! The target decodes that declaration and runs the validation the Execution
//! Engine runs before partitioning.

use cerulea_consensus::engines::manifest::{AccessManifest, PendingTransaction};
use codec::Decode;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(manifest) = AccessManifest::decode(&mut &data[..]) {
        let _ = manifest.validate();
        let _ = manifest.written_collections();
        let _ = manifest.len();
    }
    if let Ok(pending) = PendingTransaction::decode(&mut &data[..]) {
        let _ = pending.manifest.validate();
    }
});
