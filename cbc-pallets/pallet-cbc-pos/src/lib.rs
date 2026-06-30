#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

pub mod weights;
pub use weights::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

use sp_std::prelude::*;
use sp_runtime::traits::{Saturating, SaturatedConversion};
use frame_support::traits::{Currency, ReservableCurrency, Get};

/// Reasons for slashing validator stakes as punishment for violations.
#[derive(codec::Encode, codec::Decode, Clone, PartialEq, Eq, sp_runtime::RuntimeDebug, scale_info::TypeInfo, codec::MaxEncodedLen)]
pub enum RewardReason {
    ExceptionalPerformance,
    BlockAuthorship,
    ManualReward,
    InferenceQuality,
    EpochPerformance,
}

/// Reasons for validator slashing with different severity levels and recovery requirements.
#[derive(codec::Encode, codec::Decode, Clone, PartialEq, Eq, sp_runtime::RuntimeDebug, scale_info::TypeInfo, codec::MaxEncodedLen)]
pub enum SlashReason {
    Misbehavior,
    PoorPerformance,
    ManualSlash,
    ConsensusViolation,
}

pub trait ValidatorHandler<AccountId, Balance> {
    fn on_joined(validator: &AccountId, stake: Balance) -> sp_runtime::DispatchResult;
    fn on_leave_requested(validator: &AccountId) -> sp_runtime::DispatchResult;
    fn on_left(validator: &AccountId) -> sp_runtime::DispatchResult;
    fn on_stake_increased(validator: &AccountId, amount: Balance) -> sp_runtime::DispatchResult;
    fn on_stake_decreased(validator: &AccountId, amount: Balance) -> sp_runtime::DispatchResult;
    fn on_slashed(validator: &AccountId, amount: Balance, penalty: u64) -> sp_runtime::DispatchResult;
    fn on_rewarded(validator: &AccountId, amount: Balance, boost: u64) -> sp_runtime::DispatchResult;
    fn get_validator_score(validator: &AccountId) -> u64;
    fn get_active_validators() -> Vec<AccountId>;
}

impl<AccountId, Balance> ValidatorHandler<AccountId, Balance> for () {
    fn on_joined(_validator: &AccountId, _stake: Balance) -> sp_runtime::DispatchResult { Ok(()) }
    fn on_leave_requested(_validator: &AccountId) -> sp_runtime::DispatchResult { Ok(()) }
    fn on_left(_validator: &AccountId) -> sp_runtime::DispatchResult { Ok(()) }
    fn on_stake_increased(_validator: &AccountId, _amount: Balance) -> sp_runtime::DispatchResult { Ok(()) }
    fn on_stake_decreased(_validator: &AccountId, _amount: Balance) -> sp_runtime::DispatchResult { Ok(()) }
    fn on_slashed(_validator: &AccountId, _amount: Balance, _penalty: u64) -> sp_runtime::DispatchResult { Ok(()) }
    fn on_rewarded(_validator: &AccountId, _amount: Balance, _boost: u64) -> sp_runtime::DispatchResult { Ok(()) }
    fn get_validator_score(_validator: &AccountId) -> u64 { 0 }
    fn get_active_validators() -> Vec<AccountId> { Vec::new() }
}

// Runtime API declaration
sp_api::decl_runtime_apis! {
    pub trait PosApi<AccountId, Balance> 
    where
        AccountId: codec::Codec + Clone + Eq + sp_std::fmt::Debug,
        Balance: codec::Codec + Clone + Eq + sp_runtime::traits::AtLeast32BitUnsigned,
    {
        fn get_validator_stake(validator: AccountId) -> Balance;
        fn get_validator_score(validator: AccountId) -> u32;
        fn get_active_validators() -> Vec<AccountId>;
        fn get_slashing_count(validator: AccountId) -> u32;
    }
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::{pallet_prelude::*, storage::types::{StorageMap, StorageValue}};
    use frame_system::pallet_prelude::*;
    use scale_info::prelude::vec::Vec;
    use sp_runtime::traits::AtLeast32BitUnsigned;
    use codec::MaxEncodedLen;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type WeightInfo: WeightInfo;

        /// Minimum score required for a validator to be considered active
        type MinValidatorScore: Get<u32>;
        /// Minimum number of active validators required
        type MinActiveValidators: Get<u32>;
        /// Maximum number of validators allowed
        type MaxValidators: Get<u32>;
        /// Score decay per epoch
        type ValidatorScoreDecay: Get<u32>;
        /// Maximum slashing count before removal
        type MaxSlashingCount: Get<u32>;
        /// Minimum stake amount required for validators
        type MinStake: Get<BalanceOf<Self>>;
        /// The balance type
        type Balance: Parameter + Member + AtLeast32BitUnsigned + Default + Copy + MaxEncodedLen;
        
        type Currency: Currency<Self::AccountId, Balance = BalanceOf<Self>> + ReservableCurrency<Self::AccountId>;
        
        #[pallet::constant]
        type LeaveCooldown: Get<u32>;
        
        #[pallet::constant]
        type ValidatorReward: Get<BalanceOf<Self>>;
        
        #[pallet::constant]
        type SlashPercent: Get<u32>;
        
        #[pallet::constant]
        type MaxSlashPerEpoch: Get<BalanceOf<Self>>;
        
        #[pallet::constant]
        type MaxSlashPerValidator: Get<BalanceOf<Self>>;
        
        #[pallet::constant]
        type MaxRewardPerEpoch: Get<BalanceOf<Self>>;
        
        #[pallet::constant]
        type MaxRewardPerValidator: Get<BalanceOf<Self>>;
        
        #[pallet::constant]
        type SlashPenaltyDivisor: Get<u64>;
        
        #[pallet::constant]
        type MaxSlashPenalty: Get<u64>;
        
        #[pallet::constant]
        type RewardBoostDivisor: Get<u64>;
        
        #[pallet::constant]
        type MaxRewardBoost: Get<u64>;
        
        #[pallet::constant]
        type HighPerformanceScore: Get<u64>;
        
        #[pallet::constant]
        type TopPerformerPercentage: Get<u32>;
        
        type ValidatorHandler: ValidatorHandler<Self::AccountId, BalanceOf<Self>>;
    }

    type BalanceOf<T> = <T as Config>::Balance;

    #[pallet::genesis_config]
    #[derive(frame_support::DefaultNoBound)]
    pub struct GenesisConfig<T: Config> {
        pub validators: Vec<T::AccountId>,
        pub validator_scores: Vec<u32>,
        pub current_epoch: u32,
        pub slashing_count: Vec<(T::AccountId, u32)>,
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            // Initialize validators
            for validator in &self.validators {
                Validators::<T>::insert(validator, true);
            }

            // Initialize validator scores
            for (validator, score) in self.validators.iter().zip(self.validator_scores.iter()) {
                ValidatorScores::<T>::insert(validator, score);
            }

            // Initialize current epoch
            CurrentEpoch::<T>::put(self.current_epoch);

            // Initialize slashing counts
            for (validator, count) in &self.slashing_count {
                SlashingCount::<T>::insert(validator, count);
            }
        }
    }

    #[pallet::storage]
    #[pallet::getter(fn validators)]
    pub type Validators<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, bool>;

    #[pallet::storage]
    #[pallet::getter(fn validator_scores)]
    pub type ValidatorScores<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, u32>;

    #[pallet::storage]
    #[pallet::getter(fn current_epoch)]
    pub type CurrentEpoch<T: Config> = StorageValue<_, u32, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn slashing_count)]
    pub type SlashingCount<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, u32>;

    #[pallet::storage]
    #[pallet::getter(fn stake)]
    pub type Stake<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, BalanceOf<T>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn validator_leave_requests)]
    pub type ValidatorLeaveRequests<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        u32,
        OptionQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn recently_removed_validators)]
    pub type RecentlyRemovedValidators<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        u32,
        OptionQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn validator_join_time)]
    pub type ValidatorJoinTime<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        u32,
        OptionQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn epoch_total_rewarded)]
    pub type EpochTotalRewarded<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn epoch_total_slashed)]
    pub type EpochTotalSlashed<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn validator_epoch_rewarded)]
    pub type ValidatorEpochRewarded<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        BalanceOf<T>,
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn validator_epoch_slashed)]
    pub type ValidatorEpochSlashed<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        BalanceOf<T>,
        ValueQuery,
    >;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        ValidatorRegistered { validator: T::AccountId },
        ScoreSubmitted { validator: T::AccountId, score: u32 },
        ValidatorSlashed { validator: T::AccountId, slashing_count: u32 },
        ValidatorRemoved { validator: T::AccountId, reason: Vec<u8> },
        StakeBonded { validator: T::AccountId, amount: BalanceOf<T> },
        StakeUnbonded { validator: T::AccountId, amount: BalanceOf<T> },
        ScoreBoosted { 
            validator: T::AccountId, 
            old_score: u32, 
            new_score: u32, 
            boost_amount: u32 
        },
        ScoreSlashed { 
            validator: T::AccountId, 
            old_score: u32, 
            new_score: u32, 
            slash_amount: u32 
        },
        ValidatorStakeSlashed { 
            validator: T::AccountId, 
            old_stake: BalanceOf<T>, 
            new_stake: BalanceOf<T>, 
            slashed_amount: BalanceOf<T> 
        },
        ValidatorActivated { validator: T::AccountId },
        ValidatorDeactivated { validator: T::AccountId },
        StakeUpdated { 
            validator: T::AccountId, 
            old_stake: BalanceOf<T>, 
            new_stake: BalanceOf<T> 
        },
        ValidatorStakeReserved {
            validator: T::AccountId,
            amount: BalanceOf<T>,
        },
        ValidatorStakeUnreserved {
            validator: T::AccountId,
            amount: BalanceOf<T>,
        },
        ValidatorStakeIncreased {
            validator: T::AccountId,
            amount: BalanceOf<T>,
        },
        ValidatorStakeDecreased {
            validator: T::AccountId,
            amount: BalanceOf<T>,
        },
        EpochRewardsDistributed {
            epoch: u32,
            total_distributed: BalanceOf<T>,
        },
        ValidatorJoined {
            validator: T::AccountId,
            stake_amount: BalanceOf<T>,
        },
        ValidatorLeft {
            validator: T::AccountId,
        },
        ValidatorLeaveRequested {
            validator: T::AccountId,
            cooldown_expires_at: u32,
        },
        ValidatorLeaveCancelled {
            validator: T::AccountId,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        ValidatorAlreadyRegistered,
        ValidatorNotRegistered,
        InvalidScore,
        TooManyValidators,
        ScoreTooLow,
        MaxSlashingCountReached,
        InsufficientStake,
        InvalidStakeAmount,
        StakeOperationFailed,
        SlashingBoundsExceeded,
        RewardBoundsExceeded,
        ArithmeticOverflow,
        ArithmeticUnderflow,
        LeaveCooldownActive,
        CooldownActive,
        ValidatorNotInSet,
        ValidatorAlreadyExists,
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_initialize(n: BlockNumberFor<T>) -> Weight {
            let block_number = n.saturated_into::<u32>();
            Self::process_expired_leave_requests(block_number);
            Self::cleanup_recently_removed_validators(block_number);
            Weight::zero()
        }
    }

    #[pallet::storage]
    #[pallet::getter(fn validator_set)]
    pub type ValidatorSet<T: Config> = StorageValue<
        _,
        BoundedVec<T::AccountId, <T as Config>::MaxValidators>,
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn active_validators_pos)]
    pub type ActiveValidators<T: Config> = StorageValue<
        _,
        BoundedVec<T::AccountId, <T as Config>::MaxValidators>,
        ValueQuery,
    >;

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::register_validator())]
        pub fn register_validator(origin: OriginFor<T>) -> DispatchResult {
            let who = ensure_signed(origin)?;
            
            // Check if we've reached max validators
            ensure!(
                Validators::<T>::iter().count() < T::MaxValidators::get() as usize,
                Error::<T>::TooManyValidators
            );

            ensure!(!Validators::<T>::contains_key(&who), Error::<T>::ValidatorAlreadyRegistered);
            Validators::<T>::insert(&who, true);
            Self::deposit_event(Event::ValidatorRegistered { validator: who.clone() });
            // --- Telemetry: Validator registered ---
            ::log::info!("[cerulea::pos][prometheus] validator_registered{{validator={:?}}} 1", who);
            // --- Telemetry: Active validator count ---
            let active_count = Validators::<T>::iter().filter(|(_, active)| *active).count();
            ::log::info!("[cerulea::pos][prometheus] active_validators_count{{}} {}", active_count);
            Ok(())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::submit_score())]
        pub fn submit_score(origin: OriginFor<T>, validator: T::AccountId, score: u32) -> DispatchResult {
            let _who = ensure_signed(origin)?;
            ensure!(Validators::<T>::contains_key(&validator), Error::<T>::ValidatorNotRegistered);
            ensure!(score >= T::MinValidatorScore::get(), Error::<T>::ScoreTooLow);
            ValidatorScores::<T>::insert(&validator, score);
            Self::deposit_event(Event::ScoreSubmitted { validator: validator.clone(), score });
            // --- Telemetry: Score submitted ---
            ::log::info!("[cerulea::pos][prometheus] score_submitted{{validator={:?}}} {}", validator, score);
            Ok(())
        }

        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::slash_validator())]
        pub fn slash_validator_call(origin: OriginFor<T>, validator: T::AccountId) -> DispatchResult {
            let _who = ensure_signed(origin)?;
            ensure!(Validators::<T>::contains_key(&validator), Error::<T>::ValidatorNotRegistered);
            
            let count = SlashingCount::<T>::get(&validator).unwrap_or(0) + 1;
            
            if count >= T::MaxSlashingCount::get() {
                // Remove validator if max slashing count reached
                Validators::<T>::remove(&validator);
                ValidatorScores::<T>::remove(&validator);
                SlashingCount::<T>::remove(&validator);
                Self::deposit_event(Event::ValidatorRemoved { 
                    validator: validator.clone(),
                    reason: b"Max slashing count reached".to_vec(),
                });
            } else {
                SlashingCount::<T>::insert(&validator, count);
            }
            
            Self::deposit_event(Event::ValidatorSlashed { validator: validator.clone(), slashing_count: count });
            // --- Telemetry: Validator slashed ---
            ::log::info!("[cerulea::pos][prometheus] validator_slashed{{validator={:?}}} {}", validator, count);
            Ok(())
        }

        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::bond_stake())]
        pub fn bond_stake(origin: OriginFor<T>, amount: BalanceOf<T>) -> DispatchResult {
            let who = ensure_signed(origin)?;
            
            ensure!(amount >= T::MinStake::get(), Error::<T>::InsufficientStake);
            ensure!(Validators::<T>::contains_key(&who), Error::<T>::ValidatorNotRegistered);
            
            let current_stake = Stake::<T>::get(&who);
            let new_stake = current_stake.checked_add(&amount).ok_or(Error::<T>::InvalidStakeAmount)?;
            
            Stake::<T>::insert(&who, &new_stake);
            Self::deposit_event(Event::StakeBonded { validator: who.clone(), amount });
            Self::deposit_event(Event::StakeUpdated { 
                validator: who, 
                old_stake: current_stake, 
                new_stake 
            });
            
            Ok(())
        }

        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::unbond_stake())]
        pub fn unbond_stake(origin: OriginFor<T>, amount: BalanceOf<T>) -> DispatchResult {
            let who = ensure_signed(origin)?;
            
            ensure!(Validators::<T>::contains_key(&who), Error::<T>::ValidatorNotRegistered);
            
            let current_stake = Stake::<T>::get(&who);
            ensure!(current_stake >= amount, Error::<T>::InsufficientStake);
            
            let new_stake = current_stake.checked_sub(&amount).ok_or(Error::<T>::InvalidStakeAmount)?;
            ensure!(new_stake >= T::MinStake::get(), Error::<T>::InsufficientStake);
            
            Stake::<T>::insert(&who, &new_stake);
            Self::deposit_event(Event::StakeUnbonded { validator: who.clone(), amount });
            Self::deposit_event(Event::StakeUpdated { 
                validator: who, 
                old_stake: current_stake, 
                new_stake 
            });
            
            Ok(())
        }

        #[pallet::call_index(5)]
        #[pallet::weight(T::WeightInfo::submit_score())] 
        pub fn boost_score(origin: OriginFor<T>, validator: T::AccountId, weight: u32) -> DispatchResult {
            let _who = ensure_signed(origin)?;
            ensure!(Validators::<T>::contains_key(&validator), Error::<T>::ValidatorNotRegistered);

            let current_score = ValidatorScores::<T>::get(&validator).unwrap_or(0);
            let new_score = current_score.saturating_add(weight);
            ValidatorScores::<T>::insert(&validator, new_score);

            Self::deposit_event(Event::ScoreSubmitted { validator: validator.clone(), score: new_score });
            Self::deposit_event(Event::ScoreBoosted { 
                validator, 
                old_score: current_score, 
                new_score, 
                boost_amount: weight 
            });
            Ok(())
        }

        #[pallet::call_index(6)]
        #[pallet::weight(T::WeightInfo::submit_score())] 
        pub fn slash_score(origin: OriginFor<T>, validator: T::AccountId, weight: u32) -> DispatchResult {
            let _who = ensure_signed(origin)?;
            ensure!(Validators::<T>::contains_key(&validator), Error::<T>::ValidatorNotRegistered);

            let current_score = ValidatorScores::<T>::get(&validator).unwrap_or(0);
            let new_score = current_score.saturating_sub(weight);
            ValidatorScores::<T>::insert(&validator, new_score);

            Self::deposit_event(Event::ScoreSubmitted { validator: validator.clone(), score: new_score });
            Self::deposit_event(Event::ScoreSlashed { 
                validator, 
                old_score: current_score, 
                new_score, 
                slash_amount: weight 
            });
            Ok(())
        }

        #[pallet::call_index(8)]
        #[pallet::weight(T::WeightInfo::register_validator())]
        pub fn join_validators(
            origin: OriginFor<T>,
            _name: Option<BoundedVec<u8, ConstU32<32>>>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let min_stake = T::MinStake::get();
            let free_balance = T::Currency::free_balance(&who);
            ensure!(
                free_balance >= min_stake,
                Error::<T>::InsufficientStake
            );

            Self::validate_rejoin_eligibility(&who)?;

            let mut validator_set = ValidatorSet::<T>::get();
            ensure!(
                !validator_set.contains(&who),
                Error::<T>::ValidatorAlreadyExists
            );

            T::Currency::reserve(&who, min_stake)
                .map_err(|_| Error::<T>::InsufficientStake)?;

            Stake::<T>::insert(&who, min_stake);
            ValidatorJoinTime::<T>::insert(&who, frame_system::Pallet::<T>::block_number().saturated_into::<u32>());

            ensure!(
                validator_set.len() < T::MaxValidators::get() as usize,
                Error::<T>::TooManyValidators
            );

            validator_set.try_push(who.clone())
                .map_err(|_| Error::<T>::TooManyValidators)?;
            ValidatorSet::<T>::put(validator_set);

            Validators::<T>::insert(&who, true);

            T::ValidatorHandler::on_joined(&who, min_stake)?;

            Self::deposit_event(Event::ValidatorJoined {
                validator: who.clone(),
                stake_amount: min_stake,
            });

            Self::deposit_event(Event::ValidatorStakeReserved {
                validator: who,
                amount: min_stake,
            });

            Ok(())
        }

        #[pallet::call_index(9)]
        #[pallet::weight(T::WeightInfo::register_validator())]
        pub fn leave_validators(origin: OriginFor<T>) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let validator_set = ValidatorSet::<T>::get();
            ensure!(
                validator_set.contains(&who),
                Error::<T>::ValidatorNotInSet
            );

            Self::validate_leave_request_eligibility(&who)?;

            let current_block = frame_system::Pallet::<T>::block_number().saturated_into::<u32>();

            ValidatorLeaveRequests::<T>::insert(&who, current_block);

            let mut active_validators = ActiveValidators::<T>::get();
            if let Some(pos) = active_validators.iter().position(|v| v == &who) {
                active_validators.remove(pos);
                ActiveValidators::<T>::put(active_validators);
            }

            Validators::<T>::insert(&who, false);

            T::ValidatorHandler::on_leave_requested(&who)?;

            Self::deposit_event(Event::ValidatorLeaveRequested {
                validator: who.clone(),
                cooldown_expires_at: current_block + T::LeaveCooldown::get(),
            });

            Ok(())
        }

        #[pallet::call_index(10)]
        #[pallet::weight(T::WeightInfo::register_validator())]
        pub fn cancel_leave_request(origin: OriginFor<T>) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let leave_request_block = ValidatorLeaveRequests::<T>::get(&who)
                .ok_or(Error::<T>::ValidatorNotInSet)?;

            let current_block = frame_system::Pallet::<T>::block_number().saturated_into::<u32>();
            let cooldown_period = T::LeaveCooldown::get();
            let blocks_since_request = current_block.saturating_sub(leave_request_block);

            if blocks_since_request >= cooldown_period {
                return Err(Error::<T>::LeaveCooldownActive.into());
            }

            ValidatorLeaveRequests::<T>::remove(&who);

            let validator_set = ValidatorSet::<T>::get();
            if validator_set.contains(&who) {
                let mut active_validators = ActiveValidators::<T>::get();
                if !active_validators.contains(&who) {
                    if active_validators.len() < T::MaxValidators::get() as usize {
                        let _ = active_validators.try_push(who.clone());
                        ActiveValidators::<T>::put(active_validators);
                    }
                }
                Validators::<T>::insert(&who, true);
            }

            Self::deposit_event(Event::ValidatorLeaveCancelled {
                validator: who.clone(),
            });

            Ok(())
        }

        #[pallet::call_index(11)]
        #[pallet::weight(T::WeightInfo::register_validator())]
        pub fn increase_validator_stake(
            origin: OriginFor<T>,
            additional_amount: BalanceOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let validator_set = ValidatorSet::<T>::get();
            ensure!(
                validator_set.contains(&who),
                Error::<T>::ValidatorNotInSet
            );

            let free_balance = T::Currency::free_balance(&who);
            ensure!(
                free_balance >= additional_amount,
                Error::<T>::InsufficientStake
            );

            T::Currency::reserve(&who, additional_amount)
                .map_err(|_| Error::<T>::InsufficientStake)?;

            Stake::<T>::mutate(&who, |current_stake| {
                *current_stake = current_stake.saturating_add(additional_amount);
            });

            T::ValidatorHandler::on_stake_increased(&who, additional_amount)?;

            Self::deposit_event(Event::ValidatorStakeReserved {
                validator: who.clone(),
                amount: additional_amount,
            });

            Self::deposit_event(Event::ValidatorStakeIncreased {
                validator: who,
                amount: additional_amount,
            });

            Ok(())
        }

        #[pallet::call_index(12)]
        #[pallet::weight(T::WeightInfo::register_validator())]
        pub fn decrease_validator_stake(
            origin: OriginFor<T>,
            decrease_amount: BalanceOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let validator_set = ValidatorSet::<T>::get();
            ensure!(
                validator_set.contains(&who),
                Error::<T>::ValidatorNotInSet
            );

            let current_stake = Stake::<T>::get(&who);
            let min_stake = T::MinStake::get();

            ensure!(
                current_stake.saturating_sub(decrease_amount) >= min_stake,
                Error::<T>::InsufficientStake
            );

            let unreserved = T::Currency::unreserve(&who, decrease_amount);

            Stake::<T>::mutate(&who, |current_stake| {
                *current_stake = current_stake.saturating_sub(unreserved);
            });

            T::ValidatorHandler::on_stake_decreased(&who, unreserved)?;

            Self::deposit_event(Event::ValidatorStakeUnreserved {
                validator: who.clone(),
                amount: unreserved,
            });

            Self::deposit_event(Event::ValidatorStakeDecreased {
                validator: who,
                amount: unreserved,
            });

            Ok(())
        }

        #[pallet::call_index(13)]
        #[pallet::weight(T::WeightInfo::register_validator())]
        pub fn slash_validator(
            origin: OriginFor<T>,
            validator: T::AccountId,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            ensure_root(origin)?;
            Self::execute_slash_validator(&validator, amount)
        }

        #[pallet::call_index(14)]
        #[pallet::weight(T::WeightInfo::register_validator())]
        pub fn slash_validator_percentage(
            origin: OriginFor<T>,
            validator: T::AccountId,
            percentage: u32,
        ) -> DispatchResult {
            ensure_root(origin)?;
            ensure!(percentage <= 100, Error::<T>::InvalidStakeAmount);
            
            let current_stake = Stake::<T>::get(&validator);
            let slash_amount = current_stake.saturating_mul(percentage.saturated_into()) / 100u32.saturated_into();
            Self::execute_slash_validator(&validator, slash_amount)
        }

        #[pallet::call_index(15)]
        #[pallet::weight(T::WeightInfo::register_validator())]
        pub fn slash_multiple_validators(
            origin: OriginFor<T>,
            validators: Vec<T::AccountId>,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            ensure_root(origin)?;
            for validator in validators {
                let _ = Self::execute_slash_validator(&validator, amount);
            }
            Ok(())
        }

        #[pallet::call_index(16)]
        #[pallet::weight(T::WeightInfo::register_validator())]
        pub fn reward_validator_call(
            origin: OriginFor<T>,
            validator: T::AccountId,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            ensure_root(origin)?;
            Self::execute_reward_validator(&validator, amount)
        }

        #[pallet::call_index(17)]
        #[pallet::weight(T::WeightInfo::register_validator())]
        pub fn reward_multiple_validators(
            origin: OriginFor<T>,
            validators: Vec<T::AccountId>,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            ensure_root(origin)?;
            for validator in validators {
                let _ = Self::execute_reward_validator(&validator, amount);
            }
            Ok(())
        }

        #[pallet::call_index(18)]
        #[pallet::weight(T::WeightInfo::register_validator())]
        pub fn reward_all_active_validators(
            origin: OriginFor<T>,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            ensure_root(origin)?;
            let active_validators = T::ValidatorHandler::get_active_validators();
            for validator in active_validators {
                let _ = Self::execute_reward_validator(&validator, amount);
            }
            Ok(())
        }

        #[pallet::call_index(19)]
        #[pallet::weight(T::WeightInfo::register_validator())]
        pub fn distribute_epoch_rewards(
            origin: OriginFor<T>,
            total_reward_pool: BalanceOf<T>,
        ) -> DispatchResult {
            ensure_root(origin)?;

            let base_pool = (total_reward_pool * 60u32.into()) / 100u32.into();
            let performance_pool = (total_reward_pool * 25u32.into()) / 100u32.into();
            let top_performer_pool = (total_reward_pool * 15u32.into()) / 100u32.into();

            Self::distribute_rewards(base_pool, performance_pool, top_performer_pool)
        }
    }

    impl<T: Config> Pallet<T> {
        pub fn validator_stake_score(validator: &T::AccountId) -> u128 {
            Self::stake(validator).saturated_into()
        }

        pub fn calculate_slash_amount(validator: &T::AccountId) -> Result<BalanceOf<T>, DispatchError> {
            let current_stake = Stake::<T>::get(validator);
            let slash_percent = T::SlashPercent::get();
            let slash_amount = current_stake.saturating_mul(slash_percent.saturated_into()) / 100u32.saturated_into();
            Ok(slash_amount)
        }

        pub fn execute_reward_validator(validator: &T::AccountId, amount: BalanceOf<T>) -> DispatchResult {
            Self::execute_reward_validator_with_reason(validator, amount, RewardReason::ManualReward)
        }

        pub fn execute_reward_validator_with_reason(
            validator: &T::AccountId,
            amount: BalanceOf<T>,
            _reason: RewardReason,
        ) -> DispatchResult {
            let validator_set = ValidatorSet::<T>::get();
            ensure!(
                validator_set.contains(validator),
                Error::<T>::ValidatorNotInSet
            );

            let pre_balance = T::Currency::free_balance(validator);

            let reward_amount = if amount > BalanceOf::<T>::default() {
                amount
            } else {
                T::ValidatorReward::get()
            };

            let current_epoch_total = EpochTotalRewarded::<T>::get();
            let current_validator_total = ValidatorEpochRewarded::<T>::get(validator);

            let new_epoch_total = current_epoch_total.checked_add(&reward_amount)
                .ok_or(Error::<T>::ArithmeticOverflow)?;
            ensure!(
                new_epoch_total <= T::MaxRewardPerEpoch::get(),
                Error::<T>::RewardBoundsExceeded
            );

            let new_validator_total = current_validator_total.checked_add(&reward_amount)
                .ok_or(Error::<T>::ArithmeticOverflow)?;
            ensure!(
                new_validator_total <= T::MaxRewardPerValidator::get(),
                Error::<T>::RewardBoundsExceeded
            );

            let _new_balance = pre_balance.checked_add(&reward_amount)
                .ok_or(Error::<T>::ArithmeticOverflow)?;

            let _ = T::Currency::deposit_creating(validator, reward_amount);

            EpochTotalRewarded::<T>::mutate(|total| {
                *total = total.saturating_add(reward_amount);
            });
            ValidatorEpochRewarded::<T>::mutate(validator, |validator_total| {
                *validator_total = validator_total.saturating_add(reward_amount);
            });

            let score_boost = {
                let boost_raw = reward_amount.saturated_into::<u64>()
                    .checked_div(T::RewardBoostDivisor::get())
                    .unwrap_or(0);
                boost_raw.min(T::MaxRewardBoost::get())
            };

            T::ValidatorHandler::on_rewarded(validator, reward_amount, score_boost)?;

            Ok(())
        }

        pub fn execute_slash_validator(validator: &T::AccountId, amount: BalanceOf<T>) -> DispatchResult {
            Self::execute_slash_validator_with_reason(validator, amount, SlashReason::ManualSlash)
        }

        pub fn execute_slash_validator_with_reason(
            validator: &T::AccountId,
            amount: BalanceOf<T>,
            reason: SlashReason,
        ) -> DispatchResult {
            let validator_set = ValidatorSet::<T>::get();
            ensure!(
                validator_set.contains(validator),
                Error::<T>::ValidatorNotInSet
            );

            let pre_balance = T::Currency::free_balance(validator);

            let slash_amount = if amount == BalanceOf::<T>::default() {
                let slash_percent = T::SlashPercent::get();
                let slash_numerator = pre_balance.checked_mul(&slash_percent.saturated_into())
                    .ok_or(Error::<T>::ArithmeticOverflow)?;
                let slash_amount = slash_numerator.checked_div(&100u32.saturated_into())
                    .ok_or(Error::<T>::ArithmeticUnderflow)?;
                slash_amount
            } else {
                amount
            };

            let current_epoch_total = EpochTotalSlashed::<T>::get();
            let current_validator_total = ValidatorEpochSlashed::<T>::get(validator);

            let new_epoch_total = current_epoch_total.checked_add(&slash_amount)
                .ok_or(Error::<T>::ArithmeticOverflow)?;
            ensure!(
                new_epoch_total <= T::MaxSlashPerEpoch::get(),
                Error::<T>::SlashingBoundsExceeded
            );

            let new_validator_total = current_validator_total.checked_add(&slash_amount)
                .ok_or(Error::<T>::ArithmeticOverflow)?;
            ensure!(
                new_validator_total <= T::MaxSlashPerValidator::get(),
                Error::<T>::SlashingBoundsExceeded
            );

            let (_negative_imbalance, actual_slashed) = T::Currency::slash(validator, slash_amount);
            let slashed_amount = actual_slashed.min(slash_amount);

            EpochTotalSlashed::<T>::mutate(|total| {
                *total = total.saturating_add(slashed_amount);
            });
            ValidatorEpochSlashed::<T>::mutate(validator, |validator_total| {
                *validator_total = validator_total.saturating_add(slashed_amount);
            });

            let score_penalty = {
                let penalty_raw = slashed_amount.saturated_into::<u64>()
                    .checked_div(T::SlashPenaltyDivisor::get())
                    .unwrap_or(0);
                penalty_raw.min(T::MaxSlashPenalty::get())
            };

            T::ValidatorHandler::on_slashed(validator, slashed_amount, score_penalty)?;

            Ok(())
        }

        pub fn distribute_rewards(
            base_reward_pool: BalanceOf<T>,
            performance_reward_pool: BalanceOf<T>,
            top_performer_reward_pool: BalanceOf<T>,
        ) -> DispatchResult {
            let active_validators = T::ValidatorHandler::get_active_validators();
            if active_validators.is_empty() {
                return Ok(());
            }

            let mut validator_scores: Vec<(T::AccountId, u64)> = Vec::new();
            for validator in &active_validators {
                let score = T::ValidatorHandler::get_validator_score(validator);
                validator_scores.push((validator.clone(), score));
            }

            validator_scores.sort_by(|a, b| b.1.cmp(&a.1));

            let total_validators = validator_scores.len();
            let high_performance_threshold = T::HighPerformanceScore::get();
            let top_performer_count = (total_validators * T::TopPerformerPercentage::get() as usize) / 100;

            let base_reward_per_validator = if total_validators > 0 {
                base_reward_pool / (total_validators as u32).into()
            } else {
                BalanceOf::<T>::default()
            };

            let high_performers: Vec<_> = validator_scores.iter()
                .filter(|(_, score)| *score >= high_performance_threshold)
                .collect();
            
            let top_performers = &validator_scores[..top_performer_count.min(total_validators)];

            let performance_reward_per_validator = if !high_performers.is_empty() {
                performance_reward_pool / (high_performers.len() as u32).into()
            } else {
                BalanceOf::<T>::default()
            };

            let top_performer_reward_per_validator = if !top_performers.is_empty() {
                top_performer_reward_pool / (top_performers.len() as u32).into()
            } else {
                BalanceOf::<T>::default()
            };

            for (validator, score) in &validator_scores {
                let mut total_reward = base_reward_per_validator;

                if *score >= high_performance_threshold {
                    total_reward = total_reward.saturating_add(performance_reward_per_validator);
                }

                if top_performers.iter().any(|(v, _)| v == validator) {
                    total_reward = total_reward.saturating_add(top_performer_reward_per_validator);
                }

                Self::execute_reward_validator(validator, total_reward)?;
            }

            Self::deposit_event(Event::EpochRewardsDistributed {
                epoch: CurrentEpoch::<T>::get(),
                total_distributed: base_reward_pool + performance_reward_pool + top_performer_reward_pool,
            });

            Ok(())
        }

        pub fn calculate_epoch_economic_impact(_epoch: u32) -> (BalanceOf<T>, BalanceOf<T>) {
            (EpochTotalRewarded::<T>::get(), EpochTotalSlashed::<T>::get())
        }

        pub fn reset_epoch_totals() {
            EpochTotalSlashed::<T>::kill();
            EpochTotalRewarded::<T>::kill();
            let _ = ValidatorEpochSlashed::<T>::clear(u32::MAX, None);
            let _ = ValidatorEpochRewarded::<T>::clear(u32::MAX, None);
        }

        fn cleanup_recently_removed_validators(current_block: u32) {
            let cooldown_period = T::LeaveCooldown::get();
            let mut expired_entries = Vec::new();

            for (validator, removed_at_block) in RecentlyRemovedValidators::<T>::iter() {
                let blocks_passed = current_block.saturating_sub(removed_at_block);
                if blocks_passed >= cooldown_period {
                    expired_entries.push(validator);
                }
            }

            for validator in expired_entries {
                RecentlyRemovedValidators::<T>::remove(&validator);
            }
        }

        fn process_expired_leave_requests(current_block: u32) {
            let cooldown_period = T::LeaveCooldown::get();
            let mut expired_requests = Vec::new();

            for (validator, request_block) in ValidatorLeaveRequests::<T>::iter() {
                let blocks_passed = current_block.saturating_sub(request_block);
                if blocks_passed >= cooldown_period {
                    expired_requests.push(validator);
                }
            }

            for validator in expired_requests {
                let stake_amount = Stake::<T>::get(&validator);
                if stake_amount > BalanceOf::<T>::default() {
                    let _ = T::Currency::unreserve(&validator, stake_amount);
                    Self::deposit_event(Event::ValidatorStakeUnreserved {
                        validator: validator.clone(),
                        amount: stake_amount,
                    });
                }

                let mut validator_set = ValidatorSet::<T>::get();
                if let Some(pos) = validator_set.iter().position(|v| v == &validator) {
                    validator_set.remove(pos);
                    ValidatorSet::<T>::put(validator_set);
                }

                let mut active_validators = ActiveValidators::<T>::get();
                if let Some(pos) = active_validators.iter().position(|v| v == &validator) {
                    active_validators.remove(pos);
                    ActiveValidators::<T>::put(active_validators);
                }

                Validators::<T>::remove(&validator);
                ValidatorJoinTime::<T>::remove(&validator);
                Stake::<T>::remove(&validator);
                ValidatorLeaveRequests::<T>::remove(&validator);

                RecentlyRemovedValidators::<T>::insert(&validator, current_block);

                let _ = T::ValidatorHandler::on_left(&validator);

                Self::deposit_event(Event::ValidatorLeft { validator });
            }
        }

        fn validate_rejoin_eligibility(validator: &T::AccountId) -> DispatchResult {
            if let Some(removed_block) = RecentlyRemovedValidators::<T>::get(validator) {
                let current_block = frame_system::Pallet::<T>::block_number().saturated_into::<u32>();
                let cooldown_period = T::LeaveCooldown::get();
                let blocks_since_removal = current_block.saturating_sub(removed_block);
                
                ensure!(
                    blocks_since_removal >= cooldown_period,
                    Error::<T>::CooldownActive
                );
            }
            Ok(())
        }

        fn validate_leave_request_eligibility(validator: &T::AccountId) -> DispatchResult {
            ensure!(
                !ValidatorLeaveRequests::<T>::contains_key(validator),
                Error::<T>::LeaveCooldownActive
            );
            Ok(())
        }

        /// Slash a validator's stake by a specific amount
        pub fn slash_validator_stake(validator: &T::AccountId, amount: BalanceOf<T>) -> DispatchResult {
            ensure!(Validators::<T>::contains_key(validator), Error::<T>::ValidatorNotRegistered);
            
            let current_stake = Stake::<T>::get(validator);
            let new_stake = current_stake.saturating_sub(amount);
            
            // Update stake
            Stake::<T>::insert(validator, &new_stake);
            
            // Emit stake slashing event
            Self::deposit_event(Event::ValidatorStakeSlashed { 
                validator: validator.clone(), 
                old_stake: current_stake, 
                new_stake, 
                slashed_amount: amount 
            });
            
            // Increment slashing count
            let count = SlashingCount::<T>::get(validator).unwrap_or(0) + 1;
            SlashingCount::<T>::insert(validator, count);
            
            Self::deposit_event(Event::ValidatorSlashed { 
                validator: validator.clone(), 
                slashing_count: count 
            });
            
            // Remove validator if max slashing count reached
            if count >= T::MaxSlashingCount::get() {
                Validators::<T>::remove(validator);
                ValidatorScores::<T>::remove(validator);
                SlashingCount::<T>::remove(validator);
                Self::deposit_event(Event::ValidatorRemoved { 
                    validator: validator.clone(),
                    reason: b"Max slashing count reached".to_vec(),
                });
            }
            
            Ok(())
        }

        /// Reward a validator by increasing their stake
        pub fn reward_validator(validator: &T::AccountId, amount: BalanceOf<T>) -> DispatchResult {
            ensure!(Validators::<T>::contains_key(validator), Error::<T>::ValidatorNotRegistered);
            
            let current_stake = Stake::<T>::get(validator);
            let new_stake = current_stake.saturating_add(amount);
            
            Stake::<T>::insert(validator, &new_stake);
            
            Self::deposit_event(Event::StakeBonded { 
                validator: validator.clone(), 
                amount 
            });
            
            Self::deposit_event(Event::StakeUpdated { 
                validator: validator.clone(), 
                old_stake: current_stake, 
                new_stake 
            });
            
            Ok(())
        }

        /// Get all active validators
        pub fn get_active_validators() -> Vec<T::AccountId> {
            let active_list: Vec<T::AccountId> = Validators::<T>::iter()
                .filter_map(|(validator, is_active)| {
                    if is_active {
                        Some(validator)
                    } else {
                        None
                    }
                })
                .collect();
            active_list
        }

        /// Activate a validator
        pub fn activate_validator(validator: &T::AccountId) -> DispatchResult {
            ensure!(Validators::<T>::contains_key(validator), Error::<T>::ValidatorNotRegistered);
            
            let is_active = Validators::<T>::get(validator).unwrap_or(false);
            if !is_active {
                Validators::<T>::insert(validator, true);
                Self::deposit_event(Event::ValidatorActivated { 
                    validator: validator.clone() 
                });
            }
            
            Ok(())
        }

        /// Deactivate a validator
        pub fn deactivate_validator(validator: &T::AccountId) -> DispatchResult {
            ensure!(Validators::<T>::contains_key(validator), Error::<T>::ValidatorNotRegistered);
            
            let is_active = Validators::<T>::get(validator).unwrap_or(false);
            if is_active {
                Validators::<T>::insert(validator, false);
                Self::deposit_event(Event::ValidatorDeactivated { 
                    validator: validator.clone() 
                });
            }
            
            Ok(())
        }

        /// Force eject a validator immediately
        pub fn force_eject_validator(validator: &T::AccountId) -> DispatchResult {
            let stake_amount = Stake::<T>::get(validator);
            if stake_amount > BalanceOf::<T>::default() {
                let _ = T::Currency::unreserve(validator, stake_amount);
                Self::deposit_event(Event::ValidatorStakeUnreserved {
                    validator: validator.clone(),
                    amount: stake_amount,
                });
            }

            let mut validator_set = ValidatorSet::<T>::get();
            if let Some(pos) = validator_set.iter().position(|v| v == validator) {
                validator_set.remove(pos);
                ValidatorSet::<T>::put(validator_set);
            }

            let mut active_validators = ActiveValidators::<T>::get();
            if let Some(pos) = active_validators.iter().position(|v| v == validator) {
                active_validators.remove(pos);
                ActiveValidators::<T>::put(active_validators);
            }

            Validators::<T>::remove(validator);
            ValidatorJoinTime::<T>::remove(validator);
            Stake::<T>::remove(validator);
            ValidatorLeaveRequests::<T>::remove(validator);

            let current_block = frame_system::Pallet::<T>::block_number().saturated_into::<u32>();
            RecentlyRemovedValidators::<T>::insert(validator, current_block);

            // Notify handler (DCF) of removal
            let _ = T::ValidatorHandler::on_left(validator);

            Self::deposit_event(Event::ValidatorLeft { validator: validator.clone() });

            Ok(())
        }
    }
}
