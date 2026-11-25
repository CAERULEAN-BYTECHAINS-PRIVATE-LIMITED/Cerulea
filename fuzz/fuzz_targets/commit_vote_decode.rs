#![no_main]
//! The commit vote decode path.
//!
//! Every validator sends one vote per block to the author of the slot. The
//! target decodes the vote and runs the signature check the collector runs.

use cerulea_consensus::engines::commit::collect::{check_vote, vote_payload, CommitVote};
use codec::Decode;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(vote) = CommitVote::decode(&mut &data[..]) {
        let _ = vote_payload(vote.slot, &vote.block_hash);
        let _ = check_vote(&vote);
    }
});
