#![cfg_attr(not(feature = "std"), no_std)]

/// A trait defining the interface needed by DCF to notify DVF of epoch transitions.
pub trait WeightFreezer<AccountId> {
    /// Freezes the validator stakes and weights for a new epoch in the DVF gadget.
    fn freeze_epoch_weights(epoch: u32, validators: &[(AccountId, u128, u128)]);
}

impl<AccountId> WeightFreezer<AccountId> for () {
    fn freeze_epoch_weights(_epoch: u32, _validators: &[(AccountId, u128, u128)]) {}
}
