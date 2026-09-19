//! The shared parts of the Authoring Engine.
//!
//! The Authoring Engine draws the committee for each slot and names the author.
//! Three derivations have to match on every node and inside the runtime.
//!
//! 1. **The slot seed.** Every node derives the seed of a slot from the parent
//!    block hash and the slot number.
//! 2. **The epoch draw.** Each active validator evaluates a verifiable random
//!    function over the epoch number and its own draw key. A verifiable random
//!    function produces a random value beside a proof. Anyone checks the proof
//!    with the public key alone. The validator publishes the output and the
//!    proof on chain before the epoch opens.
//! 3. **The seat value.** For one slot the seat value of a validator mixes the
//!    published epoch draw with the slot seed and then scales the result by the
//!    Trust Score. The five lowest seat values form the committee in rank
//!    order.
//!
//! Splitting the draw from the slot seed keeps the derivation free of a
//! circular wait. The epoch number is known long before the epoch opens, so a
//! validator publishes its draw in good time. The slot seed follows the parent
//! hash, so the committee of the next slot stays unpredictable.

use alloc::vec::Vec;
use codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_core::crypto::VrfPublic;
use sp_core::hashing::blake2_256;
use sp_core::sr25519::vrf::{VrfSignData, VrfSignature, VrfTranscript};
use sp_core::sr25519::Public;

/// The number of validators drawn into the committee for one slot.
pub const COMMITTEE_SIZE: usize = 5;

/// The highest Trust Score, in parts per million.
pub const TRUST_SCORE_FULL: u32 = 1_000_000;

/// The floor of the standing scale, in parts per million.
///
/// The floor keeps a validator with no standing in the draw. A validator with
/// no standing still wins a seat whenever its raw value lands low enough.
pub const STANDING_FLOOR: u128 = (TRUST_SCORE_FULL / 3) as u128;

/// The separator mixed into every derived slot seed.
pub const SEED_DERIVATION_DOMAIN: &[u8] = b"cerulea.authoring.seed.v1";

/// The separator mixed into every seat value.
pub const SEAT_MIX_DOMAIN: &[u8] = b"cerulea.authoring.seat.v1";

/// The label of the Cerulea draw transcript.
pub const DRAW_LABEL: &[u8] = b"cerulea.authoring.draw.v1";

/// The context the draw output bytes are taken under.
pub const DRAW_OUTPUT_CONTEXT: &[u8] = b"cerulea.authoring.draw.output.v1";

/// The domain of the epoch number inside the draw transcript.
pub const EPOCH_DOMAIN: &[u8] = b"epoch";

/// Derive the seed of one slot from the parent block hash.
///
/// Every node holds the parent hash and the slot number, so every node derives
/// the same seed without coordination. A new parent gives a new seed, so the
/// committee of the next slot stays unpredictable.
pub fn derive_slot_seed(parent_hash: &[u8], slot: u64) -> [u8; 32] {
    let mut preimage = Vec::with_capacity(SEED_DERIVATION_DOMAIN.len() + parent_hash.len() + 8);
    preimage.extend_from_slice(SEED_DERIVATION_DOMAIN);
    preimage.extend_from_slice(parent_hash);
    preimage.extend_from_slice(&slot.to_be_bytes());
    blake2_256(&preimage)
}

/// The transcript one epoch draw signs.
pub fn epoch_draw_transcript(epoch: u32) -> VrfTranscript {
    let epoch_bytes = epoch.to_be_bytes();
    VrfTranscript::new(DRAW_LABEL, &[(EPOCH_DOMAIN, &epoch_bytes[..])])
}

/// The signing data one epoch draw signs.
pub fn epoch_draw_sign_data(epoch: u32) -> VrfSignData {
    VrfSignData::new(epoch_draw_transcript(epoch))
}

/// Check one published epoch draw and read its output.
///
/// The check reads the validator draw key alone. A proof that fails the check
/// yields nothing. The caller then rejects the publication and asks the
/// validator to publish a draw made with its registered draw key.
pub fn verify_epoch_draw(
    draw_key: &Public,
    epoch: u32,
    signature: &VrfSignature,
) -> Option<[u8; 32]> {
    let data = epoch_draw_sign_data(epoch);
    if !draw_key.vrf_verify(&data, signature) {
        return None;
    }
    draw_key
        .make_bytes::<32>(
            DRAW_OUTPUT_CONTEXT,
            &epoch_draw_transcript(epoch),
            &signature.pre_output,
        )
        .ok()
}

/// Read a draw output as a number.
///
/// The first sixteen bytes read as one big endian number. A lower number wins a
/// lower committee rank.
pub fn draw_value(output: &[u8; 32]) -> u128 {
    let mut head = [0u8; 16];
    head.copy_from_slice(&output[..16]);
    u128::from_be_bytes(head)
}

/// Scale one raw draw value by the standing of its validator.
///
/// The factor runs from one at zero standing down to one quarter at full
/// standing. A validator with full standing wins a seat far more often. A
/// validator with no standing keeps a real chance, which is what lets a new
/// validator build standing through service.
///
/// The arithmetic stays exact across the whole number range. The quotient and
/// the remainder scale on their own, so no product runs past the range.
pub fn weighted_draw(raw: u128, standing_parts_per_million: u32) -> u128 {
    let full = TRUST_SCORE_FULL as u128;
    let standing = (standing_parts_per_million.min(TRUST_SCORE_FULL)) as u128;
    let numerator = full.saturating_sub(standing).saturating_add(STANDING_FLOOR);
    let denominator = full.saturating_add(STANDING_FLOOR);
    let whole = (raw / denominator).saturating_mul(numerator);
    let part = (raw % denominator).saturating_mul(numerator) / denominator;
    whole.saturating_add(part)
}

/// The seat value of one validator for one slot.
///
/// The published epoch draw mixes with the slot seed. The mixed value scales by
/// the Trust Score. A lower seat value wins a lower rank.
pub fn seat_value(
    epoch_output: &[u8; 32],
    slot_seed: &[u8; 32],
    standing_parts_per_million: u32,
) -> u128 {
    let mut preimage = Vec::with_capacity(SEAT_MIX_DOMAIN.len() + 64);
    preimage.extend_from_slice(SEAT_MIX_DOMAIN);
    preimage.extend_from_slice(epoch_output);
    preimage.extend_from_slice(slot_seed);
    let mixed = blake2_256(&preimage);
    weighted_draw(draw_value(&mixed), standing_parts_per_million)
}

/// One candidate entering the seat ranking.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, TypeInfo, MaxEncodedLen)]
pub struct SeatInput {
    /// The sr25519 draw key of the validator.
    pub draw_key: [u8; 32],
    /// The output of the epoch draw the validator published.
    pub epoch_output: [u8; 32],
    /// The Trust Score of the validator, in parts per million.
    pub standing: u32,
}

/// One seat of one slot committee.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, TypeInfo, MaxEncodedLen)]
pub struct CommitteeSeat {
    /// The sr25519 draw key of the validator holding the seat.
    pub draw_key: [u8; 32],
    /// The rank the validator holds. Rank one authors.
    pub rank: u8,
    /// The seat value after scaling by standing.
    pub seat_value: u128,
}

/// Rank the candidates and take the committee for one slot.
///
/// A tie on the seat value settles on the draw key bytes, so every node reaches
/// the same rank order. The five lowest values form the committee.
pub fn rank_seats(candidates: &[SeatInput], slot_seed: &[u8; 32]) -> Vec<CommitteeSeat> {
    let mut scored: Vec<(u128, [u8; 32])> = candidates
        .iter()
        .map(|candidate| {
            (
                seat_value(&candidate.epoch_output, slot_seed, candidate.standing),
                candidate.draw_key,
            )
        })
        .collect();
    scored.sort_by(|left, right| left.0.cmp(&right.0).then(left.1.cmp(&right.1)));
    scored.dedup_by(|left, right| left.1 == right.1);
    scored
        .into_iter()
        .take(COMMITTEE_SIZE)
        .enumerate()
        .map(|(position, (seat_value, draw_key))| CommitteeSeat {
            draw_key,
            rank: (position as u8).saturating_add(1),
            seat_value,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seat(key: u8, output: u8, standing: u32) -> SeatInput {
        SeatInput {
            draw_key: [key; 32],
            epoch_output: [output; 32],
            standing,
        }
    }

    #[test]
    fn the_slot_seed_follows_the_parent_hash_and_the_slot() {
        let one = derive_slot_seed(&[1u8; 32], 7);
        let two = derive_slot_seed(&[2u8; 32], 7);
        let three = derive_slot_seed(&[1u8; 32], 8);
        assert_ne!(one, two);
        assert_ne!(one, three);
        assert_eq!(one, derive_slot_seed(&[1u8; 32], 7));
    }

    #[test]
    fn the_draw_value_reads_the_leading_bytes() {
        let mut output = [0u8; 32];
        output[0] = 1;
        assert_eq!(draw_value(&output), 1u128 << 120);
        assert_eq!(draw_value(&[0u8; 32]), 0);
        assert_eq!(draw_value(&[0xff; 32]), u128::MAX);
    }

    #[test]
    fn standing_lowers_the_weighted_value() {
        let raw = u128::MAX / 2;
        let none = weighted_draw(raw, 0);
        let half = weighted_draw(raw, TRUST_SCORE_FULL / 2);
        let full = weighted_draw(raw, TRUST_SCORE_FULL);
        assert!(full < half);
        assert!(half < none);
    }

    #[test]
    fn a_standing_above_the_ceiling_holds_at_the_ceiling() {
        let raw = 1_000_000_000u128;
        assert_eq!(
            weighted_draw(raw, TRUST_SCORE_FULL),
            weighted_draw(raw, TRUST_SCORE_FULL * 4)
        );
    }

    #[test]
    fn the_seat_value_follows_the_slot_seed() {
        let output = [9u8; 32];
        let one = seat_value(&output, &[1u8; 32], 0);
        let two = seat_value(&output, &[2u8; 32], 0);
        assert_ne!(one, two);
    }

    #[test]
    fn the_committee_holds_five_seats_in_rank_order() {
        let candidates: Vec<SeatInput> = (1..=12u8).map(|n| seat(n, n, 500_000)).collect();
        let committee = rank_seats(&candidates, &[4u8; 32]);
        assert_eq!(committee.len(), COMMITTEE_SIZE);
        for (position, member) in committee.iter().enumerate() {
            assert_eq!(member.rank as usize, position + 1);
        }
        for pair in committee.windows(2) {
            assert!(pair[0].seat_value <= pair[1].seat_value);
        }
    }

    #[test]
    fn a_short_candidate_list_gives_a_short_committee() {
        let candidates: Vec<SeatInput> = (1..=3u8).map(|n| seat(n, n, 0)).collect();
        assert_eq!(rank_seats(&candidates, &[1u8; 32]).len(), 3);
    }

    #[test]
    fn an_empty_candidate_list_gives_an_empty_committee() {
        assert!(rank_seats(&[], &[1u8; 32]).is_empty());
    }

    #[test]
    fn every_node_ranks_the_same_candidates_the_same_way() {
        let candidates: Vec<SeatInput> = (1..=9u8)
            .map(|n| seat(n, n.wrapping_mul(3), 250_000))
            .collect();
        let seed = derive_slot_seed(&[7u8; 32], 42);
        let first = rank_seats(&candidates, &seed);
        let mut shuffled = candidates.clone();
        shuffled.reverse();
        let second = rank_seats(&shuffled, &seed);
        assert_eq!(first, second);
    }

    #[test]
    fn a_duplicate_draw_key_takes_one_seat() {
        let candidates = alloc::vec![seat(5, 1, 0), seat(5, 2, 0), seat(6, 3, 0)];
        let committee = rank_seats(&candidates, &[1u8; 32]);
        let keys: Vec<[u8; 32]> = committee.iter().map(|member| member.draw_key).collect();
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&[5u8; 32]));
        assert!(keys.contains(&[6u8; 32]));
    }

    #[test]
    fn a_published_draw_checks_against_the_draw_key() {
        use sp_core::crypto::VrfSecret;
        use sp_core::sr25519::Pair;
        use sp_core::Pair as PairTrait;

        let pair = Pair::from_seed(&[3u8; 32]);
        let signature = pair.vrf_sign(&epoch_draw_sign_data(4));
        let output = verify_epoch_draw(&pair.public(), 4, &signature)
            .expect("the proof checks against the draw key");
        let other = Pair::from_seed(&[4u8; 32]).public();
        assert!(verify_epoch_draw(&other, 4, &signature).is_none());
        assert!(verify_epoch_draw(&pair.public(), 5, &signature).is_none());
        assert_eq!(
            verify_epoch_draw(&pair.public(), 4, &signature),
            Some(output)
        );
    }
}
