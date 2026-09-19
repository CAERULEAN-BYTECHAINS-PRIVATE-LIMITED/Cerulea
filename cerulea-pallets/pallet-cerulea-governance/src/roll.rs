//! The electorate a tally reads.
//!
//! A vote carries weight. The weight of one account and the weight of the
//! whole electorate together decide whether a proposal reaches the
//! threshold of its class.
//!
//! The module keeps its own register of weights. A deployment that draws
//! its electorate from another source supplies a different implementation
//! of [`VoterRoll`] to the runtime.

use core::marker::PhantomData;
use pallet_cerulea_schema::primitives::DeploymentId;

use crate::{Config, Electorate, RollWeight};

/// The electorate one deployment votes with.
pub trait VoterRoll {
    /// The account type the runtime carries.
    type AccountId;

    /// The weight one account carries. An account outside the electorate
    /// carries zero.
    fn weight_of(deployment: &DeploymentId, who: &Self::AccountId) -> u64;

    /// The weight of the whole electorate.
    fn total_weight(deployment: &DeploymentId) -> u64;

    /// Report whether one account votes in this deployment.
    fn is_member(deployment: &DeploymentId, who: &Self::AccountId) -> bool {
        Self::weight_of(deployment, who) > 0
    }
}

/// The electorate the governance module registers itself.
///
/// The steward gives each account its weight through `set_voter_weight`.
/// A validator set change carried out by the electorate then changes those
/// weights through the same call.
pub struct RegisteredElectorate<T>(PhantomData<T>);

impl<T: Config> VoterRoll for RegisteredElectorate<T> {
    type AccountId = T::AccountId;

    fn weight_of(deployment: &DeploymentId, who: &Self::AccountId) -> u64 {
        Electorate::<T>::get(deployment, who).unwrap_or(0)
    }

    fn total_weight(deployment: &DeploymentId) -> u64 {
        RollWeight::<T>::get(deployment)
    }
}
