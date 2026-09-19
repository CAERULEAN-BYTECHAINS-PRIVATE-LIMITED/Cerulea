#![no_main]
//! The block seal decode path.
//!
//! A header arrives carrying a seal in its digest. The bytes below stand for
//! that seal. The target decodes the claim and the signature. A decode that
//! fails answers with an error. A decode that succeeds hands back a claim the
//! Authoring Engine then reads. Neither path panics.

use cerulea_consensus::engines::authoring::seal::{BlockSeal, SealClaim};
use codec::Decode;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(claim) = SealClaim::decode(&mut &data[..]) {
        let _ = claim.slot;
        let _ = claim.rank;
        let _ = claim.author;
    }
    if let Ok(seal) = BlockSeal::decode(&mut &data[..]) {
        let _ = seal.digest_item();
    }
});
