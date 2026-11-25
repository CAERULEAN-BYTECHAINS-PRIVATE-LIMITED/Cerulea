#![no_main]
//! The aggregate signature decode path.
//!
//! The author of a slot sends one aggregate to the network. The envelope
//! carries a signer bitmap and a signature list. The target decodes the
//! envelope and asks whether the bitmap is well formed.

use cerulea_consensus::engines::commit::aggregate::AggregateSignature;
use codec::Decode;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(aggregate) = AggregateSignature::decode(&mut &data[..]) {
        let _ = aggregate.signers.well_formed();
        let _ = aggregate.signer_count();
        let _ = aggregate.encoded_len();
        let _ = aggregate.signers.positions();
    }
});
