#![no_main]
//! The state roots decode path.
//!
//! A block carries the per-domain roots and the combined root in its digest.
//! An importing node decodes both and checks that the combined root covers the
//! domain roots.

use cerulea_consensus::engines::execution::roots::StateRoots;
use codec::Decode;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(roots) = StateRoots::decode(&mut &data[..]) {
        let _ = roots.consistent();
        let _ = roots.len();
        let _ = roots.digest_item();
    }
});
