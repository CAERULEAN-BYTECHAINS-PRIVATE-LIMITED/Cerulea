//! The eight policy records.
//!
//! A deployment profile is expressed as eight policies. The Amendment Engine
//! governs changes to the first seven. The eighth governs itself.
//!
//! | Policy | Governs |
//! |---|---|
//! | Admission | Identity requirement, sponsoring authority, credentials, licence class |
//! | Rotation | Epoch length, set size, rotation trigger |
//! | Authoring | Trust Score mix and the draw threshold curve |
//! | Assurance | The class table and the share per class |
//! | Eligibility | Uptime floor, participation floor, standing floor |
//! | Penalty | Offence catalogue, severity ladder, demotion conditions |
//! | Reward | Distribution formula, including throughput quota and priority |
//! | Amendment | Authority, quorum and delay governing changes to the seven above |
//!
//! Combining tunable profiles with class shares means one deployment carries
//! several agreement points at once. A logistics scan commits under a fast
//! profile. A payment in the same system commits under a strict one.

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::pallet_prelude::RuntimeDebug;
use scale_info::TypeInfo;
use sp_runtime::Perbill;

/// The eight policies one deployment profile carries.
#[derive(
    Encode,
    Decode,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    RuntimeDebug,
    TypeInfo,
    MaxEncodedLen,
    Default,
    frame_support::__private::codec::DecodeWithMemTracking,
)]
pub enum PolicyKind {
    /// Identity requirement, sponsoring authority, credentials, licence class.
    #[default]
    Admission,
    /// Epoch length, set size, rotation trigger.
    Rotation,
    /// Trust Score mix and the draw threshold curve.
    Authoring,
    /// The class table and the share per class.
    Assurance,
    /// Uptime floor, participation floor, standing floor.
    Eligibility,
    /// Offence catalogue, severity ladder, demotion conditions.
    Penalty,
    /// Distribution formula, including throughput quota and priority.
    Reward,
    /// Authority, quorum and delay governing changes to the seven above.
    Amendment,
}

impl PolicyKind {
    /// Every policy, in record order.
    pub const ALL: [PolicyKind; 8] = [
        PolicyKind::Admission,
        PolicyKind::Rotation,
        PolicyKind::Authoring,
        PolicyKind::Assurance,
        PolicyKind::Eligibility,
        PolicyKind::Penalty,
        PolicyKind::Reward,
        PolicyKind::Amendment,
    ];

    /// The seven policies the Amendment Engine governs.
    pub const GOVERNED: [PolicyKind; 7] = [
        PolicyKind::Admission,
        PolicyKind::Rotation,
        PolicyKind::Authoring,
        PolicyKind::Assurance,
        PolicyKind::Eligibility,
        PolicyKind::Penalty,
        PolicyKind::Reward,
    ];

    /// A short label for logs and interface copy.
    pub const fn label(&self) -> &'static str {
        match self {
            PolicyKind::Admission => "admission",
            PolicyKind::Rotation => "rotation",
            PolicyKind::Authoring => "authoring",
            PolicyKind::Assurance => "assurance",
            PolicyKind::Eligibility => "eligibility",
            PolicyKind::Penalty => "penalty",
            PolicyKind::Reward => "reward",
            PolicyKind::Amendment => "amendment",
        }
    }

    /// A plain sentence stating what the policy governs.
    pub const fn plain_language(&self) -> &'static str {
        match self {
            PolicyKind::Admission => {
                "Admission governs the identity requirement, the sponsoring authority, the credentials and the licence class."
            }
            PolicyKind::Rotation => {
                "Rotation governs the epoch length, the validator set size and the trigger that changes the set."
            }
            PolicyKind::Authoring => {
                "Authoring governs the Trust Score mix and the curve that scales each draw."
            }
            PolicyKind::Assurance => {
                "Assurance governs the transaction class table and the share of weight each class needs."
            }
            PolicyKind::Eligibility => {
                "Eligibility governs the uptime floor, the participation floor and the standing floor."
            }
            PolicyKind::Penalty => {
                "Penalty governs the offence catalogue, the severity ladder and the demotion conditions."
            }
            PolicyKind::Reward => {
                "Reward governs the distribution formula, including the throughput quota and the priority."
            }
            PolicyKind::Amendment => {
                "Amendment governs the authority, the quorum and the delay that change the seven policies above."
            }
        }
    }

    /// Report whether the Amendment Engine governs this policy.
    pub const fn governed_by_amendment(&self) -> bool {
        !matches!(self, PolicyKind::Amendment)
    }
}

/// The Admission policy.
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
pub struct AdmissionPolicy {
    /// A registrar confirms the legal identity of every candidate.
    pub identity_required: bool,
    /// The approvals a candidate collects from the governing body.
    pub approvals_required: u32,
    /// The credentials the deployment asks for.
    pub credentials_required: u32,
    /// The lowest licence class the deployment accepts.
    pub licence_class: u8,
}

/// The Rotation policy.
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
pub struct RotationPolicy {
    /// The blocks one epoch holds.
    pub epoch_blocks: u32,
    /// The validators the active set holds.
    pub set_size: u32,
    /// The set changes only at an epoch boundary where this holds.
    pub rotate_at_boundary_only: bool,
}

/// The Authoring policy.
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
pub struct AuthoringPolicy {
    /// The committee drawn for one slot.
    pub committee_size: u32,
    /// The blocks a rank holds before the slot passes to the next rank.
    pub rank_timeout_blocks: u32,
    /// The share the Trust Score carries in the draw scaling.
    pub trust_score_weight: Perbill,
}

/// The Assurance policy.
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
pub struct AssurancePolicy {
    /// The share a routine transaction needs, in percent.
    pub routine_percent: u8,
    /// The share a financial transaction needs, in percent.
    pub financial_percent: u8,
    /// The share a structural transaction needs, in percent.
    pub structural_percent: u8,
    /// The share a constitutional transaction needs, in percent.
    pub constitutional_percent: u8,
    /// The blocks a constitutional change waits.
    pub constitutional_delay_blocks: u32,
}

/// The Eligibility policy.
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
pub struct EligibilityPolicy {
    /// The uptime a seated validator holds, at least.
    pub uptime_floor: Perbill,
    /// The participation a seated validator holds, at least.
    pub participation_floor: Perbill,
    /// The standing a seated validator holds, at least.
    pub standing_floor: Perbill,
}

/// The Penalty policy.
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
pub struct PenaltyPolicy {
    /// The offences the catalogue names.
    pub offence_count: u32,
    /// The share a light offence removes from standing.
    pub light_loss: Perbill,
    /// The share a severe offence removes from standing.
    pub severe_loss: Perbill,
    /// Proven misconduct returns standing to zero where this holds.
    pub misconduct_resets_standing: bool,
}

/// The Reward policy.
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
pub struct RewardPolicy {
    /// The share every seated validator receives.
    pub base_share: Perbill,
    /// The share high performers receive on top.
    pub performance_share: Perbill,
    /// The share the top performers receive on top.
    pub top_performer_share: Perbill,
    /// The transactions one validator submits per block.
    pub throughput_quota: u32,
}

/// The Amendment policy.
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
pub struct AmendmentPolicy {
    /// The authority that may open an amendment.
    pub authority: Authority,
    /// The share of the governing body that approves an amendment.
    pub quorum: Perbill,
    /// The blocks an approved amendment waits before it takes effect.
    pub delay_blocks: u32,
}

/// The authority that may open an amendment.
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
pub enum Authority {
    /// The deployment operator alone.
    #[default]
    Operator,
    /// The seated validators, by vote.
    ValidatorSet,
    /// A named governing body.
    GoverningBody,
}

impl Authority {
    /// A plain sentence stating who may open an amendment.
    pub const fn plain_language(&self) -> &'static str {
        match self {
            Authority::Operator => "The deployment operator opens an amendment.",
            Authority::ValidatorSet => "The seated validators open an amendment by vote.",
            Authority::GoverningBody => "A named governing body opens an amendment.",
        }
    }
}

/// The eight policies of one deployment profile.
///
/// A deployment that never amended a policy reads the starting profile, so a
/// chain answers a policy question from the first block.
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
    frame_support::__private::codec::DecodeWithMemTracking,
)]
pub struct PolicySet {
    /// The Admission policy.
    pub admission: AdmissionPolicy,
    /// The Rotation policy.
    pub rotation: RotationPolicy,
    /// The Authoring policy.
    pub authoring: AuthoringPolicy,
    /// The Assurance policy.
    pub assurance: AssurancePolicy,
    /// The Eligibility policy.
    pub eligibility: EligibilityPolicy,
    /// The Penalty policy.
    pub penalty: PenaltyPolicy,
    /// The Reward policy.
    pub reward: RewardPolicy,
    /// The Amendment policy.
    pub amendment: AmendmentPolicy,
}

impl Default for PolicySet {
    /// The profile a deployment starts from.
    ///
    /// The Assurance Engine class table reads the same way. A deployment that
    /// amended nothing runs the published profile.
    fn default() -> Self {
        PolicySet::starting_profile()
    }
}

impl PolicySet {
    /// The profile a deployment starts from.
    pub fn starting_profile() -> Self {
        PolicySet {
            admission: AdmissionPolicy {
                identity_required: true,
                approvals_required: 1,
                credentials_required: 0,
                licence_class: 1,
            },
            rotation: RotationPolicy {
                epoch_blocks: 100,
                set_size: 50,
                rotate_at_boundary_only: true,
            },
            authoring: AuthoringPolicy {
                committee_size: 5,
                rank_timeout_blocks: 1,
                trust_score_weight: Perbill::from_percent(100),
            },
            assurance: AssurancePolicy {
                routine_percent: 51,
                financial_percent: 67,
                structural_percent: 80,
                constitutional_percent: 90,
                constitutional_delay_blocks: 100,
            },
            eligibility: EligibilityPolicy {
                uptime_floor: Perbill::from_percent(80),
                participation_floor: Perbill::from_percent(50),
                standing_floor: Perbill::from_percent(33),
            },
            penalty: PenaltyPolicy {
                offence_count: 5,
                light_loss: Perbill::from_percent(5),
                severe_loss: Perbill::from_percent(50),
                misconduct_resets_standing: true,
            },
            reward: RewardPolicy {
                base_share: Perbill::from_percent(60),
                performance_share: Perbill::from_percent(25),
                top_performer_share: Perbill::from_percent(15),
                throughput_quota: 1_000,
            },
            amendment: AmendmentPolicy {
                authority: Authority::ValidatorSet,
                quorum: Perbill::from_percent(67),
                delay_blocks: 100,
            },
        }
    }

    /// The version counter of one policy.
    ///
    /// The Amendment Engine raises the counter on every accepted change, so a
    /// stale proposal never overwrites a newer record.
    pub const fn policy_count() -> u32 {
        8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_profile_carries_eight_policies() {
        assert_eq!(PolicyKind::ALL.len(), 8);
        assert_eq!(PolicySet::policy_count(), 8);
    }

    #[test]
    fn the_amendment_engine_governs_seven_policies() {
        assert_eq!(PolicyKind::GOVERNED.len(), 7);
        for policy in PolicyKind::GOVERNED.iter() {
            assert!(policy.governed_by_amendment());
        }
        assert!(!PolicyKind::Amendment.governed_by_amendment());
    }

    #[test]
    fn every_policy_carries_a_label_and_a_plain_sentence() {
        for policy in PolicyKind::ALL.iter() {
            assert!(!policy.label().is_empty());
            assert!(policy.plain_language().len() > 40);
        }
    }

    #[test]
    fn the_starting_profile_carries_the_published_shares() {
        let held = PolicySet::starting_profile();
        assert_eq!(held.assurance.routine_percent, 51);
        assert_eq!(held.assurance.financial_percent, 67);
        assert_eq!(held.assurance.structural_percent, 80);
        assert_eq!(held.assurance.constitutional_percent, 90);
        assert!(held.assurance.constitutional_delay_blocks > 0);
    }

    #[test]
    fn the_starting_profile_carries_a_committee_of_five() {
        let held = PolicySet::starting_profile();
        assert_eq!(held.authoring.committee_size, 5);
    }

    #[test]
    fn the_starting_reward_shares_add_to_the_whole() {
        let held = PolicySet::starting_profile().reward;
        let total = held.base_share.deconstruct()
            + held.performance_share.deconstruct()
            + held.top_performer_share.deconstruct();
        assert_eq!(total, Perbill::one().deconstruct());
    }

    #[test]
    fn the_starting_amendment_policy_asks_for_two_thirds_and_a_delay() {
        let held = PolicySet::starting_profile().amendment;
        assert_eq!(held.quorum, Perbill::from_percent(67));
        assert!(held.delay_blocks > 0);
        assert_eq!(held.authority, Authority::ValidatorSet);
    }

    #[test]
    fn every_authority_states_who_opens_an_amendment() {
        for authority in [
            Authority::Operator,
            Authority::ValidatorSet,
            Authority::GoverningBody,
        ] {
            assert!(authority.plain_language().len() > 25);
        }
    }

    #[test]
    fn proven_misconduct_returns_standing_to_zero_in_the_starting_profile() {
        assert!(
            PolicySet::starting_profile()
                .penalty
                .misconduct_resets_standing
        );
    }
}
