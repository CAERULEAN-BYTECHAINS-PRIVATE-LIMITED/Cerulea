extern crate alloc;

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

/// Trait-friendly representation of a validator's state.
#[derive(codec::Encode, codec::Decode, Clone, PartialEq, Eq, sp_runtime::RuntimeDebug, scale_info::TypeInfo)]
pub struct ValidatorState {
    pub last_active_epoch: u32,
    pub current: crate::EpochStats,
    pub history: alloc::vec::Vec<crate::EpochStats>,
    pub uptime: u32,
    pub inference_success_count: u32,
    pub participation_rate: u32,
    pub inference_count: u64,
    pub last_active_block: u32,
    pub name: Option<alloc::vec::Vec<u8>>,
    pub trust_score: u64,
}

/// A trait defining the interface for querying and mutating validator registry state.
pub trait ValidatorRegistryProvider<AccountId, Balance, BlockNumber> {
    /// Returns a list of active validator account IDs.
    fn get_active_validators() -> alloc::vec::Vec<AccountId>;
    
    /// Returns the profile of a specific validator.
    fn get_validator_profile(validator: &AccountId) -> Option<crate::ValidatorProfile<AccountId, Balance, BlockNumber>>;
    
    /// Returns the status of a specific validator.
    fn get_validator_status(validator: &AccountId) -> Option<crate::ValidatorStatus>;

    /// Returns a list of all registered validator account IDs.
    fn get_validator_set() -> alloc::vec::Vec<AccountId>;

    /// Updates the validator set list.
    fn update_validator_set(set: alloc::vec::Vec<AccountId>);

    /// Checks if a validator is currently active.
    fn is_validator_active(validator: &AccountId) -> bool;

    /// Ejects a validator from the active set for a given reason.
    fn eject_validator(validator: &AccountId, reason: crate::EjectionReason) -> sp_runtime::DispatchResult;

    /// Returns the state of a specific validator.
    fn get_validator_state(validator: &AccountId) -> Option<ValidatorState>;

    /// Updates the state of a specific validator.
    fn update_validator_state(validator: &AccountId, state: ValidatorState);

    /// Checks if a validator state exists.
    fn contains_validator_state(validator: &AccountId) -> bool;

    /// Removes the state of a validator.
    fn remove_validator_state(validator: &AccountId);

    /// Returns the display name of a validator.
    fn get_validator_name(validator: &AccountId) -> Option<alloc::vec::Vec<u8>>;

    /// Sets the display name of a validator.
    fn set_validator_name(validator: &AccountId, name: alloc::vec::Vec<u8>);

    /// Returns the metadata of a validator.
    fn get_validator_metadata(validator: &AccountId) -> Option<crate::ValidatorMetadataInfo>;

    /// Sets the metadata of a validator.
    fn set_validator_metadata(validator: &AccountId, metadata: crate::ValidatorMetadataInfo);

    /// Removes the metadata of a validator.
    fn remove_validator_metadata(validator: &AccountId);

    /// Returns the detailed cooldown status of a validator.
    fn get_validator_detailed_cooldown_status(validator: &AccountId) -> Option<(u32, bool)>;

    /// Returns the block number when a leave request was made.
    fn get_validator_leave_request(validator: &AccountId) -> Option<u32>;

    /// Returns all pending validator actions (Join/Leave requests).
    fn get_pending_actions() -> alloc::vec::Vec<(AccountId, crate::ValidatorAction)>;

    /// Removes a pending validator action.
    fn remove_pending_action(validator: &AccountId);

    /// Adds a pending validator action.
    fn add_pending_action(validator: &AccountId, action: crate::ValidatorAction);

    /// Updates the active validator set list.
    fn update_active_validators(active: alloc::vec::Vec<AccountId>);

    /// Returns the block number when a validator joined.
    fn get_validator_join_time(validator: &AccountId) -> Option<u32>;

    /// Sets the block number when a validator joined.
    fn set_validator_join_time(validator: &AccountId, val: u32);

    /// Removes the block number when a validator joined.
    fn remove_validator_join_time(validator: &AccountId);

    /// Sets the block number when a leave request was made.
    fn set_validator_leave_request(validator: &AccountId, val: u32);

    /// Removes the block number when a leave request was made.
    fn remove_validator_leave_request(validator: &AccountId);

    /// Returns the block number when a validator was recently removed.
    fn get_recently_removed_validator(validator: &AccountId) -> Option<u32>;

    /// Sets the block number when a validator was recently removed.
    fn set_recently_removed_validator(validator: &AccountId, val: u32);

    /// Removes the block number when a validator was recently removed.
    fn remove_recently_removed_validator(validator: &AccountId);

    // Historical Performance & Metrics
    fn get_validator_performance_history(validator: &AccountId) -> alloc::vec::Vec<crate::PerformanceRecord>;
    fn set_validator_performance_history(validator: &AccountId, history: alloc::vec::Vec<crate::PerformanceRecord>);
    fn remove_validator_performance_history(validator: &AccountId);
    fn get_validator_last_seen(validator: &AccountId) -> u32;
    fn set_validator_last_seen(validator: &AccountId, val: u32);
    fn remove_validator_last_seen(validator: &AccountId);
    fn get_validator_blocks_authored(validator: &AccountId) -> u32;
    fn set_validator_blocks_authored(validator: &AccountId, val: u32);
    fn remove_validator_blocks_authored(validator: &AccountId);
    fn get_validator_blocks_missed(validator: &AccountId) -> u32;
    fn set_validator_blocks_missed(validator: &AccountId, val: u32);
    fn remove_validator_blocks_missed(validator: &AccountId);
}

/// No-op implementation of ValidatorRegistryProvider for unit tests and dummy configurations.
impl<AccountId, Balance, BlockNumber> ValidatorRegistryProvider<AccountId, Balance, BlockNumber> for () {
    fn get_active_validators() -> alloc::vec::Vec<AccountId> {
        alloc::vec::Vec::new()
    }
    
    fn get_validator_profile(_validator: &AccountId) -> Option<crate::ValidatorProfile<AccountId, Balance, BlockNumber>> {
        None
    }
    
    fn get_validator_status(_validator: &AccountId) -> Option<crate::ValidatorStatus> {
        None
    }

    fn get_validator_set() -> alloc::vec::Vec<AccountId> {
        alloc::vec::Vec::new()
    }

    fn update_validator_set(_set: alloc::vec::Vec<AccountId>) {}

    fn is_validator_active(_validator: &AccountId) -> bool {
        false
    }

    fn eject_validator(_validator: &AccountId, _reason: crate::EjectionReason) -> sp_runtime::DispatchResult {
        Ok(())
    }

    fn get_validator_state(_validator: &AccountId) -> Option<ValidatorState> {
        None
    }

    fn update_validator_state(_validator: &AccountId, _state: ValidatorState) {}

    fn contains_validator_state(_validator: &AccountId) -> bool {
        false
    }

    fn remove_validator_state(_validator: &AccountId) {}

    fn get_validator_name(_validator: &AccountId) -> Option<alloc::vec::Vec<u8>> {
        None
    }

    fn set_validator_name(_validator: &AccountId, _name: alloc::vec::Vec<u8>) {}

    fn get_validator_metadata(_validator: &AccountId) -> Option<crate::ValidatorMetadataInfo> {
        None
    }

    fn set_validator_metadata(_validator: &AccountId, _metadata: crate::ValidatorMetadataInfo) {}

    fn remove_validator_metadata(_validator: &AccountId) {}

    fn get_validator_detailed_cooldown_status(_validator: &AccountId) -> Option<(u32, bool)> {
        None
    }

    fn get_validator_leave_request(_validator: &AccountId) -> Option<u32> {
        None
    }

    fn get_pending_actions() -> alloc::vec::Vec<(AccountId, crate::ValidatorAction)> {
        alloc::vec::Vec::new()
    }

    fn remove_pending_action(_validator: &AccountId) {}

    fn add_pending_action(_validator: &AccountId, _action: crate::ValidatorAction) {}

    fn update_active_validators(_active: alloc::vec::Vec<AccountId>) {}

    fn get_validator_join_time(_validator: &AccountId) -> Option<u32> {
        None
    }

    fn set_validator_join_time(_validator: &AccountId, _val: u32) {}

    fn remove_validator_join_time(_validator: &AccountId) {}

    fn set_validator_leave_request(_validator: &AccountId, _val: u32) {}

    fn remove_validator_leave_request(_validator: &AccountId) {}

    fn get_recently_removed_validator(_validator: &AccountId) -> Option<u32> {
        None
    }

    fn set_recently_removed_validator(_validator: &AccountId, _val: u32) {}

    fn remove_recently_removed_validator(_validator: &AccountId) {}

    fn get_validator_performance_history(_validator: &AccountId) -> alloc::vec::Vec<crate::PerformanceRecord> {
        alloc::vec::Vec::new()
    }
    fn set_validator_performance_history(_validator: &AccountId, _history: alloc::vec::Vec<crate::PerformanceRecord>) {}
    fn remove_validator_performance_history(_validator: &AccountId) {}
    fn get_validator_last_seen(_validator: &AccountId) -> u32 { 0 }
    fn set_validator_last_seen(_validator: &AccountId, _val: u32) {}
    fn remove_validator_last_seen(_validator: &AccountId) {}
    fn get_validator_blocks_authored(_validator: &AccountId) -> u32 { 0 }
    fn set_validator_blocks_authored(_validator: &AccountId, _val: u32) {}
    fn remove_validator_blocks_authored(_validator: &AccountId) {}
    fn get_validator_blocks_missed(_validator: &AccountId) -> u32 { 0 }
    fn set_validator_blocks_missed(_validator: &AccountId, _val: u32) {}
    fn remove_validator_blocks_missed(_validator: &AccountId) {}
}
