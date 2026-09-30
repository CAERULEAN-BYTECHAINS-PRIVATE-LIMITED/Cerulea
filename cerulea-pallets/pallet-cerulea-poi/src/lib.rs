#![cfg_attr(not(feature = "std"), no_std)]
//! # The optional inference contribution
//!
//! Deployments running Cerulea Intelligence on their own validators may enable
//! an additional scoring contribution based on served inference work. Custody
//! proof remains the default for every other deployment.
//!
//! The module stays off by default. The `inference` cargo feature turns it on.
//! The gate in [`gate`] holds three conditions. The build carries the module.
//! Determinism is proven. The task came from real workload.
//!
//! 1. **Submission.** A validator submits an inference result with a
//!    confidence score.
//! 2. **Optimistic acceptance.** The chain accepts the result provisionally.
//! 3. **Challenge.** Any validator challenges inside the window. A challenge
//!    triggers redundant execution by a randomly drawn committee, and the
//!    losing side carries the penalty.
//! 4. **Epochs.** Results and challenges are tracked per epoch.
//! 5. **Events.** The pallet reports every submission, challenge and outcome.

pub use pallet::*;

/// The ceiling one inference contribution reaches.
///
/// The contribution reports a share of one, counted in parts per billion, the
/// way every Standing Engine component reports. A validator serving the whole
/// of what the deployment asked reaches this number. A raw result above the
/// ceiling holds at the ceiling and records a fault against the validator.
pub const CONTRIBUTION_CEILING: u64 = 1_000_000_000;

// The gate on the optional inference contribution
pub mod gate;

// --- Weights Module --- //
pub mod weights;
pub use weights::*;

// --- Test Modules --- //
#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

// --- Benchmarking --- //
#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

// --- Imports --- //
use codec::Encode;
use frame_support::traits::Get;
use frame_system::pallet_prelude::BlockNumberFor;
use scale_info::prelude::format;
use sp_runtime::{offchain::storage::StorageValueRef, traits::SaturatedConversion, DispatchResult};
use sp_std::prelude::*;

// --- Runtime API Declaration --- //
// Exposes PoI state and queries to the runtime API.
sp_api::decl_runtime_apis! {
    pub trait PoiApi<AccountId>
    where
        AccountId: codec::Codec + Clone + Eq + sp_std::fmt::Debug,
    {
        /// Get the inference result and epoch for a validator.
        fn get_inference_result(validator: AccountId) -> Option<(u32, u32)>;
        /// Get the challenge (challenger, result, epoch) for a validator.
        fn get_challenge(validator: AccountId) -> Option<(AccountId, u32, u32)>;
        /// Get the current inference epoch.
        fn get_current_epoch() -> u32;
    }
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::DispatchResult;
    use frame_support::{pallet_prelude::*, storage::types::StorageMap};
    use frame_system::pallet_prelude::*;
    use scale_info::prelude::vec::Vec;

    /// Main pallet struct.
    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// Pallet settings trait.
    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// The overarching event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// Weight information for extrinsics.
        type WeightInfo: WeightInfo;

        /// Minimum confidence threshold for inference (0-100).
        type MinInferenceConfidence: Get<u32>;
        /// Maximum age of inference in epochs.
        type MaxInferenceAge: Get<u32>;
        /// Number of epochs to challenge an inference.
        type ChallengeWindow: Get<u32>;
        /// Reward for correct inference.
        type InferenceReward: Get<u128>;
        /// Reward for successful challenge.
        type ChallengeReward: Get<u128>;
        /// Interface to PoS pallet for boosting/slashing scores
        type PosInterface: PosInterface<Self::AccountId>;
        /// Interface to DCF pallet for inference tracking
        type DcfInterface: DcfInterface<Self::AccountId>;

        // PoI scoring parameters
        #[pallet::constant]
        type InferenceBoostLow: Get<u64>;
        #[pallet::constant]
        type InferenceBoostMedium: Get<u64>;
        #[pallet::constant]
        type InferenceBoostHigh: Get<u64>;
        #[pallet::constant]
        type InferencePenaltyLow: Get<u64>;
        #[pallet::constant]
        type InferencePenaltyMedium: Get<u64>;
        #[pallet::constant]
        type InferencePenaltyHigh: Get<u64>;
        #[pallet::constant]
        type InferenceConfidenceThresholdLow: Get<u32>;
        #[pallet::constant]
        type InferenceConfidenceThresholdHigh: Get<u32>;

        #[pallet::constant]
        type MaxValidatorScore: Get<u64>;
        #[pallet::constant]
        type PercentagePrecision: Get<u32>;
        #[pallet::constant]
        type OffchainWorkerInterval: Get<u32>;

        // Rate limiting settingss for apply_offchain_poi_scores
        #[pallet::constant]
        type MaxValidatorIterationWeight: Get<u64>;
        #[pallet::constant]
        type MaxLoopIterations: Get<u32>;
    }

    /// Trait for PoS score manipulation (to be implemented by PoS pallet or runtime)
    pub trait PosInterface<AccountId> {
        fn boost_score(validator: &AccountId, weight: u32) -> DispatchResult;
        fn slash_score(validator: &AccountId, weight: u32) -> DispatchResult;
        fn get_active_validators() -> Vec<AccountId>;
    }

    /// Trait for DCF inference tracking (to be implemented by DCF pallet or runtime)
    pub trait DcfInterface<AccountId> {
        fn record_inference_activity(validator: &AccountId) -> DispatchResult;
        fn eject_validator(validator: &AccountId, reason: &'static str) -> DispatchResult;
        fn update_final_score(validator: &AccountId) -> DispatchResult;
    }

    /// Data structure for inference data collected by off-chain workers.
    #[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo)]
    pub struct InferenceData<T: Config> {
        pub validator: T::AccountId,
        pub epoch: u32,
        pub inference_result: u32,
        pub confidence_score: u32,
        pub timestamp: u64,
        pub data_sources: Vec<Vec<u8>>,
    }

    /// Off-chain storage structure for computed inference scores.
    #[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo)]
    pub struct OffchainPoiScore<T: Config> {
        pub validator: T::AccountId,
        pub score: u64,
        pub block_number: u32,
        pub timestamp: u64,
    }

    /// Severity levels for inference errors and performance issues.
    #[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    pub enum InferenceErrorSeverity {
        High,
        Medium,
        Low,
    }

    /// Genesis settings for PoI pallet.
    #[pallet::genesis_config]
    #[derive(frame_support::DefaultNoBound)]
    pub struct GenesisConfig<T: Config> {
        /// Initial inference results: (validator, result).
        pub inference_results: Vec<(T::AccountId, u32)>,
        /// Initial challenges: (challenger, challenged, result).
        pub challenges: Vec<(T::AccountId, T::AccountId, u32)>,
        /// Initial epoch number.
        pub current_epoch: u32,
    }

    /// Genesis build logic for initializing storage.
    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            // Initialize inference results for validators.
            for (account, result) in &self.inference_results {
                InferenceResults::<T>::insert(account, (*result, self.current_epoch));
            }

            // Initialize challenges.
            for (challenger, challenged, result) in &self.challenges {
                Challenges::<T>::insert(challenger, (challenged, *result, self.current_epoch));
            }

            // Set the current epoch.
            CurrentEpoch::<T>::put(self.current_epoch);
        }
    }

    /// Storage for inference result submissions from validators.
    /// Maps validator account to (result, epoch).
    #[pallet::storage]
    #[pallet::getter(fn inference_results)]
    pub type InferenceResults<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        (u32, u32), // (result, epoch)
        OptionQuery,
    >;

    /// Storage for challenge submissions.
    /// Maps challenger account to (challenged, result, epoch).
    #[pallet::storage]
    #[pallet::getter(fn challenges)]
    pub type Challenges<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        (T::AccountId, u32, u32), // (challenged, result, epoch)
        OptionQuery,
    >;

    /// Storage for the current inference epoch or round.
    #[pallet::storage]
    #[pallet::getter(fn current_epoch)]
    pub type CurrentEpoch<T: Config> = StorageValue<_, u32, ValueQuery>;

    /// The determinism evidence this deployment declared.
    ///
    /// Two validators running the same task produce identical bytes only where
    /// every condition holds. The contribution stays absent until the evidence
    /// proves each one.
    #[pallet::storage]
    #[pallet::getter(fn determinism_evidence)]
    pub type DeterminismEvidenceHeld<T: Config> =
        StorageValue<_, crate::gate::DeterminismEvidence, ValueQuery>;

    /// The results the chain accepted provisionally.
    ///
    /// A result stands unless a validator challenges inside the window.
    #[pallet::storage]
    #[pallet::getter(fn provisional_result)]
    pub type ProvisionalResults<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, crate::gate::ProvisionalResult, OptionQuery>;

    /// Storage for validator inference count.
    #[pallet::storage]
    #[pallet::getter(fn validator_inference_count)]
    pub type ValidatorInferenceCount<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, u64, ValueQuery>;

    /// The count of raw results that sat above the contribution ceiling.
    ///
    /// The contribution reports a share of one. A raw result above that
    /// ceiling holds at the ceiling, and the count here rises by one. An
    /// operator reads the count to find a validator reporting figures the
    /// share cannot carry.
    #[pallet::storage]
    #[pallet::getter(fn contribution_faults)]
    pub type ContributionFaults<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, u32, ValueQuery>;

    /// Events emitted by the PoI pallet.
    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// An inference result was submitted. [who, result, confidence]
        InferenceSubmitted {
            who: T::AccountId,
            result: u32,
            confidence: u32,
        },
        /// An inference result was accepted. [validator, confidence]
        InferenceAccepted {
            validator: T::AccountId,
            confidence: u32,
        },
        /// An inference result was rejected. [validator, confidence]
        InferenceRejected {
            validator: T::AccountId,
            confidence: u32,
        },
        /// An inference result was challenged. [challenger, challenged, result]
        InferenceChallenged {
            challenger: T::AccountId,
            challenged: T::AccountId,
            result: u32,
        },
        /// A challenge was resolved. [challenger, challenged, result, success]
        ChallengeResolved {
            challenger: T::AccountId,
            challenged: T::AccountId,
            result: u32,
            success: bool,
        },
        /// A validator was slashed. [validator, reason]
        ValidatorSlashed {
            validator: T::AccountId,
            reason: Vec<u8>,
        },
        /// inference score was updated for a validator
        PoiScoreUpdated {
            validator: T::AccountId,
            old_score: u32,
            new_score: u32,
            epoch: u32,
        },
        /// Epoch advanced in PoI system
        PoiEpochAdvanced { old_epoch: u32, new_epoch: u32 },
        /// inference score updated from off-chain worker
        ValidatorPoiScoreUpdated {
            validator: T::AccountId,
            poi_score: u64,
        },

        /// The deployment declared its determinism evidence.
        DeterminismDeclared {
            /// The evidence the deployment declared.
            evidence: crate::gate::DeterminismEvidence,
        },
        /// A raw inference result sat above the contribution ceiling.
        ///
        /// The share held at the ceiling of one and the fault count rose.
        ContributionCeilingApplied {
            /// The validator that reported the result.
            validator: T::AccountId,
            /// The raw result the chain holds.
            raw: u64,
            /// The share the contribution answered, in parts per billion.
            held: u64,
            /// The number of faults this validator now carries.
            faults: u32,
        },
        /// A challenge against a provisional result settled.
        InferenceChallengeSettled {
            /// The validator that reported the result.
            validator: T::AccountId,
            /// The validator that challenged the result.
            challenger: T::AccountId,
            /// The redundant execution matched the reported result.
            reporter_held: bool,
        },
    }

    /// Errors returned by the PoI pallet.
    #[pallet::error]
    pub enum Error<T> {
        /// The inference result already exists for this validator.
        InferenceAlreadySubmitted,
        /// The inference result does not exist.
        InferenceNotFound,
        /// The challenge is invalid (e.g., wrong result).
        InvalidChallenge,
        /// The confidence level is too low.
        ConfidenceTooLow,
        /// The challenge window has expired.
        ChallengeWindowExpired,
        /// The inference is too old to be challenged.
        InferenceTooOld,
        /// Cannot challenge own inference.
        CannotChallengeSelf,
        /// Challenge already exists for this validator.
        ChallengeAlreadyExists,
        /// Limit of iterations or weight exceeded.
        WeightLimitExceeded,
        /// Validator not found in register.
        ValidatorNotFound,
        /// This build carries no inference module.
        InferenceModuleAbsent,
        /// Determinism is unproven for this deployment.
        DeterminismUnproven,
        /// The task was generated rather than served.
        TaskSynthetic,
        /// The determinism evidence misses a condition.
        DeterminismEvidenceIncomplete,
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn offchain_worker(block_number: BlockNumberFor<T>) {
            if (block_number.saturated_into::<u32>()) % T::OffchainWorkerInterval::get() != 0 {
                return;
            }
            let _ = Self::run_offchain_computation(block_number);
        }
    }

    /// Dispatchable functions (extrinsics) for the PoI pallet.
    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Submit an inference result for the current epoch.
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::submit_inference())]
        pub fn submit_inference(
            origin: OriginFor<T>,
            result: u32,
            confidence: u32,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let current_epoch = CurrentEpoch::<T>::get();

            // One validator contributes one inference per epoch. Wait for the
            // next epoch to submit again.
            ensure!(
                InferenceResults::<T>::get(&who)
                    .map(|(_, held)| held != current_epoch)
                    .unwrap_or(true),
                Error::<T>::InferenceAlreadySubmitted
            );

            // Ensure confidence meets minimum threshold.
            ensure!(
                confidence >= T::MinInferenceConfidence::get(),
                Error::<T>::ConfidenceTooLow
            );

            // Store the inference result with current epoch.
            let old_result = InferenceResults::<T>::get(&who)
                .map(|(r, _)| r)
                .unwrap_or(0);
            InferenceResults::<T>::insert(&who, (result, current_epoch));

            // Emit inference score update event
            Self::deposit_event(Event::PoiScoreUpdated {
                validator: who.clone(),
                old_score: old_result,
                new_score: result,
                epoch: current_epoch,
            });

            // Emit an event.
            Self::deposit_event(Event::InferenceSubmitted {
                who: who.clone(),
                result,
                confidence,
            });
            // --- Telemetry: Inference submitted ---
            ::log::info!("[cerulea::poi][prometheus] inference_submitted{{validator={:?}}} {{result={},confidence={}}}", who, result, confidence);

            // --- PoS boost logic ---
            let boost = if confidence >= 90 {
                10
            } else if confidence >= 70 {
                5
            } else {
                2
            };
            if let Err(e) = T::PosInterface::boost_score(&who, boost) {
                ::log::warn!(
                    "[cerulea::poi][prometheus] boost_score_failed{{validator={:?}}} {:?}",
                    who,
                    e
                );
            } else {
                Self::deposit_event(Event::InferenceAccepted {
                    validator: who.clone(),
                    confidence,
                });
                // --- Telemetry: Inference accepted ---
                ::log::info!("[cerulea::poi][prometheus] inference_accepted{{validator={:?}}} {{confidence={}}}", who, confidence);
            }

            // --- DCF inference tracking ---
            if let Err(e) = T::DcfInterface::record_inference_activity(&who) {
                ::log::warn!("[cerulea::poi][prometheus] dcf_inference_tracking_failed{{validator={:?}}} {:?}", who, e);
            } else {
                ::log::info!(
                    "[cerulea::poi][prometheus] dcf_inference_tracked{{validator={:?}}}",
                    who
                );
            }
            Ok(())
        }

        /// Submit a challenge against another validator's inference result.
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::challenge_inference())]
        pub fn challenge_inference(
            origin: OriginFor<T>,
            challenged: T::AccountId,
            result: u32,
        ) -> DispatchResult {
            let challenger = ensure_signed(origin)?;

            // Get the inference result and its epoch.
            let (stored_result, epoch) =
                InferenceResults::<T>::get(&challenged).ok_or(Error::<T>::InferenceNotFound)?;

            // A validator carries no verdict on its own work. Submit the
            // challenge from a second validator account instead.
            ensure!(challenger != challenged, Error::<T>::CannotChallengeSelf);

            let current_epoch = CurrentEpoch::<T>::get();

            // One challenger lodges one challenge against one validator in one
            // epoch. Wait for the next epoch to lodge another.
            ensure!(
                Challenges::<T>::get(&challenger)
                    .map(|(held, _, lodged)| held != challenged || lodged != current_epoch)
                    .unwrap_or(true),
                Error::<T>::ChallengeAlreadyExists
            );

            // Ensure the inference is not too old.
            ensure!(
                current_epoch - epoch <= T::MaxInferenceAge::get(),
                Error::<T>::InferenceTooOld
            );

            // Ensure we're within the challenge window.
            ensure!(
                current_epoch - epoch <= T::ChallengeWindow::get(),
                Error::<T>::ChallengeWindowExpired
            );

            // Ensure the inference result matches.
            ensure!(stored_result == result, Error::<T>::InvalidChallenge);

            // Store the challenge with current epoch.
            Challenges::<T>::insert(&challenger, (&challenged, result, current_epoch));

            // Emit an event.
            Self::deposit_event(Event::InferenceChallenged {
                challenger: challenger.clone(),
                challenged: challenged.clone(),
                result,
            });
            // --- Telemetry: Inference challenged ---
            ::log::info!("[cerulea::poi][prometheus] inference_challenged{{challenger={:?},challenged={:?}}} {{result={}}}", challenger, challenged, result);

            // --- PoS slash logic ---
            let slash = 7; // Example: fixed penalty, could be parameterized
            if let Err(e) = T::PosInterface::slash_score(&challenged, slash) {
                ::log::warn!(
                    "[cerulea::poi][prometheus] slash_score_failed{{validator={:?}}} {:?}",
                    challenged,
                    e
                );
            } else {
                Self::deposit_event(Event::InferenceRejected {
                    validator: challenged.clone(),
                    confidence: 0,
                });
                Self::deposit_event(Event::ValidatorSlashed {
                    validator: challenged.clone(),
                    reason: b"Invalid inference".to_vec(),
                });
                // --- Telemetry: Inference rejected and validator slashed ---
                ::log::info!(
                    "[cerulea::poi][prometheus] inference_rejected{{validator={:?}}} 1",
                    challenged
                );
                ::log::info!(
                    "[cerulea::poi][prometheus] validator_slashed{{validator={:?}}} 1",
                    challenged
                );
            }
            Ok(())
        }

        /// Simulate inference computation (moved from DCF).
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::submit_inference())]
        pub fn simulate_inference(
            origin: OriginFor<T>,
            target_validator: T::AccountId,
        ) -> DispatchResult {
            ensure_signed(origin)?;

            let active_validators = T::PosInterface::get_active_validators();
            ensure!(
                active_validators.contains(&target_validator),
                Error::<T>::ValidatorNotFound
            );

            Self::record_inference_activity(&target_validator)?;

            let current_epoch = CurrentEpoch::<T>::get();
            let simulated_result =
                Self::simulate_inference_computation(&target_validator, current_epoch);

            log::info!(
                "PoI: Simulated inference for validator={:?}, result={}, confidence={}",
                target_validator,
                simulated_result.0,
                simulated_result.1
            );

            Self::deposit_event(Event::InferenceSubmitted {
                who: target_validator,
                result: simulated_result.0,
                confidence: simulated_result.1,
            });

            Ok(())
        }

        /// Apply inference scores computed by off-chain worker (moved from DCF).
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::submit_inference())]
        pub fn apply_offchain_poi_scores(
            origin: OriginFor<T>,
            block_number: u32,
        ) -> DispatchResult {
            ensure_signed(origin)?;

            let validators = T::PosInterface::get_active_validators();

            let estimated_weight = validators.len() as u64 * 50_000;
            if estimated_weight > T::MaxValidatorIterationWeight::get() {
                return Err(Error::<T>::WeightLimitExceeded.into());
            }

            let max_iterations = T::MaxLoopIterations::get().min(validators.len() as u32);
            let validators_to_process = &validators[..max_iterations as usize];

            let current_epoch = CurrentEpoch::<T>::get();

            for validator in validators_to_process.iter() {
                if let Ok(Some(poi_score)) = Self::get_offchain_poi_score(validator, block_number) {
                    if poi_score <= T::MaxValidatorScore::get() {
                        InferenceResults::<T>::insert(validator, (poi_score as u32, current_epoch));

                        let _ = T::DcfInterface::update_final_score(validator);

                        Self::deposit_event(Event::ValidatorPoiScoreUpdated {
                            validator: validator.clone(),
                            poi_score,
                        });
                    }
                }
            }

            Ok(())
        }

        /// Update a validator's inference score (moved from DCF).
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::submit_inference())]
        pub fn update_validator_inference_score(
            origin: OriginFor<T>,
            validator: T::AccountId,
        ) -> DispatchResult {
            ensure_signed(origin)?;

            if InferenceResults::<T>::contains_key(&validator) {
                T::DcfInterface::update_final_score(&validator)?;
            }
            Ok(())
        }

        /// Declare the determinism evidence for this deployment.
        ///
        /// Two validators running the same task produce identical bytes only
        /// where the weights, the seed, the arithmetic and the runtime are all
        /// pinned. The contribution stays absent until every condition holds.
        #[pallet::call_index(5)]
        #[pallet::weight(T::WeightInfo::submit_inference())]
        pub fn declare_determinism(
            origin: OriginFor<T>,
            evidence: crate::gate::DeterminismEvidence,
        ) -> DispatchResult {
            ensure_root(origin)?;
            ensure!(evidence.proven(), Error::<T>::DeterminismEvidenceIncomplete);
            DeterminismEvidenceHeld::<T>::put(evidence);
            Self::deposit_event(Event::DeterminismDeclared { evidence });
            Ok(())
        }

        /// Record one served inference against the score of a validator.
        ///
        /// The gate holds three conditions. The build carries the module. The
        /// deployment proved determinism. The task came from Cerulea
        /// Intelligence workload. A call failing any condition names the fix.
        #[pallet::call_index(6)]
        #[pallet::weight(T::WeightInfo::submit_inference())]
        pub fn record_served_inference(
            origin: OriginFor<T>,
            validator: T::AccountId,
            confidence: u32,
            source: crate::gate::TaskSource,
        ) -> DispatchResult {
            ensure_root(origin)?;
            Self::check_contribution_gate(source)?;

            let epoch = CurrentEpoch::<T>::get();
            let window = T::ChallengeWindow::get();
            ProvisionalResults::<T>::insert(
                &validator,
                crate::gate::ProvisionalResult::accept(confidence, source, epoch, window),
            );
            Self::handle_valid_inference(&validator, confidence)
        }
    }
}

// --- Helper implementations for Pallet --- //
impl<T: Config> Pallet<T> {
    /// Report whether the optional inference contribution counts here.
    ///
    /// The gate holds three conditions. The build carries the module. The
    /// deployment proved determinism. The task came from real workload.
    pub fn check_contribution_gate(source: crate::gate::TaskSource) -> DispatchResult {
        let declared = DeterminismEvidenceHeld::<T>::get().proven();
        match crate::gate::inspect(declared, source) {
            Ok(()) => Ok(()),
            Err(crate::gate::GateFailure::ModuleAbsentFromBuild) => {
                Err(Error::<T>::InferenceModuleAbsent.into())
            }
            Err(crate::gate::GateFailure::DeterminismUnproven) => {
                Err(Error::<T>::DeterminismUnproven.into())
            }
            Err(crate::gate::GateFailure::TaskSynthetic) => Err(Error::<T>::TaskSynthetic.into()),
        }
    }

    /// Report whether this build carries the inference contribution.
    pub const fn contribution_in_build() -> bool {
        crate::gate::module_in_build()
    }

    /// Settle one challenge against a provisional result.
    ///
    /// The committee re-executes the task. The losing side carries the
    /// penalty.
    pub fn settle_inference_challenge(
        validator: &T::AccountId,
        challenger: &T::AccountId,
        redundant: sp_std::vec::Vec<u32>,
    ) -> DispatchResult {
        let held = ProvisionalResults::<T>::get(validator).ok_or(Error::<T>::InferenceNotFound)?;
        let outcome = crate::gate::settle_challenge(held.value, &redundant);
        match outcome {
            crate::gate::ChallengeOutcome::ReporterHeld => {
                Self::handle_invalid_inference(challenger, InferenceErrorSeverity::Medium)?;
            }
            crate::gate::ChallengeOutcome::ChallengerHeld => {
                Self::handle_invalid_inference(validator, InferenceErrorSeverity::High)?;
                ProvisionalResults::<T>::remove(validator);
            }
        }
        Self::deposit_event(Event::InferenceChallengeSettled {
            validator: validator.clone(),
            challenger: challenger.clone(),
            reporter_held: matches!(outcome, crate::gate::ChallengeOutcome::ReporterHeld),
        });
        Ok(())
    }

    /// Exposed clean interface for querying the score
    /// Move the inference epoch forward.
    ///
    /// The framework pallet calls this at every epoch boundary, so the
    /// inference epoch and the chain epoch stay in step. A validator
    /// contributes one inference per epoch, and the counter here decides
    /// which epoch that is.
    pub fn advance_epoch(epoch: u32) {
        CurrentEpoch::<T>::put(epoch);
    }

    /// The inference contribution of one validator, as a share of one.
    ///
    /// The answer reads in parts per billion, the way every Standing Engine
    /// component reads. A validator with no recorded result answers zero. A
    /// raw result above [`crate::CONTRIBUTION_CEILING`] holds at the ceiling
    /// and records a fault.
    pub fn get_score(validator: &T::AccountId) -> u64 {
        Self::get_poi_score(validator)
    }

    pub fn get_poi_score(validator: &T::AccountId) -> u64 {
        Self::get_inference_score(validator)
    }

    pub fn get_inference_score(validator: &T::AccountId) -> u64 {
        Self::contribution_share(validator)
    }

    /// The raw inference result the chain holds for one validator.
    ///
    /// The figure carries no ceiling. Read [`Self::contribution_share`] for
    /// the share the Standing Engine composition takes.
    pub fn raw_inference_result(validator: &T::AccountId) -> u64 {
        InferenceResults::<T>::get(validator)
            .map(|(result, _)| result as u64)
            .unwrap_or(0)
    }

    /// The inference contribution of one validator, as a share of one.
    ///
    /// The share reads in parts per billion. A raw result above the ceiling
    /// holds at the ceiling, raises the fault count of that validator and
    /// reports the event.
    pub fn contribution_share(validator: &T::AccountId) -> u64 {
        let raw = Self::raw_inference_result(validator);
        if raw <= crate::CONTRIBUTION_CEILING {
            return raw;
        }
        let faults = ContributionFaults::<T>::mutate(validator, |count| {
            *count = count.saturating_add(1);
            *count
        });
        Self::deposit_event(Event::ContributionCeilingApplied {
            validator: validator.clone(),
            raw,
            held: crate::CONTRIBUTION_CEILING,
            faults,
        });
        crate::CONTRIBUTION_CEILING
    }

    pub fn submit_score(
        validator: T::AccountId,
        score: u64,
        block_number: BlockNumberFor<T>,
    ) -> Result<(), &'static str> {
        Self::store_offchain_poi_score(&validator, score, block_number)
    }

    pub fn verify_score(validator: &T::AccountId, score: u64, block_number: u32) -> bool {
        if let Ok(Some(stored_score)) = Self::get_offchain_poi_score(validator, block_number) {
            stored_score == score
        } else {
            false
        }
    }

    pub fn validator_inference_score(validator: &T::AccountId) -> u64 {
        Self::get_inference_score(validator)
    }

    /// Core helpers moved from DCF
    pub fn record_inference_activity(validator: &T::AccountId) -> DispatchResult {
        ValidatorInferenceCount::<T>::mutate(validator, |count| {
            *count = count.saturating_add(1);
        });

        let _ = T::DcfInterface::record_inference_activity(validator);
        Ok(())
    }

    pub fn collect_inference_data(
        validator: &T::AccountId,
        epoch: u32,
    ) -> Result<InferenceData<T>, &'static str> {
        if let Some((result, confidence)) = InferenceResults::<T>::get(validator) {
            return Ok(InferenceData {
                validator: validator.clone(),
                epoch,
                inference_result: result,
                confidence_score: confidence,
                timestamp: Self::get_offchain_timestamp(),
                data_sources: vec![b"poi_pallet".to_vec()],
            });
        }

        Self::collect_from_external_sources(validator, epoch)
    }

    fn collect_from_external_sources(
        validator: &T::AccountId,
        epoch: u32,
    ) -> Result<InferenceData<T>, &'static str> {
        let simulated_result = Self::simulate_inference_computation(validator, epoch);

        Ok(InferenceData {
            validator: validator.clone(),
            epoch,
            inference_result: simulated_result.0,
            confidence_score: simulated_result.1,
            timestamp: Self::get_offchain_timestamp(),
            data_sources: vec![b"simulation".to_vec()],
        })
    }

    fn simulate_inference_computation(validator: &T::AccountId, epoch: u32) -> (u32, u32) {
        let validator_bytes = validator.encode();
        let mut hash_input = validator_bytes;
        hash_input.extend_from_slice(&epoch.to_le_bytes());

        let hash = sp_core::hashing::blake2_256(&hash_input);
        let result = u32::from_le_bytes([hash[0], hash[1], hash[2], hash[3]])
            % T::PercentagePrecision::get();
        let confidence = T::InferenceConfidenceThresholdLow::get()
            + (u32::from_le_bytes([hash[4], hash[5], hash[6], hash[7]])
                % (T::InferenceConfidenceThresholdHigh::get()
                    - T::InferenceConfidenceThresholdLow::get()));

        (result, confidence)
    }

    fn compute_poi_score(data: &InferenceData<T>) -> u64 {
        let base_score = data.inference_result as u64;
        let confidence_multiplier = data.confidence_score as u64;

        let weighted_score =
            (base_score * confidence_multiplier) / T::PercentagePrecision::get() as u64;
        weighted_score.min(T::MaxValidatorScore::get())
    }

    fn get_offchain_timestamp() -> u64 {
        sp_io::offchain::timestamp().unix_millis()
    }

    fn submit_poi_score_update(
        validator: T::AccountId,
        poi_score: u64,
        block_number: BlockNumberFor<T>,
    ) -> Result<(), &'static str> {
        log::info!(
            "PoI: Computed inference score for validator={:?}, score={}, block={}",
            validator,
            poi_score,
            block_number.saturated_into::<u32>()
        );
        Self::store_offchain_poi_score(&validator, poi_score, block_number)?;
        Ok(())
    }

    fn store_offchain_poi_score(
        validator: &T::AccountId,
        score: u64,
        block_number: BlockNumberFor<T>,
    ) -> Result<(), &'static str> {
        let key = format!(
            "dcf::poi_score::{:?}::{}",
            validator,
            block_number.saturated_into::<u32>()
        );
        let storage_ref = StorageValueRef::persistent(key.as_bytes());

        let score_data: OffchainPoiScore<T> = OffchainPoiScore {
            validator: validator.clone(),
            score,
            block_number: block_number.saturated_into::<u32>(),
            timestamp: Self::get_offchain_timestamp(),
        };

        storage_ref.set(&score_data);
        Ok(())
    }

    fn get_offchain_poi_score(
        validator: &T::AccountId,
        block_number: u32,
    ) -> Result<Option<u64>, &'static str> {
        let key = format!("dcf::poi_score::{:?}::{}", validator, block_number);
        let storage_ref = StorageValueRef::persistent(key.as_bytes());

        match storage_ref.get::<OffchainPoiScore<T>>() {
            Ok(Some(score_data)) => Ok(Some(score_data.score)),
            Ok(None) => Ok(None),
            Err(_) => Err("Failed to retrieve inference score from off-chain storage"),
        }
    }

    pub fn handle_valid_inference(validator: &T::AccountId, confidence: u32) -> DispatchResult {
        let boost_amount = if confidence >= T::InferenceConfidenceThresholdHigh::get() {
            T::InferenceBoostHigh::get()
        } else if confidence >= T::InferenceConfidenceThresholdLow::get() {
            T::InferenceBoostMedium::get()
        } else {
            T::InferenceBoostLow::get()
        };

        Self::record_inference_activity(validator)?;

        T::PosInterface::boost_score(validator, boost_amount as u32)?;
        let _ = T::DcfInterface::update_final_score(validator);
        Ok(())
    }

    pub fn handle_invalid_inference(
        validator: &T::AccountId,
        severity: InferenceErrorSeverity,
    ) -> DispatchResult {
        let penalty = match severity {
            InferenceErrorSeverity::High => T::InferencePenaltyHigh::get(),
            InferenceErrorSeverity::Medium => T::InferencePenaltyMedium::get(),
            InferenceErrorSeverity::Low => T::InferencePenaltyLow::get(),
        };

        T::PosInterface::slash_score(validator, penalty as u32)?;
        let _ = T::DcfInterface::update_final_score(validator);
        Ok(())
    }

    pub fn run_offchain_computation(block_number: BlockNumberFor<T>) -> Result<(), &'static str> {
        let validators = T::PosInterface::get_active_validators();
        let current_epoch = CurrentEpoch::<T>::get();

        for validator in validators.iter() {
            if let Ok(inference_data) = Self::collect_inference_data(validator, current_epoch) {
                let poi_score = Self::compute_poi_score(&inference_data);
                // The comparison reads the raw result the chain holds, since
                // the figure the worker computes is a raw result too.
                let current_on_chain_score = Self::raw_inference_result(validator);

                if poi_score == current_on_chain_score {
                    log::trace!(
                        target: "poi",
                        "PoI off-chain worker: score unchanged for validator={:?}, score={}, skipping submission",
                        validator,
                        poi_score,
                    );
                    continue;
                }

                let _ = Self::submit_poi_score_update(validator.clone(), poi_score, block_number);
            }
        }

        log::info!("PoI off-chain worker completed computation");
        Ok(())
    }
}
