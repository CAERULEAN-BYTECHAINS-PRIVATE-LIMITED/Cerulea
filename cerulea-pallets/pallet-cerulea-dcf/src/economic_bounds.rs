//! Economic bounds and overflow protection for DCF pallet
//!
//! This module implements comprehensive bounds checking and overflow protection
//! for all economic operations including slashing, rewards, and balance calculations.

use super::*;
use frame_support::traits::{Get, Currency, ReservableCurrency};
use sp_runtime::traits::{Saturating, CheckedAdd, CheckedSub, CheckedMul, CheckedDiv, Zero};
use sp_std::collections::btree_map::BTreeMap;

// Type alias to resolve ambiguous Balance type
type BalanceOf<T> = <T as pallet::Config>::Balance;

/// Per-epoch economic bounds configuration
#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub struct EpochEconomicBounds<Balance> {
    /// Maximum total slashing amount per epoch across all validators
    pub max_total_slashing_per_epoch: Balance,
    /// Maximum slashing amount per validator per epoch
    pub max_validator_slashing_per_epoch: Balance,
    /// Maximum total reward amount per epoch across all validators
    pub max_total_rewards_per_epoch: Balance,
    /// Maximum reward amount per validator per epoch
    pub max_validator_reward_per_epoch: Balance,
    /// Maximum percentage of stake that can be slashed per epoch (in basis points)
    pub max_slashing_percentage_per_epoch: u32,
    /// Current epoch for these bounds
    pub epoch: u32,
}

impl<Balance: Default> Default for EpochEconomicBounds<Balance> {
    fn default() -> Self {
        Self {
            max_total_slashing_per_epoch: Balance::default(),
            max_validator_slashing_per_epoch: Balance::default(),
            max_total_rewards_per_epoch: Balance::default(),
            max_validator_reward_per_epoch: Balance::default(),
            max_slashing_percentage_per_epoch: 3000, // 30% max per epoch
            epoch: 0,
        }
    }
}

/// Per-epoch economic tracking for bounds enforcement
#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub struct EpochEconomicTracker<Balance> {
    /// Current epoch being tracked
    pub epoch: u32,
    /// Total slashing amount applied this epoch
    pub total_slashing_this_epoch: Balance,
    /// Total rewards distributed this epoch
    pub total_rewards_this_epoch: Balance,
    /// Per-validator slashing amounts this epoch
    pub validator_slashing_this_epoch: BTreeMap<Vec<u8>, Balance>, // AccountId encoded as Vec<u8>
    /// Per-validator reward amounts this epoch
    pub validator_rewards_this_epoch: BTreeMap<Vec<u8>, Balance>, // AccountId encoded as Vec<u8>
    /// Block number when epoch started
    pub epoch_start_block: u32,
}

impl<Balance: Default + Zero> Default for EpochEconomicTracker<Balance> {
    fn default() -> Self {
        Self {
            epoch: 0,
            total_slashing_this_epoch: Balance::zero(),
            total_rewards_this_epoch: Balance::zero(),
            validator_slashing_this_epoch: BTreeMap::new(),
            validator_rewards_this_epoch: BTreeMap::new(),
            epoch_start_block: 0,
        }
    }
}

/// Economic operation result with overflow protection
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EconomicOperationResult<Balance> {
    /// Operation completed successfully
    Success {
        /// Amount actually processed (may be less than requested due to bounds)
        amount_processed: Balance,
        /// Pre-operation balance
        balance_before: Balance,
        /// Post-operation balance
        balance_after: Balance,
        /// Reason code for the operation
        reason_code: EconomicReasonCode,
    },
    /// Operation failed due to bounds violation
    BoundsViolation {
        /// Requested amount
        requested_amount: Balance,
        /// Maximum allowed amount
        max_allowed: Balance,
        /// Type of bounds violation
        violation_type: BoundsViolationType,
    },
    /// Operation failed due to arithmetic overflow/underflow
    ArithmeticError {
        /// Type of arithmetic error
        error_type: ArithmeticErrorType,
        /// Values involved in the operation
        operands: Vec<Balance>,
    },
    /// Operation failed due to insufficient balance
    InsufficientBalance {
        /// Available balance
        available: Balance,
        /// Requested amount
        requested: Balance,
    },
}

/// Reason codes for economic operations
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, TypeInfo, MaxEncodedLen)]
pub enum EconomicReasonCode {
    /// Regular slashing for misbehavior
    MisbehaviorSlashing,
    /// Automatic slashing due to threshold breach
    ThresholdSlashing,
    /// Manual slashing via governance
    GovernanceSlashing,
    /// Performance-based reward
    PerformanceReward,
    /// Base epoch reward
    BaseReward,
    /// Top performer bonus
    TopPerformerBonus,
    /// Manual reward via governance
    GovernanceReward,
    /// Stake reservation for validator joining
    StakeReservation,
    /// Stake unreservation for validator leaving
    StakeUnreservation,
}

/// Types of bounds violations
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundsViolationType {
    /// Per-epoch total slashing limit exceeded
    TotalSlashingPerEpoch,
    /// Per-validator per-epoch slashing limit exceeded
    ValidatorSlashingPerEpoch,
    /// Per-epoch total rewards limit exceeded
    TotalRewardsPerEpoch,
    /// Per-validator per-epoch rewards limit exceeded
    ValidatorRewardsPerEpoch,
    /// Slashing percentage limit exceeded
    SlashingPercentageLimit,
}

/// Types of arithmetic errors
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArithmeticErrorType {
    /// Addition would overflow
    AdditionOverflow,
    /// Subtraction would underflow
    SubtractionUnderflow,
    /// Multiplication would overflow
    MultiplicationOverflow,
    /// Division by zero
    DivisionByZero,
}

impl<T: Config> Pallet<T> {
    /// Apply slashing with comprehensive bounds checking and overflow protection
    pub fn apply_slashing_with_bounds(
        validator: &T::AccountId,
        amount: T::Balance,
        reason: EconomicReasonCode,
    ) -> Result<EconomicOperationResult<T::Balance>, Error<T>> {
        let current_epoch = Self::current_epoch();
        let bounds = Self::get_epoch_economic_bounds(current_epoch);
        let mut tracker = Self::get_epoch_economic_tracker(current_epoch);

        // Check per-epoch total slashing bounds
        let new_total_slashing = tracker.total_slashing_this_epoch
            .checked_add(&amount)
            .ok_or(Error::<T>::ArithmeticOverflow)?;

        if new_total_slashing > bounds.max_total_slashing_per_epoch {
            return Ok(EconomicOperationResult::BoundsViolation {
                requested_amount: amount,
                max_allowed: bounds.max_total_slashing_per_epoch
                    .checked_sub(&tracker.total_slashing_this_epoch)
                    .unwrap_or_else(T::Balance::zero),
                violation_type: BoundsViolationType::TotalSlashingPerEpoch,
            });
        }

        // Check per-validator per-epoch slashing bounds
        let validator_key = validator.encode();
        let current_validator_slashing = tracker.validator_slashing_this_epoch
            .get(&validator_key)
            .cloned()
            .unwrap_or_else(T::Balance::zero);

        let new_validator_slashing = current_validator_slashing
            .checked_add(&amount)
            .ok_or(Error::<T>::ArithmeticOverflow)?;

        if new_validator_slashing > bounds.max_validator_slashing_per_epoch {
            return Ok(EconomicOperationResult::BoundsViolation {
                requested_amount: amount,
                max_allowed: bounds.max_validator_slashing_per_epoch
                    .checked_sub(&current_validator_slashing)
                    .unwrap_or_else(T::Balance::zero),
                violation_type: BoundsViolationType::ValidatorSlashingPerEpoch,
            });
        }

        // Check slashing percentage bounds
        let reserved_balance = T::Currency::reserved_balance(validator);
        if !reserved_balance.is_zero() {
            let slashing_percentage = amount
                .checked_mul(&10000u32.into()) // Convert to basis points
                .and_then(|x| x.checked_div(&reserved_balance))
                .ok_or(Error::<T>::ArithmeticOverflow)?;

            let slashing_percentage_u32: u32 = slashing_percentage
                .try_into()
                .map_err(|_| Error::<T>::ArithmeticOverflow)?;

            if slashing_percentage_u32 > bounds.max_slashing_percentage_per_epoch {
                return Ok(EconomicOperationResult::BoundsViolation {
                    requested_amount: amount,
                    max_allowed: reserved_balance
                        .checked_mul(&bounds.max_slashing_percentage_per_epoch.into())
                        .and_then(|x| x.checked_div(&10000u32.into()))
                        .unwrap_or_else(T::Balance::zero),
                    violation_type: BoundsViolationType::SlashingPercentageLimit,
                });
            }
        }

        // Check sufficient balance
        if reserved_balance < amount {
            return Ok(EconomicOperationResult::InsufficientBalance {
                available: reserved_balance,
                requested: amount,
            });
        }

        // Apply the slashing
        let balance_before = reserved_balance;
        T::Currency::slash_reserved(validator, amount);
        let balance_after = T::Currency::reserved_balance(validator);

        // Update tracking
        tracker.total_slashing_this_epoch = new_total_slashing;
        tracker.validator_slashing_this_epoch.insert(validator_key, new_validator_slashing);
        Self::set_epoch_economic_tracker(current_epoch, tracker);

        // Emit detailed slashing event
        Self::deposit_event(Event::ValidatorSlashedWithDetails {
            validator: validator.clone(),
            amount,
            balance_before,
            balance_after,
            reason_code: reason.clone(),
            epoch: current_epoch,
        });

        Ok(EconomicOperationResult::Success {
            amount_processed: amount,
            balance_before,
            balance_after,
            reason_code: reason,
        })
    }

    /// Apply rewards with comprehensive bounds checking and overflow protection
    pub fn apply_reward_with_bounds(
        validator: &T::AccountId,
        amount: T::Balance,
        reason: EconomicReasonCode,
    ) -> Result<EconomicOperationResult<T::Balance>, Error<T>> {
        let current_epoch = Self::current_epoch();
        let bounds = Self::get_epoch_economic_bounds(current_epoch);
        let mut tracker = Self::get_epoch_economic_tracker(current_epoch);

        // Check per-epoch total rewards bounds
        let new_total_rewards = tracker.total_rewards_this_epoch
            .checked_add(&amount)
            .ok_or(Error::<T>::ArithmeticOverflow)?;

        if new_total_rewards > bounds.max_total_rewards_per_epoch {
            return Ok(EconomicOperationResult::BoundsViolation {
                requested_amount: amount,
                max_allowed: bounds.max_total_rewards_per_epoch
                    .checked_sub(&tracker.total_rewards_this_epoch)
                    .unwrap_or_else(T::Balance::zero),
                violation_type: BoundsViolationType::TotalRewardsPerEpoch,
            });
        }

        // Check per-validator per-epoch rewards bounds
        let validator_key = validator.encode();
        let current_validator_rewards = tracker.validator_rewards_this_epoch
            .get(&validator_key)
            .cloned()
            .unwrap_or_else(T::Balance::zero);

        let new_validator_rewards = current_validator_rewards
            .checked_add(&amount)
            .ok_or(Error::<T>::ArithmeticOverflow)?;

        if new_validator_rewards > bounds.max_validator_reward_per_epoch {
            return Ok(EconomicOperationResult::BoundsViolation {
                requested_amount: amount,
                max_allowed: bounds.max_validator_reward_per_epoch
                    .checked_sub(&current_validator_rewards)
                    .unwrap_or_else(T::Balance::zero),
                violation_type: BoundsViolationType::ValidatorRewardsPerEpoch,
            });
        }

        // Check for balance overflow
        let current_balance = T::Currency::free_balance(validator);
        let new_balance = current_balance
            .checked_add(&amount)
            .ok_or(Error::<T>::ArithmeticOverflow)?;

        // Apply the reward
        let balance_before = current_balance;
        T::Currency::deposit_creating(validator, amount);
        let balance_after = T::Currency::free_balance(validator);

        // Update tracking
        tracker.total_rewards_this_epoch = new_total_rewards;
        tracker.validator_rewards_this_epoch.insert(validator_key, new_validator_rewards);
        Self::set_epoch_economic_tracker(current_epoch, tracker);

        // Emit detailed reward event
        Self::deposit_event(Event::ValidatorRewardedWithDetails {
            validator: validator.clone(),
            amount,
            balance_before,
            balance_after,
            reason_code: reason.clone(),
            epoch: current_epoch,
        });

        Ok(EconomicOperationResult::Success {
            amount_processed: amount,
            balance_before,
            balance_after,
            reason_code: reason,
        })
    }

    /// Get economic bounds for a specific epoch
    pub fn get_epoch_economic_bounds(epoch: u32) -> EpochEconomicBounds<T::Balance> {
        EpochEconomicBoundsStorage::<T>::get(epoch).unwrap_or_else(|| {
            // Generate default bounds based on current configuration
            let total_stake: T::Balance = Self::validator_set()
                .iter()
                .map(|v| T::Currency::reserved_balance(v))
                .fold(T::Balance::zero(), |acc, stake| acc.saturating_add(stake));

            let max_validators = T::MaxValidators::get();
            let default_reward = T::ValidatorReward::get();

            EpochEconomicBounds {
                max_total_slashing_per_epoch: total_stake
                    .checked_div(&10u32.into()) // 10% of total stake max
                    .unwrap_or_else(|| total_stake),
                max_validator_slashing_per_epoch: T::MinStake::get()
                    .checked_mul(&5u32.into()) // 5x minimum stake max
                    .unwrap_or_else(|| T::MinStake::get()),
                max_total_rewards_per_epoch: default_reward
                    .checked_mul(&max_validators.into())
                    .unwrap_or_else(|| default_reward),
                max_validator_reward_per_epoch: default_reward
                    .checked_mul(&10u32.into()) // 10x default reward max
                    .unwrap_or_else(|| default_reward),
                max_slashing_percentage_per_epoch: 3000, // 30% max
                epoch,
            }
        })
    }

    /// Get economic tracker for a specific epoch
    pub fn get_epoch_economic_tracker(epoch: u32) -> EpochEconomicTracker<T::Balance> {
        EpochEconomicTrackerStorage::<T>::get(epoch).unwrap_or_else(|| {
            EpochEconomicTracker {
                epoch,
                total_slashing_this_epoch: T::Balance::zero(),
                total_rewards_this_epoch: T::Balance::zero(),
                validator_slashing_this_epoch: BTreeMap::new(),
                validator_rewards_this_epoch: BTreeMap::new(),
                epoch_start_block: Self::current_block_number(),
            }
        })
    }

    /// Set economic tracker for a specific epoch
    pub fn set_epoch_economic_tracker(
        epoch: u32,
        tracker: EpochEconomicTracker<T::Balance>,
    ) {
        EpochEconomicTrackerStorage::<T>::insert(epoch, tracker);
    }

    /// Reset economic tracking for new epoch
    pub fn reset_epoch_economic_tracking(new_epoch: u32) {
        let tracker = EpochEconomicTracker {
            epoch: new_epoch,
            total_slashing_this_epoch: T::Balance::zero(),
            total_rewards_this_epoch: T::Balance::zero(),
            validator_slashing_this_epoch: BTreeMap::new(),
            validator_rewards_this_epoch: BTreeMap::new(),
            epoch_start_block: Self::current_block_number(),
        };
        Self::set_epoch_economic_tracker(new_epoch, tracker);
    }

    /// Validate economic bounds configuration
    pub fn validate_economic_bounds(
        bounds: &EpochEconomicBounds<T::Balance>,
    ) -> Result<(), Error<T>> {
        // Ensure bounds are non-zero and reasonable
        if bounds.max_total_slashing_per_epoch.is_zero() {
            return Err(Error::<T>::InvalidEpochConfig);
        }

        if bounds.max_validator_slashing_per_epoch.is_zero() {
            return Err(Error::<T>::InvalidEpochConfig);
        }

        if bounds.max_total_rewards_per_epoch.is_zero() {
            return Err(Error::<T>::InvalidEpochConfig);
        }

        if bounds.max_validator_reward_per_epoch.is_zero() {
            return Err(Error::<T>::InvalidEpochConfig);
        }

        // Ensure percentage is reasonable (0-100%)
        if bounds.max_slashing_percentage_per_epoch > 10000 {
            return Err(Error::<T>::InvalidEpochConfig);
        }

        // Ensure per-validator limits don't exceed total limits
        let max_validators = T::MaxValidators::get();
        let theoretical_max_validator_slashing = bounds.max_validator_slashing_per_epoch
            .checked_mul(&max_validators.into())
            .ok_or(Error::<T>::ArithmeticOverflow)?;

        if theoretical_max_validator_slashing < bounds.max_total_slashing_per_epoch {
            return Err(Error::<T>::InvalidEpochConfig);
        }

        let theoretical_max_validator_rewards = bounds.max_validator_reward_per_epoch
            .checked_mul(&max_validators.into())
            .ok_or(Error::<T>::ArithmeticOverflow)?;

        if theoretical_max_validator_rewards < bounds.max_total_rewards_per_epoch {
            return Err(Error::<T>::InvalidEpochConfig);
        }

        Ok(())
    }

    /// Get current block number
    fn current_block_number() -> u32 {
        <frame_system::Pallet<T>>::block_number().saturated_into()
    }
}

/// Storage items for economic bounds and tracking
#[frame_support::pallet]
pub mod economic_storage {
    use super::*;

    /// Economic bounds configuration per epoch
    #[pallet::storage]
    #[pallet::getter(fn epoch_economic_bounds)]
    pub type EpochEconomicBoundsStorage<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u32, // epoch
        EpochEconomicBounds<T::Balance>,
        OptionQuery,
    >;

    /// Economic tracking per epoch
    #[pallet::storage]
    #[pallet::getter(fn epoch_economic_tracker)]
    pub type EpochEconomicTrackerStorage<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u32, // epoch
        EpochEconomicTracker<T::Balance>,
        OptionQuery,
    >;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::*;
    use frame_support::{assert_ok, assert_noop};

    #[test]
    fn test_slashing_bounds_enforcement() {
        new_test_ext().execute_with(|| {
            // Setup validator
            let validator = 1u64;
            let _ = Balances::make_free_balance_be(&validator, 100_000_000);
            assert_ok!(DcfModule::join_validators(RuntimeOrigin::signed(validator)));

            // Set restrictive bounds
            let bounds = EpochEconomicBounds {
                max_total_slashing_per_epoch: 1_000_000,
                max_validator_slashing_per_epoch: 500_000,
                max_total_rewards_per_epoch: 1_000_000,
                max_validator_reward_per_epoch: 500_000,
                max_slashing_percentage_per_epoch: 1000, // 10%
                epoch: 1,
            };
            EpochEconomicBoundsStorage::<Test>::insert(1, bounds);

            // Test slashing within bounds
            let result = DcfModule::apply_slashing_with_bounds(
                &validator,
                400_000,
                EconomicReasonCode::MisbehaviorSlashing,
            ).unwrap();

            match result {
                EconomicOperationResult::Success { amount_processed, .. } => {
                    assert_eq!(amount_processed, 400_000);
                },
                _ => panic!("Expected successful slashing"),
            }

            // Test slashing exceeding validator bounds
            let result = DcfModule::apply_slashing_with_bounds(
                &validator,
                200_000, // Would exceed 500_000 limit
                EconomicReasonCode::MisbehaviorSlashing,
            ).unwrap();

            match result {
                EconomicOperationResult::BoundsViolation { violation_type, .. } => {
                    assert_eq!(violation_type, BoundsViolationType::ValidatorSlashingPerEpoch);
                },
                _ => panic!("Expected bounds violation"),
            }
        });
    }

    #[test]
    fn test_reward_bounds_enforcement() {
        new_test_ext().execute_with(|| {
            // Setup validator
            let validator = 1u64;
            let _ = Balances::make_free_balance_be(&validator, 100_000_000);
            assert_ok!(DcfModule::join_validators(RuntimeOrigin::signed(validator)));

            // Set restrictive bounds
            let bounds = EpochEconomicBounds {
                max_total_slashing_per_epoch: 1_000_000,
                max_validator_slashing_per_epoch: 500_000,
                max_total_rewards_per_epoch: 1_000_000,
                max_validator_reward_per_epoch: 500_000,
                max_slashing_percentage_per_epoch: 1000,
                epoch: 1,
            };
            EpochEconomicBoundsStorage::<Test>::insert(1, bounds);

            // Test reward within bounds
            let result = DcfModule::apply_reward_with_bounds(
                &validator,
                400_000,
                EconomicReasonCode::PerformanceReward,
            ).unwrap();

            match result {
                EconomicOperationResult::Success { amount_processed, .. } => {
                    assert_eq!(amount_processed, 400_000);
                },
                _ => panic!("Expected successful reward"),
            }

            // Test reward exceeding bounds
            let result = DcfModule::apply_reward_with_bounds(
                &validator,
                200_000, // Would exceed 500_000 limit
                EconomicReasonCode::PerformanceReward,
            ).unwrap();

            match result {
                EconomicOperationResult::BoundsViolation { violation_type, .. } => {
                    assert_eq!(violation_type, BoundsViolationType::ValidatorRewardsPerEpoch);
                },
                _ => panic!("Expected bounds violation"),
            }
        });
    }

    #[test]
    fn test_arithmetic_overflow_protection() {
        new_test_ext().execute_with(|| {
            let validator = 1u64;
            let _ = Balances::make_free_balance_be(&validator, u128::MAX);

            // Test overflow protection in reward calculation
            let result = DcfModule::apply_reward_with_bounds(
                &validator,
                1, // Small amount that would cause overflow when added to MAX
                EconomicReasonCode::PerformanceReward,
            );

            // Should handle overflow gracefully
            assert!(result.is_err() || matches!(result.unwrap(), EconomicOperationResult::ArithmeticError { .. }));
        });
    }
}