//! The Trust Score composition.
//!
//! One composite measure drives four engines. The Trust Score. Nothing else
//! governs validator behaviour.
//!
//! The composition differs between the two networks Cerulea runs.
//!
//! | Component | Public-access chain | Private chain |
//! |---|---|---|
//! | Standing accumulated through service | 40 percent | absent |
//! | Role weight | absent | 35 percent |
//! | Credential attestation | absent | 25 percent |
//! | Measured operational performance | 30 percent | 25 percent |
//! | Custody proof | 20 percent | 15 percent |
//! | Verification sampling | 10 percent | absent |
//!
//! Each component reads as a [`Perbill`], which is a share in parts per
//! billion. The total is the weighted sum of the components the network uses.

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::pallet_prelude::RuntimeDebug;
use scale_info::TypeInfo;
use sp_runtime::Perbill;

/// The network a deployment runs.
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
pub enum ChainKind {
    /// The public-access chain. Anyone meeting the conditions may join.
    #[default]
    PublicAccess,
    /// A private chain. Governance admits every validator.
    Private,
}

impl ChainKind {
    /// A plain sentence stating how this network weighs a validator.
    pub const fn plain_language(&self) -> &'static str {
        match self {
            ChainKind::PublicAccess => {
                "The public-access chain weighs standing at 40 percent, performance at 30 percent, custody at 20 percent and sampling at 10 percent."
            }
            ChainKind::Private => {
                "A private chain weighs role at 35 percent, attestation at 25 percent, performance at 25 percent and custody at 15 percent."
            }
        }
    }
}

/// The share standing carries on the public-access chain, in percent.
pub const PUBLIC_STANDING_PERCENT: u32 = 40;

/// The share performance carries on the public-access chain, in percent.
pub const PUBLIC_PERFORMANCE_PERCENT: u32 = 30;

/// The share custody carries on the public-access chain, in percent.
pub const PUBLIC_CUSTODY_PERCENT: u32 = 20;

/// The share sampling carries on the public-access chain, in percent.
pub const PUBLIC_SAMPLING_PERCENT: u32 = 10;

/// The share role carries on a private chain, in percent.
pub const PRIVATE_ROLE_PERCENT: u32 = 35;

/// The share attestation carries on a private chain, in percent.
pub const PRIVATE_ATTESTATION_PERCENT: u32 = 25;

/// The share performance carries on a private chain, in percent.
pub const PRIVATE_PERFORMANCE_PERCENT: u32 = 25;

/// The share custody carries on a private chain, in percent.
pub const PRIVATE_CUSTODY_PERCENT: u32 = 15;

/// The measured components of one validator.
///
/// Each field is a share in parts per billion. A field the network leaves out
/// contributes nothing.
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
pub struct Components {
    /// Standing accumulated through service.
    pub standing: Perbill,
    /// Measured operational performance.
    pub performance: Perbill,
    /// Custody proof.
    pub custody: Perbill,
    /// Verification sampling.
    pub sampling: Perbill,
    /// Role weight on a private chain.
    pub role: Perbill,
    /// Credential attestation on a private chain.
    pub attestation: Perbill,
}

/// The Trust Score of one validator.
///
/// The score carries the total and every component that produced it, so an
/// operator reads why a validator sits where it sits.
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
pub struct TrustScore {
    /// The weighted total.
    pub total: Perbill,
    /// Standing accumulated through service.
    pub standing: Perbill,
    /// Measured operational performance.
    pub performance: Perbill,
    /// Custody proof.
    pub custody: Perbill,
    /// Verification sampling.
    pub sampling: Perbill,
    /// Role weight on a private chain.
    pub role: Perbill,
    /// Credential attestation on a private chain.
    pub attestation: Perbill,
}

impl TrustScore {
    /// Read the total as parts per billion.
    pub fn parts_per_billion(&self) -> u32 {
        self.total.deconstruct()
    }

    /// Read the total as a voting weight.
    ///
    /// The Commit Engine aggregates this weight across signers.
    pub fn weight(&self) -> u128 {
        self.total.deconstruct() as u128
    }
}

/// Scale one component by a share in percent.
fn scaled(value: Perbill, percent: u32) -> Perbill {
    Perbill::from_parts(((value.deconstruct() as u64 * percent as u64) / 100u64) as u32)
}

/// Compose the Trust Score of one validator.
///
/// The public-access chain reads standing, performance, custody and sampling.
/// A private chain reads role, attestation, performance and custody. Each
/// network weighs its four components to one whole, so a validator meeting
/// every component reaches the full score.
pub fn compose(kind: ChainKind, components: Components) -> TrustScore {
    let total_parts = match kind {
        ChainKind::PublicAccess => {
            scaled(components.standing, PUBLIC_STANDING_PERCENT).deconstruct() as u64
                + scaled(components.performance, PUBLIC_PERFORMANCE_PERCENT).deconstruct() as u64
                + scaled(components.custody, PUBLIC_CUSTODY_PERCENT).deconstruct() as u64
                + scaled(components.sampling, PUBLIC_SAMPLING_PERCENT).deconstruct() as u64
        }
        ChainKind::Private => {
            scaled(components.role, PRIVATE_ROLE_PERCENT).deconstruct() as u64
                + scaled(components.attestation, PRIVATE_ATTESTATION_PERCENT).deconstruct() as u64
                + scaled(components.performance, PRIVATE_PERFORMANCE_PERCENT).deconstruct() as u64
                + scaled(components.custody, PRIVATE_CUSTODY_PERCENT).deconstruct() as u64
        }
    };

    let total = Perbill::from_parts(total_parts.min(Perbill::one().deconstruct() as u64) as u32);

    match kind {
        ChainKind::PublicAccess => TrustScore {
            total,
            standing: components.standing,
            performance: components.performance,
            custody: components.custody,
            sampling: components.sampling,
            role: Perbill::zero(),
            attestation: Perbill::zero(),
        },
        ChainKind::Private => TrustScore {
            total,
            standing: Perbill::zero(),
            performance: components.performance,
            custody: components.custody,
            sampling: Perbill::zero(),
            role: components.role,
            attestation: components.attestation,
        },
    }
}

/// The shares one network reads, in percent, in composition order.
pub const fn shares(kind: ChainKind) -> [u32; 4] {
    match kind {
        ChainKind::PublicAccess => [
            PUBLIC_STANDING_PERCENT,
            PUBLIC_PERFORMANCE_PERCENT,
            PUBLIC_CUSTODY_PERCENT,
            PUBLIC_SAMPLING_PERCENT,
        ],
        ChainKind::Private => [
            PRIVATE_ROLE_PERCENT,
            PRIVATE_ATTESTATION_PERCENT,
            PRIVATE_PERFORMANCE_PERCENT,
            PRIVATE_CUSTODY_PERCENT,
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full() -> Components {
        Components {
            standing: Perbill::one(),
            performance: Perbill::one(),
            custody: Perbill::one(),
            sampling: Perbill::one(),
            role: Perbill::one(),
            attestation: Perbill::one(),
        }
    }

    #[test]
    fn each_network_weighs_its_components_to_one_whole() {
        assert_eq!(shares(ChainKind::PublicAccess).iter().sum::<u32>(), 100);
        assert_eq!(shares(ChainKind::Private).iter().sum::<u32>(), 100);
    }

    #[test]
    fn a_validator_meeting_every_component_reaches_the_full_score() {
        for kind in [ChainKind::PublicAccess, ChainKind::Private] {
            let score = compose(kind, full());
            assert_eq!(score.total, Perbill::one(), "{:?} reaches the whole", kind);
        }
    }

    #[test]
    fn a_validator_meeting_no_component_reaches_zero() {
        for kind in [ChainKind::PublicAccess, ChainKind::Private] {
            let score = compose(kind, Components::default());
            assert_eq!(score.total, Perbill::zero());
        }
    }

    #[test]
    fn the_public_access_shares_read_forty_thirty_twenty_ten() {
        let mut components = Components::default();
        components.standing = Perbill::one();
        assert_eq!(
            compose(ChainKind::PublicAccess, components).total,
            Perbill::from_percent(40)
        );

        let mut components = Components::default();
        components.performance = Perbill::one();
        assert_eq!(
            compose(ChainKind::PublicAccess, components).total,
            Perbill::from_percent(30)
        );

        let mut components = Components::default();
        components.custody = Perbill::one();
        assert_eq!(
            compose(ChainKind::PublicAccess, components).total,
            Perbill::from_percent(20)
        );

        let mut components = Components::default();
        components.sampling = Perbill::one();
        assert_eq!(
            compose(ChainKind::PublicAccess, components).total,
            Perbill::from_percent(10)
        );
    }

    #[test]
    fn the_private_shares_read_thirty_five_twenty_five_twenty_five_fifteen() {
        let mut components = Components::default();
        components.role = Perbill::one();
        assert_eq!(
            compose(ChainKind::Private, components).total,
            Perbill::from_percent(35)
        );

        let mut components = Components::default();
        components.attestation = Perbill::one();
        assert_eq!(
            compose(ChainKind::Private, components).total,
            Perbill::from_percent(25)
        );

        let mut components = Components::default();
        components.performance = Perbill::one();
        assert_eq!(
            compose(ChainKind::Private, components).total,
            Perbill::from_percent(25)
        );

        let mut components = Components::default();
        components.custody = Perbill::one();
        assert_eq!(
            compose(ChainKind::Private, components).total,
            Perbill::from_percent(15)
        );
    }

    #[test]
    fn the_public_access_composition_leaves_out_role_and_attestation() {
        let score = compose(ChainKind::PublicAccess, full());
        assert_eq!(score.role, Perbill::zero());
        assert_eq!(score.attestation, Perbill::zero());
    }

    #[test]
    fn the_private_composition_leaves_out_standing_and_sampling() {
        let score = compose(ChainKind::Private, full());
        assert_eq!(score.standing, Perbill::zero());
        assert_eq!(score.sampling, Perbill::zero());
    }

    #[test]
    fn custody_alone_never_reaches_the_whole_score() {
        // The retired model let one component carry the whole score. Custody
        // now carries a fifth on the public-access chain.
        let mut components = Components::default();
        components.custody = Perbill::one();
        let score = compose(ChainKind::PublicAccess, components);
        assert!(score.total < Perbill::one());
        assert_eq!(score.total, Perbill::from_percent(20));
    }

    #[test]
    fn a_missing_component_lowers_the_total_by_its_share() {
        let mut components = full();
        components.custody = Perbill::zero();
        let score = compose(ChainKind::PublicAccess, components);
        assert_eq!(score.total, Perbill::from_percent(80));
    }

    #[test]
    fn the_score_reads_as_a_voting_weight() {
        let score = compose(ChainKind::PublicAccess, full());
        assert_eq!(score.weight(), Perbill::one().deconstruct() as u128);
        assert_eq!(score.parts_per_billion(), Perbill::one().deconstruct());
    }

    #[test]
    fn every_network_carries_a_plain_sentence() {
        assert!(ChainKind::PublicAccess.plain_language().len() > 40);
        assert!(ChainKind::Private.plain_language().len() > 40);
    }
}
