#![no_main]
//! The header digest decode path.
//!
//! The whole digest of an arriving header decodes before any engine reads it.
//! The target decodes the digest and asks the Authoring Engine and the Commit
//! Engine to read what they own from it.

use cerulea_consensus::engines::commit::aggregate::AggregateSignature;
use cerulea_consensus::engines::execution::roots::StateRoots;
use codec::Decode;
use libfuzzer_sys::fuzz_target;
use sp_runtime::generic::Digest;

fuzz_target!(|data: &[u8]| {
    if let Ok(digest) = Digest::decode(&mut &data[..]) {
        let _ = StateRoots::from_digest(&digest);
        let _ = AggregateSignature::from_digest(&digest);
    }
});
