//! Committee rank ordering weighted by Trust Score.
//!
//! Each slot draws a committee of five validators. Every eligible validator
//! publishes the draw it produced for that slot together with the proof. The
//! Authoring Engine checks each proof, scales each output by the standing of
//! the validator, sorts the scaled values and takes the five lowest. Those five
//! form the committee in rank order. Rank one authors. Ranks two through five
//! stand by.
//!
//! # How standing scales a draw
//!
//! A raw draw output is a number spread evenly across its range. Scaling
//! multiplies that number by a factor that falls as standing rises. A lower
//! scaled value wins a lower rank.
//!
//! The factor runs from one at zero standing down to one quarter at full
//! standing. A validator with full standing therefore wins a rank far more
//! often than a validator with no standing. A validator with no standing still
//! wins whenever its raw draw lands low enough, so every eligible validator
//! keeps a real chance. Keeping that chance is what lets a new validator build
//! standing through service.

use sp_core::sr25519::Public;

use super::draw::{draw_value, verify_result, DrawInput, DrawResult};
use crate::engines::{
    AuthoringEngine, Committee, CommitteeMember, Engine, EngineError, EngineKind, Rank, TrustScore,
    COMMITTEE_SIZE,
};

/// The floor of the standing scale, in parts per million.
///
/// The floor is what keeps a validator with no standing in the draw. The scale
/// factor at full standing is the floor divided by the floor plus one million.
/// The shared primitives crate carries the value, so the runtime scales the
/// same way.
pub const STANDING_FLOOR: u128 = cerulea_primitives::STANDING_FLOOR;

/// The committee drawn for one slot.
pub type SlotCommittee = Committee<Public>;

/// Scale one raw draw output by the standing of its validator.
///
/// The arithmetic stays exact across the whole number range. The quotient and
/// the remainder scale on their own, so no product ever runs past the range.
pub fn weighted_draw(raw: u128, standing: TrustScore) -> u128 {
    cerulea_primitives::weighted_draw(raw, standing.parts_per_million())
}

/// The Authoring Engine committee builder.
#[derive(Clone, Copy, Debug, Default)]
pub struct CommitteeDraw;

impl CommitteeDraw {
    /// Build a committee builder.
    pub fn new() -> Self {
        CommitteeDraw
    }

    /// Assemble the committee for one slot from the published draws.
    ///
    /// Each draw carries its own proof. A draw that fails its proof leaves the
    /// committee and the call reports the problem. The five lowest scaled
    /// values form the committee in rank order.
    pub fn assemble(
        &self,
        input: &DrawInput,
        entries: &[(DrawResult, TrustScore)],
    ) -> Result<SlotCommittee, EngineError> {
        if entries.is_empty() {
            return Err(EngineError::EmptyValidatorSet);
        }
        let mut scaled: Vec<(u128, Public)> = Vec::with_capacity(entries.len());
        for (result, standing) in entries {
            verify_result(input, result)?;
            scaled.push((
                weighted_draw(draw_value(&result.output), *standing),
                result.validator,
            ));
        }
        Ok(rank_scaled(input, scaled))
    }

    /// Assemble the committee from scaled values that a caller already checked.
    ///
    /// The import path uses this after it checks the published draws once.
    pub fn rank(&self, input: &DrawInput, scaled: Vec<(u128, Public)>) -> SlotCommittee {
        rank_scaled(input, scaled)
    }
}

fn rank_scaled(input: &DrawInput, mut scaled: Vec<(u128, Public)>) -> SlotCommittee {
    // A tie on the scaled value settles on the validator key bytes, so every
    // node reaches the same rank order.
    scaled.sort_by(|left, right| left.0.cmp(&right.0).then(left.1 .0.cmp(&right.1 .0)));
    scaled.dedup_by(|left, right| left.1 == right.1);
    let members = scaled
        .into_iter()
        .take(COMMITTEE_SIZE)
        .enumerate()
        .map(|(position, (weighted_draw, validator))| CommitteeMember {
            validator,
            rank: Rank::from_index(position),
            weighted_draw,
        })
        .collect();
    Committee {
        slot: input.slot,
        members,
    }
}

impl Engine for CommitteeDraw {
    const KIND: EngineKind = EngineKind::Authoring;
}

impl AuthoringEngine for CommitteeDraw {
    type ValidatorId = Public;

    fn committee(
        &self,
        input: &DrawInput,
        entries: &[(DrawResult, TrustScore)],
    ) -> Result<Committee<Self::ValidatorId>, EngineError> {
        self.assemble(input, entries)
    }

    fn scheduled_author<'a>(
        &self,
        committee: &'a Committee<Self::ValidatorId>,
        rank: Rank,
    ) -> Option<&'a Self::ValidatorId> {
        committee.at_rank(rank)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engines::authoring::draw::draw;
    use crate::engines::{Slot, SlotSeed};
    use sp_core::sr25519::Pair;
    use sp_core::Pair as PairTrait;

    fn validator(seed: u8) -> Pair {
        Pair::from_seed(&[seed; 32])
    }

    fn input(slot: u64) -> DrawInput {
        DrawInput::new(Slot(slot), SlotSeed([5u8; 32]))
    }

    fn entries(count: u8, slot: u64, standing: TrustScore) -> Vec<(DrawResult, TrustScore)> {
        let input = input(slot);
        (1..=count)
            .map(|seed| (draw(&validator(seed), &input), standing))
            .collect()
    }

    #[test]
    fn a_committee_holds_five_members_in_rank_order() {
        let input = input(3);
        let entries = entries(12, 3, TrustScore::from_percent(50));
        let committee = CommitteeDraw::new()
            .assemble(&input, &entries)
            .expect("the committee assembles");
        assert_eq!(committee.len(), COMMITTEE_SIZE);
        for (position, member) in committee.members.iter().enumerate() {
            assert_eq!(member.rank, Rank::from_index(position));
        }
        let mut values: Vec<u128> = committee
            .members
            .iter()
            .map(|member| member.weighted_draw)
            .collect();
        let sorted = {
            let mut copy = values.clone();
            copy.sort_unstable();
            copy
        };
        assert_eq!(values, sorted, "the ranks run from the lowest value upward");
        values.dedup();
        assert_eq!(values.len(), COMMITTEE_SIZE);
    }

    #[test]
    fn a_smaller_set_takes_a_smaller_committee() {
        let input = input(3);
        let entries = entries(3, 3, TrustScore::from_percent(50));
        let committee = CommitteeDraw::new()
            .assemble(&input, &entries)
            .expect("the committee assembles");
        assert_eq!(committee.len(), 3);
        assert!(committee.at_rank(Rank(4)).is_none());
    }

    #[test]
    fn an_empty_set_names_the_fix() {
        let error = CommitteeDraw::new()
            .assemble(&input(1), &[])
            .expect_err("an empty set reports a problem");
        assert_eq!(error, EngineError::EmptyValidatorSet);
        assert!(error.to_string().contains("Fix:"));
    }

    #[test]
    fn the_committee_repeats_for_every_node() {
        let input = input(7);
        let entries = entries(9, 7, TrustScore::from_percent(70));
        let first = CommitteeDraw::new()
            .assemble(&input, &entries)
            .expect("the committee assembles");
        let mut shuffled = entries.clone();
        shuffled.reverse();
        let second = CommitteeDraw::new()
            .assemble(&input, &shuffled)
            .expect("the committee assembles");
        assert_eq!(first, second);
    }

    #[test]
    fn a_draw_that_fails_its_proof_stops_the_assembly() {
        let input = input(4);
        let mut entries = entries(6, 4, TrustScore::from_percent(50));
        entries[2].0.output[0] ^= 0xff;
        let error = CommitteeDraw::new()
            .assemble(&input, &entries)
            .expect_err("the assembly reports the broken draw");
        assert!(matches!(error, EngineError::DrawOutputMismatch { .. }));
    }

    #[test]
    fn a_draw_produced_for_another_slot_stops_the_assembly() {
        let mut entries = entries(6, 4, TrustScore::from_percent(50));
        entries[1].0 = draw(&validator(1), &input(99));
        let error = CommitteeDraw::new()
            .assemble(&input(4), &entries)
            .expect_err("the assembly reports the wrong slot");
        assert!(matches!(error, EngineError::DrawProofRejected { .. }));
    }

    #[test]
    fn full_standing_scales_a_draw_to_about_one_quarter() {
        let raw = 4_000_000_000u128;
        let scaled = weighted_draw(raw, TrustScore::from_percent(100));
        let quarter = raw / 4;
        let gap = quarter.abs_diff(scaled);
        assert!(
            gap * 1_000 < quarter,
            "the scaled value sits within a tenth of a percent of one quarter: {} against {}",
            scaled,
            quarter
        );
    }

    #[test]
    fn zero_standing_leaves_a_draw_unscaled() {
        let raw = 4_000_000_000u128;
        assert_eq!(weighted_draw(raw, TrustScore(0)), raw);
    }

    #[test]
    fn a_higher_standing_never_raises_a_scaled_value() {
        let raw = 123_456_789_012_345u128;
        let mut previous = u128::MAX;
        for percent in 0..=100u8 {
            let scaled = weighted_draw(raw, TrustScore::from_percent(percent));
            assert!(scaled <= previous);
            previous = scaled;
        }
    }

    #[test]
    fn the_scale_stays_inside_the_number_range() {
        assert!(weighted_draw(u128::MAX, TrustScore(0)) > 0);
        assert!(weighted_draw(u128::MAX, TrustScore::from_percent(100)) < u128::MAX);
        assert_eq!(weighted_draw(0, TrustScore::from_percent(100)), 0);
    }

    #[test]
    fn higher_standing_wins_more_ranks_over_many_slots() {
        let strong: Vec<u8> = (1..=6).collect();
        let weak: Vec<u8> = (7..=12).collect();
        let mut strong_seats = 0usize;
        let mut weak_seats = 0usize;
        for slot in 0..40u64 {
            let input = DrawInput::new(Slot(slot), SlotSeed([9u8; 32]));
            let mut entries: Vec<(DrawResult, TrustScore)> = Vec::new();
            for seed in &strong {
                entries.push((
                    draw(&validator(*seed), &input),
                    TrustScore::from_percent(95),
                ));
            }
            for seed in &weak {
                entries.push((draw(&validator(*seed), &input), TrustScore::from_percent(5)));
            }
            let committee = CommitteeDraw::new()
                .assemble(&input, &entries)
                .expect("the committee assembles");
            for member in &committee.members {
                let strong_member = strong
                    .iter()
                    .any(|seed| validator(*seed).public() == member.validator);
                if strong_member {
                    strong_seats += 1;
                } else {
                    weak_seats += 1;
                }
            }
        }
        assert!(
            strong_seats > weak_seats,
            "higher standing takes more seats: {} against {}",
            strong_seats,
            weak_seats
        );
        assert!(
            weak_seats > 0,
            "a validator with low standing keeps a real chance"
        );
    }

    #[test]
    fn a_tie_settles_on_the_validator_key() {
        let input = input(2);
        let left = validator(1).public();
        let right = validator(2).public();
        let committee = CommitteeDraw::new().rank(&input, vec![(10, right), (10, left)]);
        let ordered = if left.0 < right.0 {
            [left, right]
        } else {
            [right, left]
        };
        assert_eq!(committee.members[0].validator, ordered[0]);
        assert_eq!(committee.members[1].validator, ordered[1]);
    }

    #[test]
    fn one_validator_takes_one_seat() {
        let input = input(2);
        let repeated = validator(1).public();
        let committee = CommitteeDraw::new().rank(
            &input,
            vec![(10, repeated), (10, repeated), (20, validator(2).public())],
        );
        assert_eq!(committee.len(), 2);
    }

    #[test]
    fn the_engine_reads_back_the_scheduled_author() {
        let input = input(3);
        let entries = entries(8, 3, TrustScore::from_percent(50));
        let engine = CommitteeDraw::new();
        let committee = engine
            .committee(&input, &entries)
            .expect("the committee assembles");
        let author = engine
            .scheduled_author(&committee, Rank::FIRST)
            .expect("rank one holds an author");
        assert_eq!(*author, committee.members[0].validator);
        assert!(engine.scheduled_author(&committee, Rank(9)).is_none());
        assert_eq!(CommitteeDraw::KIND, EngineKind::Authoring);
        assert_eq!(CommitteeDraw::engine_name(), "Authoring Engine");
    }

    #[test]
    fn the_committee_changes_from_slot_to_slot() {
        let mut first_authors = Vec::new();
        for slot in 0..12u64 {
            let input = DrawInput::new(Slot(slot), SlotSeed([3u8; 32]));
            let entries: Vec<(DrawResult, TrustScore)> = (1..=10u8)
                .map(|seed| (draw(&validator(seed), &input), TrustScore::from_percent(50)))
                .collect();
            let committee = CommitteeDraw::new()
                .assemble(&input, &entries)
                .expect("the committee assembles");
            first_authors.push(committee.members[0].validator);
        }
        first_authors.dedup();
        assert!(
            first_authors.len() > 1,
            "the author changes across slots: {:?}",
            first_authors.len()
        );
    }
}
