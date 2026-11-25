//! The outcome profile solver.
//!
//! Seven consensus properties genuinely trade against one another. Cerulea
//! exposes the outcomes and solves for the properties. The customer declares
//! what matters and never touches a parameter.
//!
//! The solver takes five plain-language declarations, searches the parameter
//! space, checks every candidate against the fixed limits, and returns three to
//! five valid profiles with their consequences stated in plain language.
//!
//! The solver is a pure function. The same call answers a chain query and an
//! off-chain question, so a customer sees the same profiles the chain would
//! accept.

use super::limits::{inspect, CandidatePoint, LimitBreach, PartitionChoice};
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::pallet_prelude::RuntimeDebug;
use frame_support::BoundedVec;
use scale_info::TypeInfo;
use sp_std::vec::Vec;

/// The fewest profiles the solver returns.
pub const MIN_PROFILES: usize = 3;

/// The most profiles the solver returns.
pub const MAX_PROFILES: usize = 5;

/// Whether instant confirmation or maximum certainty matters more.
#[derive(
    Encode,
    Decode,
    Clone,
    Copy,
    PartialEq,
    Eq,
    RuntimeDebug,
    TypeInfo,
    MaxEncodedLen,
    Default,
    frame_support::__private::codec::DecodeWithMemTracking,
)]
pub enum ConfirmationPreference {
    /// A record confirms as soon as possible.
    #[default]
    InstantConfirmation,
    /// A record confirms with the greatest certainty available.
    MaximumCertainty,
}

/// How many organisations run infrastructure.
#[derive(
    Encode,
    Decode,
    Clone,
    Copy,
    PartialEq,
    Eq,
    RuntimeDebug,
    TypeInfo,
    MaxEncodedLen,
    Default,
    frame_support::__private::codec::DecodeWithMemTracking,
)]
pub enum OperatorCount {
    /// One organisation runs every machine.
    #[default]
    SingleOrganisation,
    /// A handful of organisations share the work.
    SeveralOrganisations,
    /// Many independent organisations take part.
    ManyOrganisations,
}

/// What a single wrong record would cost.
#[derive(
    Encode,
    Decode,
    Clone,
    Copy,
    PartialEq,
    Eq,
    RuntimeDebug,
    TypeInfo,
    MaxEncodedLen,
    Default,
    frame_support::__private::codec::DecodeWithMemTracking,
)]
pub enum ErrorCost {
    /// A wrong record is corrected with no lasting harm.
    #[default]
    Recoverable,
    /// A wrong record costs money or time to unwind.
    Expensive,
    /// A wrong record causes harm that no correction undoes.
    Irreversible,
}

/// Whether the system keeps serving during a network split.
#[derive(
    Encode,
    Decode,
    Clone,
    Copy,
    PartialEq,
    Eq,
    RuntimeDebug,
    TypeInfo,
    MaxEncodedLen,
    Default,
    frame_support::__private::codec::DecodeWithMemTracking,
)]
pub enum SplitBehaviour {
    /// Service continues on both sides of a split.
    #[default]
    KeepServing,
    /// Service halts on the smaller side to stay consistent.
    HaltToStayConsistent,
}

/// How long records remain provable.
#[derive(
    Encode,
    Decode,
    Clone,
    Copy,
    PartialEq,
    Eq,
    RuntimeDebug,
    TypeInfo,
    MaxEncodedLen,
    Default,
    frame_support::__private::codec::DecodeWithMemTracking,
)]
pub enum RetentionNeed {
    /// Recent records suffice.
    #[default]
    RecentOnly,
    /// Records remain provable for several years.
    SeveralYears,
    /// Records remain provable for as long as the network runs.
    Permanent,
}

/// The five declarations the customer makes.
#[derive(
    Encode,
    Decode,
    Clone,
    Copy,
    PartialEq,
    Eq,
    RuntimeDebug,
    TypeInfo,
    MaxEncodedLen,
    Default,
    frame_support::__private::codec::DecodeWithMemTracking,
)]
pub struct Declarations {
    /// Whether instant confirmation or maximum certainty matters more.
    pub confirmation: ConfirmationPreference,
    /// How many organisations run infrastructure.
    pub operators: OperatorCount,
    /// What a single wrong record would cost.
    pub error_cost: ErrorCost,
    /// Whether the system keeps serving during a network split.
    pub split: SplitBehaviour,
    /// How long records remain provable.
    pub retention: RetentionNeed,
}

/// The greatest number of bytes one consequence sentence carries.
pub const MAX_CONSEQUENCE_BYTES: u32 = 256;

/// The greatest number of consequence sentences one profile carries.
pub const MAX_CONSEQUENCES: u32 = 8;

/// One valid profile the solver returns.
#[derive(
    Encode,
    Decode,
    Clone,
    PartialEq,
    Eq,
    RuntimeDebug,
    TypeInfo,
    MaxEncodedLen,
    frame_support::__private::codec::DecodeWithMemTracking,
)]
pub struct Profile {
    /// The point in the parameter space.
    pub point: CandidatePoint,
    /// The blocks a record waits before it commits, at the routine class.
    pub routine_finality_blocks: u32,
    /// The blocks a record waits before it commits, at the constitutional
    /// class.
    pub constitutional_finality_blocks: u32,
    /// The blocks of history the deployment keeps. Zero means every block.
    pub retention_blocks: u32,
    /// The consequences stated in plain language.
    pub consequences: BoundedVec<
        BoundedVec<u8, frame_support::traits::ConstU32<MAX_CONSEQUENCE_BYTES>>,
        frame_support::traits::ConstU32<MAX_CONSEQUENCES>,
    >,
}

fn sentence(text: &str) -> BoundedVec<u8, frame_support::traits::ConstU32<MAX_CONSEQUENCE_BYTES>> {
    let mut bytes = text.as_bytes().to_vec();
    bytes.truncate(MAX_CONSEQUENCE_BYTES as usize);
    BoundedVec::try_from(bytes).unwrap_or_default()
}

/// The validator counts the search walks.
const SET_SIZES: [u32; 6] = [4, 7, 10, 21, 50, 100];

/// The routine shares the search walks.
const ROUTINE_SHARES: [u8; 3] = [51, 60, 67];

/// The constitutional shares the search walks.
const CONSTITUTIONAL_SHARES: [u8; 3] = [80, 90, 95];

/// Score one candidate against the declarations.
///
/// A lower score fits the declarations better. The solver keeps the best
/// scoring valid points.
fn distance(point: &CandidatePoint, declarations: &Declarations) -> u32 {
    let mut score = 0u32;

    // Confirmation preference reads on the routine share and the set size.
    score += match declarations.confirmation {
        ConfirmationPreference::InstantConfirmation => {
            (point.routine_percent as u32 - 51) * 2 + point.validator_count / 4
        }
        ConfirmationPreference::MaximumCertainty => {
            (67 - point.routine_percent as u32) * 2 + (100 - point.validator_count.min(100)) / 4
        }
    };

    // The operator count reads on the set size.
    let wanted_set = match declarations.operators {
        OperatorCount::SingleOrganisation => 4,
        OperatorCount::SeveralOrganisations => 10,
        OperatorCount::ManyOrganisations => 50,
    };
    score += point.validator_count.abs_diff(wanted_set) / 2;

    // The cost of a wrong record reads on the constitutional share.
    let wanted_constitutional = match declarations.error_cost {
        ErrorCost::Recoverable => 80u8,
        ErrorCost::Expensive => 90,
        ErrorCost::Irreversible => 95,
    };
    score += (point.constitutional_percent as u32).abs_diff(wanted_constitutional as u32);

    // The split behaviour reads on the partition choice.
    let wanted_choice = match declarations.split {
        SplitBehaviour::KeepServing => PartitionChoice::KeepServing,
        SplitBehaviour::HaltToStayConsistent => PartitionChoice::HaltToStayConsistent,
    };
    if point.partition_choice != wanted_choice {
        score += 40;
    }

    score
}

/// The blocks of history one retention need keeps.
fn retention_blocks(need: RetentionNeed) -> u32 {
    match need {
        // Roughly one month at six second blocks.
        RetentionNeed::RecentOnly => 432_000,
        // Roughly three years at six second blocks.
        RetentionNeed::SeveralYears => 15_768_000,
        RetentionNeed::Permanent => 0,
    }
}

/// Build the consequence sentences for one point.
fn consequences(point: &CandidatePoint, declarations: &Declarations) -> Profile {
    let routine_blocks = 1 + point.validator_count / 25;
    let constitutional_blocks = routine_blocks + 2;
    let keep = retention_blocks(declarations.retention);

    let lines: Vec<BoundedVec<u8, frame_support::traits::ConstU32<MAX_CONSEQUENCE_BYTES>>> = sp_std::vec![
        sentence(match point.validator_count {
            0..=6 => "The network runs on a small set. Operating cost stays low. The network tolerates one failure at a time.",
            7..=20 => "The network runs on a moderate set. Trust spreads across several operators. Message volume stays moderate.",
            _ => "The network runs on a wide set. Trust spreads widely. Message volume and operating cost rise.",
        }),
        sentence(match point.routine_percent {
            51..=55 => "A routine record confirms at a simple majority of weight. Confirmation is quickest at this share.",
            56..=64 => "A routine record confirms above a simple majority. Confirmation costs a little more time and carries more certainty.",
            _ => "A routine record confirms at two thirds of weight. Confirmation is slowest at this share and carries the most certainty.",
        }),
        sentence(match point.constitutional_percent {
            0..=84 => "A policy change confirms at four fifths of weight. A smaller group can carry a change through.",
            85..=92 => "A policy change confirms at nine tenths of weight. A change needs near unanimity.",
            _ => "A policy change confirms at nineteen twentieths of weight. A change needs the agreement of almost every validator.",
        }),
        sentence(point.partition_choice.plain_language()),
        sentence(match declarations.retention {
            RetentionNeed::RecentOnly => "The deployment keeps roughly one month of history. Storage cost stays low. Older records leave the chain.",
            RetentionNeed::SeveralYears => "The deployment keeps roughly three years of history. Storage cost is moderate. Records stay provable for that span.",
            RetentionNeed::Permanent => "The deployment keeps every block from genesis. Storage cost rises over time. Every record stays provable.",
        }),
        sentence(
            "The network tolerates failures in fewer than a third of the set. Crossing that share is refused by the framework.",
        ),
    ];

    Profile {
        point: *point,
        routine_finality_blocks: routine_blocks,
        constitutional_finality_blocks: constitutional_blocks,
        retention_blocks: keep,
        consequences: BoundedVec::try_from(lines).unwrap_or_default(),
    }
}

/// Search the parameter space and return the valid profiles.
///
/// Every candidate passes the fixed limit checks before it enters the result
/// set, so an invalid point never surfaces. The result holds between three and
/// five profiles.
pub fn solve(declarations: Declarations) -> Vec<Profile> {
    let wanted_choice = match declarations.split {
        SplitBehaviour::KeepServing => PartitionChoice::KeepServing,
        SplitBehaviour::HaltToStayConsistent => PartitionChoice::HaltToStayConsistent,
    };

    let mut scored: Vec<(u32, CandidatePoint)> = Vec::new();

    for set_size in SET_SIZES.iter() {
        for routine in ROUTINE_SHARES.iter() {
            for constitutional in CONSTITUTIONAL_SHARES.iter() {
                for choice in [wanted_choice, PartitionChoice::HaltToStayConsistent] {
                    let point = CandidatePoint {
                        validator_count: *set_size,
                        tolerated_failures: super::limits::max_faulty(*set_size),
                        routine_percent: *routine,
                        constitutional_percent: *constitutional,
                        partition_choice: choice,
                        partition_choice_stated: true,
                    };
                    if inspect(&point).is_err() {
                        continue;
                    }
                    if scored.iter().any(|(_, held)| held == &point) {
                        continue;
                    }
                    scored.push((distance(&point, &declarations), point));
                }
            }
        }
    }

    scored.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then(left.1.validator_count.cmp(&right.1.validator_count))
            .then(left.1.routine_percent.cmp(&right.1.routine_percent))
            .then(
                left.1
                    .constitutional_percent
                    .cmp(&right.1.constitutional_percent),
            )
    });

    // Keep distinct set sizes where possible, so the profiles differ in a way
    // a person reads at a glance.
    let mut chosen: Vec<CandidatePoint> = Vec::new();
    let mut seen_sizes: Vec<u32> = Vec::new();
    for (_, point) in scored.iter() {
        if chosen.len() >= MAX_PROFILES {
            break;
        }
        if seen_sizes.contains(&point.validator_count) {
            continue;
        }
        seen_sizes.push(point.validator_count);
        chosen.push(*point);
    }
    for (_, point) in scored.iter() {
        if chosen.len() >= MIN_PROFILES {
            break;
        }
        if !chosen.contains(point) {
            chosen.push(*point);
        }
    }

    chosen
        .iter()
        .map(|point| consequences(point, &declarations))
        .collect()
}

/// Inspect one profile against the fixed limits.
///
/// The Amendment Engine runs this before it accepts a chosen profile.
pub fn inspect_profile(profile: &Profile) -> Result<(), LimitBreach> {
    inspect(&profile.point)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn declarations() -> Declarations {
        Declarations {
            confirmation: ConfirmationPreference::InstantConfirmation,
            operators: OperatorCount::SeveralOrganisations,
            error_cost: ErrorCost::Expensive,
            split: SplitBehaviour::HaltToStayConsistent,
            retention: RetentionNeed::Permanent,
        }
    }

    #[test]
    fn the_solver_returns_between_three_and_five_profiles() {
        let profiles = solve(declarations());
        assert!(profiles.len() >= MIN_PROFILES);
        assert!(profiles.len() <= MAX_PROFILES);
    }

    /// Design assertion 12.
    ///
    /// A solver profile that would cross a fixed limit never appears in the
    /// result set.
    #[test]
    fn no_profile_crosses_a_fixed_limit() {
        let every_declaration = [
            ConfirmationPreference::InstantConfirmation,
            ConfirmationPreference::MaximumCertainty,
        ];
        let every_operator = [
            OperatorCount::SingleOrganisation,
            OperatorCount::SeveralOrganisations,
            OperatorCount::ManyOrganisations,
        ];
        let every_cost = [
            ErrorCost::Recoverable,
            ErrorCost::Expensive,
            ErrorCost::Irreversible,
        ];
        let every_split = [
            SplitBehaviour::KeepServing,
            SplitBehaviour::HaltToStayConsistent,
        ];
        let every_retention = [
            RetentionNeed::RecentOnly,
            RetentionNeed::SeveralYears,
            RetentionNeed::Permanent,
        ];

        for confirmation in every_declaration.iter() {
            for operators in every_operator.iter() {
                for error_cost in every_cost.iter() {
                    for split in every_split.iter() {
                        for retention in every_retention.iter() {
                            let held = Declarations {
                                confirmation: *confirmation,
                                operators: *operators,
                                error_cost: *error_cost,
                                split: *split,
                                retention: *retention,
                            };
                            let profiles = solve(held);
                            assert!(profiles.len() >= MIN_PROFILES);
                            assert!(profiles.len() <= MAX_PROFILES);
                            for profile in profiles.iter() {
                                assert_eq!(
                                    inspect_profile(profile),
                                    Ok(()),
                                    "every profile passes every fixed limit"
                                );
                                assert!(
                                    super::super::limits::participants_exceed_three_times_faulty(
                                        profile.point.validator_count,
                                        profile.point.tolerated_failures
                                    )
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn every_profile_states_its_consequences_in_plain_language() {
        for profile in solve(declarations()).iter() {
            assert!(profile.consequences.len() >= 5);
            for line in profile.consequences.iter() {
                let text = sp_std::str::from_utf8(line).expect("the sentence reads as text");
                assert!(text.len() > 30);
                assert!(text.ends_with('.'));
            }
        }
    }

    #[test]
    fn the_solver_answers_the_same_declarations_the_same_way() {
        assert_eq!(solve(declarations()), solve(declarations()));
    }

    #[test]
    fn the_profiles_differ_from_one_another() {
        let profiles = solve(declarations());
        for (at, profile) in profiles.iter().enumerate() {
            for other in profiles.iter().skip(at + 1) {
                assert_ne!(profile.point, other.point);
            }
        }
    }

    #[test]
    fn a_single_organisation_receives_a_small_set_first() {
        let mut held = declarations();
        held.operators = OperatorCount::SingleOrganisation;
        let profiles = solve(held);
        assert!(profiles[0].point.validator_count <= 10);
    }

    #[test]
    fn many_organisations_receive_a_wider_set_first() {
        let mut held = declarations();
        held.operators = OperatorCount::ManyOrganisations;
        let profiles = solve(held);
        assert!(profiles[0].point.validator_count >= 21);
    }

    #[test]
    fn an_irreversible_cost_raises_the_constitutional_share() {
        let mut recoverable = declarations();
        recoverable.error_cost = ErrorCost::Recoverable;
        let mut irreversible = declarations();
        irreversible.error_cost = ErrorCost::Irreversible;

        assert!(
            solve(irreversible)[0].point.constitutional_percent
                > solve(recoverable)[0].point.constitutional_percent
        );
    }

    #[test]
    fn a_split_preference_reaches_the_partition_choice() {
        let mut keep = declarations();
        keep.split = SplitBehaviour::KeepServing;
        assert_eq!(
            solve(keep)[0].point.partition_choice,
            PartitionChoice::KeepServing
        );

        let mut halt = declarations();
        halt.split = SplitBehaviour::HaltToStayConsistent;
        assert_eq!(
            solve(halt)[0].point.partition_choice,
            PartitionChoice::HaltToStayConsistent
        );
    }

    #[test]
    fn a_permanent_retention_keeps_every_block() {
        let mut held = declarations();
        held.retention = RetentionNeed::Permanent;
        assert_eq!(solve(held)[0].retention_blocks, 0);

        held.retention = RetentionNeed::RecentOnly;
        assert!(solve(held)[0].retention_blocks > 0);
    }

    #[test]
    fn a_wider_set_reports_a_longer_wait_to_finality() {
        let mut narrow = declarations();
        narrow.operators = OperatorCount::SingleOrganisation;
        let mut wide = declarations();
        wide.operators = OperatorCount::ManyOrganisations;
        assert!(solve(wide)[0].routine_finality_blocks >= solve(narrow)[0].routine_finality_blocks);
    }

    #[test]
    fn a_constitutional_change_waits_longer_than_a_routine_record() {
        for profile in solve(declarations()).iter() {
            assert!(profile.constitutional_finality_blocks > profile.routine_finality_blocks);
        }
    }
}
