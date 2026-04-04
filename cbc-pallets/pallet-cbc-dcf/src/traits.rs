#![cfg_attr(not(feature = "std"), no_std)]

/// A trait defining the interface needed by DCF to notify DVF of epoch transitions.
pub trait WeightFreezer<AccountId> {
    /// Freezes the validator stakes and weights for a new epoch in the DVF gadget.
    fn freeze_epoch_weights(epoch: u32, validators: &[(AccountId, u128, u128)]);
}

impl<AccountId> WeightFreezer<AccountId> for () {
    fn freeze_epoch_weights(_epoch: u32, _validators: &[(AccountId, u128, u128)]) {}
}

/// A trait for querying the last DVF-finalized block number.
///
/// DCF progressive finality must not advance the hard finalized head past the
/// last DVF-finalized block, so that DVF remains the primary finality authority.
pub trait DvfFinalizedBlockProvider {
    /// Returns the highest block number that has been finalized by DVF.
    /// Returns 0 if no block has been DVF-finalized yet.
    fn dvf_finalized_block() -> u32;
}

/// No-op implementation: treats every block as DVF-finalized (no restriction).
/// Used in tests and when DVF is not configured.
impl DvfFinalizedBlockProvider for () {
    fn dvf_finalized_block() -> u32 {
        u32::MAX
    }
}
