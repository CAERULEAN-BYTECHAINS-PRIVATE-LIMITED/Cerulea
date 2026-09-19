//! Mock runtime for DCF pallet tests

use super::*;
extern crate alloc;
use frame_support::{
    parameter_types,
    traits::{ConstU128, ConstU32, ConstU64, ConstU8},
    weights::Weight,
};
use sp_runtime::{
    traits::{BlakeTwo256, IdentityLookup},
    BuildStorage,
};

type Block = frame_system::mocking::MockBlock<Test>;

// Set a mock runtime to test the pallet.
frame_support::construct_runtime!(
    pub enum Test
    {
        System: frame_system,
        Balances: pallet_balances,
        Timestamp: pallet_timestamp,
        DcfPallet: crate,
        PalletCeruleaPos: pallet_cerulea_pos,
        PalletCeruleaPoi: pallet_cerulea_poi,
    }
);

impl frame_system::Config for Test {
    type BaseCallFilter = frame_support::traits::Everything;
    type BlockWeights = ();
    type BlockLength = ();
    type DbWeight = ();
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;
    type Nonce = u64;
    type Hash = sp_core::H256;
    type Hashing = BlakeTwo256;
    type AccountId = u64;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Block = Block;
    type RuntimeEvent = RuntimeEvent;
    type BlockHashCount = ConstU64<250>;
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = pallet_balances::AccountData<u128>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type SS58Prefix = ();
    type OnSetCode = ();
    type MaxConsumers = ConstU32<16>;
    type RuntimeTask = ();
    type ExtensionsWeightInfo = ();
    type SingleBlockMigrations = ();
    type MultiBlockMigrator = ();
    type PreInherents = ();
    type PostInherents = ();
    type PostTransactions = ();
}

impl pallet_balances::Config for Test {
    type MaxLocks = ConstU32<50>;
    type MaxReserves = ();
    type ReserveIdentifier = [u8; 8];
    type Balance = u128;
    type RuntimeEvent = RuntimeEvent;
    type DustRemoval = ();
    type ExistentialDeposit = ConstU128<500>;
    type AccountStore = System;
    type WeightInfo = ();
    type FreezeIdentifier = ();
    type MaxFreezes = ();
    type RuntimeHoldReason = ();
    type RuntimeFreezeReason = ();
    type DoneSlashHandler = ();
}

impl pallet_timestamp::Config for Test {
    type Moment = u64;
    type OnTimestampSet = ();
    type MinimumPeriod = ConstU64<5>;
    type WeightInfo = ();
}

// Values below match the runtime defaults for every shared parameter.
parameter_types! {
    pub const MinStake: u128 = 1000;
    pub const MaxValidators: u32 = 100;
    pub const MaxSlashingCount: u32 = 3;
    pub const MinValidatorScore: u32 = 50;
    pub const MinActiveValidators: u32 = 3;
    pub const ValidatorScoreDecay: u32 = 10;
    pub const MinInferenceConfidence: u32 = 80;
    pub const MaxInferenceAge: u32 = 10;
    pub const ChallengeWindow: u32 = 5;
    pub const InferenceReward: u128 = 1000;
    pub const ChallengeReward: u128 = 500;
    pub const MaxEpochHistory: u32 = 24;
    pub const DefaultPosWeight: u64 = 6000;
    pub const DefaultPoiWeight: u64 = 4000;
}

impl pallet_cerulea_pos::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type MinValidatorScore = MinValidatorScore;
    type MinActiveValidators = MinActiveValidators;
    type MaxValidators = MaxValidators;
    type ValidatorScoreDecay = ValidatorScoreDecay;
    type MaxSlashingCount = MaxSlashingCount;
    type MinStake = MinStake;
    type Balance = u128;
    type Currency = Balances;
    type LeaveCooldown = ConstU32<1000>;
    type ValidatorReward = ConstU128<10000>;
    type SlashPercent = ConstU32<10>;
    type MaxSlashPerEpoch = ConstU128<50000>;
    type MaxSlashPerValidator = ConstU128<20000>;
    type MaxRewardPerEpoch = ConstU128<30000>;
    type MaxRewardPerValidator = ConstU128<10000>;
    type SlashPenaltyDivisor = ConstU64<1000>;
    type MaxSlashPenalty = ConstU64<50>;
    type RewardBoostDivisor = ConstU64<1000>;
    type MaxRewardBoost = ConstU64<20>;
    type HighPerformanceScore = ConstU64<80>;
    type TopPerformerPercentage = ConstU32<20>;
    type ValidatorHandler = TestValidatorRegistry;
}

/// The stake interface the inference pallet calls in these tests.
/// Every call reaches the real stake pallet inside the mock runtime.
pub struct MockPosInterface;

impl pallet_cerulea_poi::PosInterface<u64> for MockPosInterface {
    fn boost_score(validator: &u64, weight: u32) -> DispatchResult {
        pallet_cerulea_pos::Pallet::<Test>::boost_score(
            frame_system::RawOrigin::Root.into(),
            *validator,
            weight,
        )
    }

    fn slash_score(validator: &u64, weight: u32) -> DispatchResult {
        pallet_cerulea_pos::Pallet::<Test>::slash_score(
            frame_system::RawOrigin::Root.into(),
            *validator,
            weight,
        )
    }

    fn get_active_validators() -> alloc::vec::Vec<u64> {
        pallet_cerulea_pos::Pallet::<Test>::get_active_validators()
    }
}

impl pallet_cerulea_poi::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type MinInferenceConfidence = MinInferenceConfidence;
    type MaxInferenceAge = MaxInferenceAge;
    type ChallengeWindow = ChallengeWindow;
    type InferenceReward = InferenceReward;
    type ChallengeReward = ChallengeReward;
    type PosInterface = MockPosInterface;
    type DcfInterface = crate::Pallet<Test>;
    type InferenceBoostLow = ConstU64<2>;
    type InferenceBoostMedium = ConstU64<5>;
    type InferenceBoostHigh = ConstU64<10>;
    type InferencePenaltyLow = ConstU64<1>;
    type InferencePenaltyMedium = ConstU64<3>;
    type InferencePenaltyHigh = ConstU64<7>;
    type InferenceConfidenceThresholdLow = ConstU32<70>;
    type InferenceConfidenceThresholdHigh = ConstU32<90>;
    type MaxValidatorScore = ConstU64<100>;
    type PercentagePrecision = ConstU32<10000>;
    type OffchainWorkerInterval = ConstU32<5>;
    type MaxValidatorIterationWeight = ConstU64<1_000_000_000>;
    type MaxLoopIterations = ConstU32<1000>;
}

// Mock weight info
pub struct MockWeightInfo;
impl WeightInfo for MockWeightInfo {
    fn on_initialize() -> Weight {
        Weight::from_parts(10_000, 0)
    }
    fn offchain_worker() -> Weight {
        Weight::from_parts(10_000, 0)
    }
    fn update_validator_stake_score() -> Weight {
        Weight::from_parts(10_000, 0)
    }
    fn update_validator_inference_score() -> Weight {
        Weight::from_parts(10_000, 0)
    }
    fn update_consensus_weights() -> Weight {
        Weight::from_parts(10_000, 0)
    }
    fn set_governance_mode() -> Weight {
        Weight::from_parts(10_000, 0)
    }
    fn sudo_advance_epoch() -> Weight {
        Weight::from_parts(50_000, 0)
    }
    fn submit_proposal() -> Weight {
        Weight::from_parts(20_000, 0)
    }
    fn vote_proposal() -> Weight {
        Weight::from_parts(15_000, 0)
    }
    fn execute_proposal() -> Weight {
        Weight::from_parts(30_000, 0)
    }
    fn join_validator_set() -> Weight {
        Weight::from_parts(25_000, 0)
    }
    fn leave_validator_set() -> Weight {
        Weight::from_parts(25_000, 0)
    }
    fn set_validator_name() -> Weight {
        Weight::from_parts(15_000, 0)
    }
    fn apply_offchain_poi_scores() -> Weight {
        Weight::from_parts(40_000, 0)
    }
    fn on_initialize_with_validators(v: u32) -> Weight {
        Weight::from_parts(10_000u64.saturating_mul(v as u64), 0)
    }
    fn apply_score_decay_multiple(v: u32) -> Weight {
        Weight::from_parts(5_000u64.saturating_mul(v as u64), 0)
    }
    fn validator_set_operations(v: u32) -> Weight {
        Weight::from_parts(3_000u64.saturating_mul(v as u64), 0)
    }
    fn runtime_api_calls(v: u32) -> Weight {
        Weight::from_parts(2_000u64.saturating_mul(v as u64), 0)
    }
    fn epoch_transition_multiple(v: u32) -> Weight {
        Weight::from_parts(15_000u64.saturating_mul(v as u64), 0)
    }
    fn governance_with_multiple_voters(v: u32) -> Weight {
        Weight::from_parts(8_000u64.saturating_mul(v as u64), 0)
    }
    fn join_validators() -> Weight {
        Weight::from_parts(10_000, 0)
    }
    fn leave_validators() -> Weight {
        Weight::from_parts(10_000, 0)
    }
    fn slash_validator() -> Weight {
        Weight::from_parts(10_000, 0)
    }
    fn cancel_leave_request() -> Weight {
        Weight::from_parts(10_000, 0)
    }
    fn report_validator_misbehavior() -> Weight {
        Weight::from_parts(10_000, 0)
    }
    fn simulate_inference() -> Weight {
        Weight::from_parts(10_000, 0)
    }
    fn propose_slash_validator() -> Weight {
        Weight::from_parts(10_000, 0)
    }
    fn propose_reward_validator() -> Weight {
        Weight::from_parts(10_000, 0)
    }
    fn propose_eject_validator() -> Weight {
        Weight::from_parts(10_000, 0)
    }
    fn increase_validator_stake() -> Weight {
        Weight::from_parts(10_000, 0)
    }
    fn decrease_validator_stake() -> Weight {
        Weight::from_parts(10_000, 0)
    }
    fn slash_validator_percentage() -> Weight {
        Weight::from_parts(10_000, 0)
    }
    fn slash_multiple_validators() -> Weight {
        Weight::from_parts(10_000, 0)
    }
    fn execute_proposals() -> Weight {
        Weight::from_parts(10_000, 0)
    }
    fn epoch_transition() -> Weight {
        Weight::from_parts(10_000, 0)
    }
    fn set_validator_metadata() -> Weight {
        Weight::from_parts(10_000, 0)
    }
    fn update_validator_activity() -> Weight {
        Weight::from_parts(10_000, 0)
    }
    fn distribute_epoch_rewards() -> Weight {
        Weight::from_parts(10_000, 0)
    }
    fn propose_default_reward_validator() -> Weight {
        Weight::from_parts(10_000, 0)
    }
    fn propose_default_reward_multiple_validators() -> Weight {
        Weight::from_parts(10_000, 0)
    }
    fn propose_reward_all_active_validators() -> Weight {
        Weight::from_parts(10_000, 0)
    }
    fn propose_reward_multiple_validators(_v: u32) -> Weight {
        Weight::from_parts(10_000, 0)
    }
}

/// The validator registry the framework pallet reads and writes in these
/// tests. Every record lives in the test externalities, so a test observes the
/// real effect of every registry call.
pub struct TestValidatorRegistry;

type TestProfile = crate::ValidatorProfile<u64, u128, u64>;

const REG_ACTIVE: &[u8] = b"registry/active";
const REG_SET: &[u8] = b"registry/set";
const REG_PENDING: &[u8] = b"registry/pending";
const REG_PROFILE: &[u8] = b"registry/profile/";
const REG_STATUS: &[u8] = b"registry/status/";
const REG_STATE: &[u8] = b"registry/state/";
const REG_NAME: &[u8] = b"registry/name/";
const REG_META: &[u8] = b"registry/meta/";
const REG_JOIN: &[u8] = b"registry/join/";
const REG_LEAVE: &[u8] = b"registry/leave/";
const REG_REMOVED: &[u8] = b"registry/removed/";
const REG_HISTORY: &[u8] = b"registry/history/";
const REG_SEEN: &[u8] = b"registry/seen/";
const REG_AUTHORED: &[u8] = b"registry/authored/";
const REG_MISSED: &[u8] = b"registry/missed/";
const REG_EPOCH_WEIGHTS: &[u8] = b"registry/epoch_weights";
const REG_DRAW_OWNER: &[u8] = b"registry/draw_owner/";
const REG_SEAL_KEY: &[u8] = b"registry/seal_key/";

fn reg_key(prefix: &[u8], validator: &u64) -> alloc::vec::Vec<u8> {
    let mut key = prefix.to_vec();
    key.extend_from_slice(&codec::Encode::encode(validator));
    key
}

fn reg_get<V: codec::Decode>(key: &[u8]) -> Option<V> {
    sp_io::storage::get(key).and_then(|raw| V::decode(&mut &raw[..]).ok())
}

fn reg_put<V: codec::Encode>(key: &[u8], value: &V) {
    sp_io::storage::set(key, &codec::Encode::encode(value));
}

fn reg_list(key: &[u8]) -> alloc::vec::Vec<u64> {
    reg_get::<alloc::vec::Vec<u64>>(key).unwrap_or_default()
}

/// Record the two keys of one validator in the mock registry.
///
/// The Authoring Engine records the draw key and the seal key of every
/// validator. The equivocation evidence path reads both back, so a test seeds
/// them the way the Admission Engine seeds them on a running chain.
pub fn registry_record_keys(validator: u64, draw_key: [u8; 32], seal_key: [u8; 32]) {
    let mut key = REG_DRAW_OWNER.to_vec();
    key.extend_from_slice(&draw_key);
    reg_put(&key, &validator);
    reg_put(&reg_key(REG_SEAL_KEY, &validator), &seal_key);
}

/// Seed the registry with a validator set. Genesis calls this.
///
/// A genesis validator reaches the chain through the Admission Engine, which
/// registers it in the stake pallet as well. The seeding therefore writes the
/// stake pallet registration beside the registry record, so a test reads the
/// same state a running chain holds.
pub fn registry_seed(validators: alloc::vec::Vec<u64>) {
    reg_put(REG_SET, &validators);
    reg_put(REG_ACTIVE, &validators);
    for validator in validators.iter() {
        reg_put(
            &reg_key(REG_STATUS, validator),
            &crate::ValidatorStatus::Active,
        );
        reg_put(&reg_key(REG_JOIN, validator), &0u32);
        pallet_cerulea_pos::Validators::<Test>::insert(validator, true);
    }
}

/// Read the epoch weights the framework pallet froze.
pub fn frozen_epoch_weights() -> alloc::vec::Vec<(u32, alloc::vec::Vec<(u64, u128, u128)>)> {
    reg_get(REG_EPOCH_WEIGHTS).unwrap_or_default()
}

/// The weight freezer the framework pallet calls at every epoch boundary.
/// The mock keeps every frozen set, so a test reads what the pallet sent.
pub struct TestWeightFreezer;

impl crate::traits::WeightFreezer<u64> for TestWeightFreezer {
    fn freeze_epoch_weights(epoch: u32, validators: &[(u64, u128, u128)]) {
        let mut held = frozen_epoch_weights();
        held.retain(|(held_epoch, _)| *held_epoch != epoch);
        held.push((epoch, validators.to_vec()));
        reg_put(REG_EPOCH_WEIGHTS, &held);
    }
}

/// The stake pallet reads the active validator set from the Admission Engine.
///
/// The runtime wires the validator registry pallet into the stake pallet for
/// exactly this read. The mock registry answers the same way, so a test sees
/// the one active set both pallets read.
impl pallet_cerulea_pos::ValidatorHandler<u64, u128> for TestValidatorRegistry {
    fn on_joined(_validator: &u64, _stake: u128) -> sp_runtime::DispatchResult {
        Ok(())
    }

    fn on_leave_requested(_validator: &u64) -> sp_runtime::DispatchResult {
        Ok(())
    }

    fn on_left(_validator: &u64) -> sp_runtime::DispatchResult {
        Ok(())
    }

    fn on_stake_increased(_validator: &u64, _amount: u128) -> sp_runtime::DispatchResult {
        Ok(())
    }

    fn on_stake_decreased(_validator: &u64, _amount: u128) -> sp_runtime::DispatchResult {
        Ok(())
    }

    fn on_slashed(_validator: &u64, _amount: u128, _penalty: u64) -> sp_runtime::DispatchResult {
        Ok(())
    }

    fn on_rewarded(_validator: &u64, _amount: u128, _boost: u64) -> sp_runtime::DispatchResult {
        Ok(())
    }

    fn get_validator_score(validator: &u64) -> u64 {
        crate::Pallet::<Test>::validator_inference_score(validator)
    }

    fn get_active_validators() -> alloc::vec::Vec<u64> {
        reg_list(REG_ACTIVE)
    }
}

impl crate::traits::ValidatorRegistryProvider<u64, u128, u64> for TestValidatorRegistry {
    fn get_active_validators() -> alloc::vec::Vec<u64> {
        reg_list(REG_ACTIVE)
    }

    fn draw_key_owner(draw_key: &[u8; 32]) -> Option<u64> {
        let mut key = REG_DRAW_OWNER.to_vec();
        key.extend_from_slice(draw_key);
        reg_get(&key)
    }

    fn validator_seal_key(validator: &u64) -> Option<[u8; 32]> {
        reg_get(&reg_key(REG_SEAL_KEY, validator))
    }

    fn get_validator_profile(validator: &u64) -> Option<TestProfile> {
        reg_get(&reg_key(REG_PROFILE, validator))
    }

    fn get_validator_status(validator: &u64) -> Option<crate::ValidatorStatus> {
        reg_get(&reg_key(REG_STATUS, validator))
    }

    fn get_validator_set() -> alloc::vec::Vec<u64> {
        reg_list(REG_SET)
    }

    fn update_validator_set(set: alloc::vec::Vec<u64>) {
        reg_put(REG_SET, &set);
    }

    fn is_validator_active(validator: &u64) -> bool {
        reg_list(REG_ACTIVE).contains(validator)
    }

    fn eject_validator(
        validator: &u64,
        _reason: crate::EjectionReason,
    ) -> sp_runtime::DispatchResult {
        let mut active = reg_list(REG_ACTIVE);
        active.retain(|held| held != validator);
        reg_put(REG_ACTIVE, &active);

        let mut set = reg_list(REG_SET);
        set.retain(|held| held != validator);
        reg_put(REG_SET, &set);

        reg_put(
            &reg_key(REG_STATUS, validator),
            &crate::ValidatorStatus::Ejected,
        );
        let now: u32 = frame_system::Pallet::<Test>::block_number() as u32;
        reg_put(&reg_key(REG_REMOVED, validator), &now);
        Ok(())
    }

    fn get_validator_state(validator: &u64) -> Option<crate::traits::ValidatorState> {
        reg_get(&reg_key(REG_STATE, validator))
    }

    fn update_validator_state(validator: &u64, state: crate::traits::ValidatorState) {
        reg_put(&reg_key(REG_STATE, validator), &state);
    }

    fn contains_validator_state(validator: &u64) -> bool {
        sp_io::storage::get(&reg_key(REG_STATE, validator)).is_some()
    }

    fn remove_validator_state(validator: &u64) {
        sp_io::storage::clear(&reg_key(REG_STATE, validator));
    }

    fn get_validator_name(validator: &u64) -> Option<alloc::vec::Vec<u8>> {
        reg_get(&reg_key(REG_NAME, validator))
    }

    fn set_validator_name(validator: &u64, name: alloc::vec::Vec<u8>) {
        reg_put(&reg_key(REG_NAME, validator), &name);
    }

    fn get_validator_metadata(validator: &u64) -> Option<crate::ValidatorMetadataInfo> {
        reg_get(&reg_key(REG_META, validator))
    }

    fn set_validator_metadata(validator: &u64, metadata: crate::ValidatorMetadataInfo) {
        reg_put(&reg_key(REG_META, validator), &metadata);
    }

    fn remove_validator_metadata(validator: &u64) {
        sp_io::storage::clear(&reg_key(REG_META, validator));
    }

    fn get_validator_detailed_cooldown_status(validator: &u64) -> Option<(u32, bool)> {
        let requested: u32 = reg_get(&reg_key(REG_LEAVE, validator))?;
        let now: u32 = frame_system::Pallet::<Test>::block_number() as u32;
        let cooldown: u32 =
            <<Test as pallet_cerulea_pos::Config>::LeaveCooldown as frame_support::traits::Get<
                u32,
            >>::get();
        let elapsed = now.saturating_sub(requested);
        let remaining = cooldown.saturating_sub(elapsed);
        Some((remaining, remaining == 0))
    }

    fn get_validator_leave_request(validator: &u64) -> Option<u32> {
        reg_get(&reg_key(REG_LEAVE, validator))
    }

    fn get_pending_actions() -> alloc::vec::Vec<(u64, crate::ValidatorAction)> {
        reg_get(REG_PENDING).unwrap_or_default()
    }

    fn remove_pending_action(validator: &u64) {
        let mut pending: alloc::vec::Vec<(u64, crate::ValidatorAction)> =
            reg_get(REG_PENDING).unwrap_or_default();
        pending.retain(|(held, _)| held != validator);
        reg_put(REG_PENDING, &pending);
    }

    fn add_pending_action(validator: &u64, action: crate::ValidatorAction) {
        let mut pending: alloc::vec::Vec<(u64, crate::ValidatorAction)> =
            reg_get(REG_PENDING).unwrap_or_default();
        pending.retain(|(held, _)| held != validator);
        pending.push((*validator, action));
        reg_put(REG_PENDING, &pending);
    }

    fn update_active_validators(active: alloc::vec::Vec<u64>) {
        reg_put(REG_ACTIVE, &active);
    }

    fn get_validator_join_time(validator: &u64) -> Option<u32> {
        reg_get(&reg_key(REG_JOIN, validator))
    }

    fn set_validator_join_time(validator: &u64, val: u32) {
        reg_put(&reg_key(REG_JOIN, validator), &val);
    }

    fn remove_validator_join_time(validator: &u64) {
        sp_io::storage::clear(&reg_key(REG_JOIN, validator));
    }

    fn set_validator_leave_request(validator: &u64, val: u32) {
        reg_put(&reg_key(REG_LEAVE, validator), &val);
    }

    fn remove_validator_leave_request(validator: &u64) {
        sp_io::storage::clear(&reg_key(REG_LEAVE, validator));
    }

    fn get_recently_removed_validator(validator: &u64) -> Option<u32> {
        reg_get(&reg_key(REG_REMOVED, validator))
    }

    fn set_recently_removed_validator(validator: &u64, val: u32) {
        reg_put(&reg_key(REG_REMOVED, validator), &val);
    }

    fn remove_recently_removed_validator(validator: &u64) {
        sp_io::storage::clear(&reg_key(REG_REMOVED, validator));
    }

    fn get_validator_performance_history(
        validator: &u64,
    ) -> alloc::vec::Vec<crate::PerformanceRecord> {
        reg_get(&reg_key(REG_HISTORY, validator)).unwrap_or_default()
    }

    fn set_validator_performance_history(
        validator: &u64,
        history: alloc::vec::Vec<crate::PerformanceRecord>,
    ) {
        reg_put(&reg_key(REG_HISTORY, validator), &history);
    }

    fn remove_validator_performance_history(validator: &u64) {
        sp_io::storage::clear(&reg_key(REG_HISTORY, validator));
    }

    fn get_validator_last_seen(validator: &u64) -> u32 {
        reg_get(&reg_key(REG_SEEN, validator)).unwrap_or(0)
    }

    fn set_validator_last_seen(validator: &u64, val: u32) {
        reg_put(&reg_key(REG_SEEN, validator), &val);
    }

    fn remove_validator_last_seen(validator: &u64) {
        sp_io::storage::clear(&reg_key(REG_SEEN, validator));
    }

    fn get_validator_blocks_authored(validator: &u64) -> u32 {
        reg_get(&reg_key(REG_AUTHORED, validator)).unwrap_or(0)
    }

    fn set_validator_blocks_authored(validator: &u64, val: u32) {
        reg_put(&reg_key(REG_AUTHORED, validator), &val);
    }

    fn remove_validator_blocks_authored(validator: &u64) {
        sp_io::storage::clear(&reg_key(REG_AUTHORED, validator));
    }

    fn get_validator_blocks_missed(validator: &u64) -> u32 {
        reg_get(&reg_key(REG_MISSED, validator)).unwrap_or(0)
    }

    fn set_validator_blocks_missed(validator: &u64, val: u32) {
        reg_put(&reg_key(REG_MISSED, validator), &val);
    }

    fn remove_validator_blocks_missed(validator: &u64) {
        sp_io::storage::clear(&reg_key(REG_MISSED, validator));
    }
}

impl Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type WeightFreezer = TestWeightFreezer;
    type ValidatorRegistry = TestValidatorRegistry;
    type Balance = u128;
    type WeightInfo = MockWeightInfo;

    // Epoch history and scoring weights
    type MaxEpochHistory = MaxEpochHistory;
    type DefaultPosWeight = DefaultPosWeight;
    type DefaultPoiWeight = DefaultPoiWeight;

    // Score decay and activity parameters
    type MaxInactiveEpochs = ConstU32<5>;
    type ScoreDecayInterval = ConstU32<10>;
    type ParticipationUpdateInterval = ConstU32<100>;
    type UnderperformanceCheckInterval = ConstU32<50>;
    type ValidatorProposalInterval = ConstU32<200>;
    type HealthMetricsInterval = ConstU32<1000>;
    type EpochLength = ConstU32<100>;

    // Block authorship reward and penalty
    type BlockAuthorshipBoost = ConstU64<10>;
    type MissedBlockPenalty = ConstU64<5>;

    // Validator metadata limits
    type MaxValidatorNameLength = ConstU32<32>;
    type MaxValidatorWebsiteLength = ConstU32<64>;
    type MaxValidatorContactLength = ConstU32<64>;
    type MaxValidatorDescriptionLength = ConstU32<128>;
    type MaxValidatorLocationLength = ConstU32<32>;
    type MaxPerformanceHistoryLength = ConstU32<100>;
    type MaxValidatorHistoryLength = ConstU32<10>;
    type MaxCommissionRate = ConstU32<10000>;

    // Misbehaviour reporting
    type MaxEvidenceLength = ConstU32<1000>;
    type MisbehaviorSlashThreshold = ConstU32<3>;

    // Off-chain worker bounds
    type OffchainWorkerTimeout = ConstU64<30000>;
    type EstimatedBlockTime = ConstU64<6000>;

    // Performance thresholds
    type MinPerformanceScore = ConstU64<30>;
    type MinParticipationRate = ConstU32<50>;
    type HighParticipationRate = ConstU32<90>;
    type MaxMissedBlocks = ConstU32<10>;
    type MaxMissedBlocksHigh = ConstU32<2>;
    type HealthyValidatorScore = ConstU64<50>;
    type HealthyParticipationRate = ConstU32<80>;
    type HealthyMissedBlocksMax = ConstU32<5>;

    // Score change thresholds
    type ScoreChangeThreshold = ConstU64<1000>;
    type ScoreChangePercentage = ConstU32<10>;
    type ScoreImprovementThreshold = ConstU64<1000>;
    type ScoreImprovementPercentage = ConstU32<10>;

    // Contribution balance thresholds
    type MaxPosContribution = ConstU32<90>;
    type MaxPoiContribution = ConstU32<90>;
    type ImbalanceWarningThreshold = ConstU32<85>;

    // Block processing intervals
    type LeaveRequestCheckInterval = ConstU32<10>;
    type MetricsUpdateInterval = ConstU32<10>;
    type ScoreRefreshInterval = ConstU32<50>;
    type DetailedLoggingInterval = ConstU32<100>;
    type ImbalanceCheckInterval = ConstU32<500>;

    // Validator set limits
    type TopValidatorsDisplayCount = ConstU32<5>;
    type HealthCheckSampleSize = ConstU32<5>;

    // Percentage constants
    type FullPercentage = ConstU32<100>;
    type HighPerformancePercentage = ConstU32<80>;

    // Reward distribution percentages
    type BaseRewardPercentage = ConstU32<60>;
    type PerformanceRewardPercentage = ConstU32<25>;
    type TopPerformerRewardPercentage = ConstU32<15>;

    // Trust score weights and bounds
    type TrustScoreUptimeWeight = ConstU64<4000>;
    type TrustScoreInferenceWeight = ConstU64<4000>;
    type TrustScoreSlashingWeight = ConstU64<2000>;
    type MaxTrustScore = ConstU64<10000>;
    type MinTrustScore = ConstU64<1000>;
    type MaxTrustScoreGrowthRate = ConstU32<500>;
    type MaxTrustScoreDecayRate = ConstU32<200>;
    type TrustScoreStabilityFactor = ConstU32<8000>;

    // Bounded collection sizes and error codes
    type MaxValidatorHistorySize = ConstU32<10>;
    type MaxValidatorNameSize = ConstU32<32>;
    type MaxRuntimeApiBoundedVecSize = ConstU32<100>;
    type MaxProposalActionBoundedVecSize = ConstU32<100>;
    type AuthorNotActiveErrorCode = ConstU8<1>;
    type AuthorMismatchErrorCode = ConstU8<2>;
}

// Validator lifecycle helpers for the framework pallet tests.
// The lifecycle itself lives in the validator registry pallet. These helpers
// drive the same state the framework pallet reads, so every lifecycle test
// keeps its meaning inside this mock runtime.

use crate::traits::ValidatorRegistryProvider as _;
use frame_support::traits::{Currency, ReservableCurrency};

/// Admit a validator. The caller reserves the minimum stake and joins the set.
pub fn mock_join_validators(
    origin: RuntimeOrigin,
    _name: Option<alloc::vec::Vec<u8>>,
) -> DispatchResult {
    let who = frame_system::ensure_signed(origin)?;

    let min_stake = <Test as pallet_cerulea_pos::Config>::MinStake::get();
    let free = <Balances as Currency<u64>>::free_balance(&who);
    frame_support::ensure!(
        free >= min_stake,
        pallet_cerulea_pos::Error::<Test>::InsufficientStake
    );

    let mut set = TestValidatorRegistry::get_validator_set();
    frame_support::ensure!(
        !set.contains(&who),
        pallet_cerulea_pos::Error::<Test>::ValidatorAlreadyExists
    );
    frame_support::ensure!(
        set.len()
            < <<Test as pallet_cerulea_pos::Config>::MaxValidators as frame_support::traits::Get<
                u32,
            >>::get() as usize,
        pallet_cerulea_pos::Error::<Test>::TooManyValidators
    );

    <Balances as ReservableCurrency<u64>>::reserve(&who, min_stake)
        .map_err(|_| pallet_cerulea_pos::Error::<Test>::InsufficientStake)?;

    pallet_cerulea_pos::Stake::<Test>::insert(who, min_stake);
    pallet_cerulea_pos::Validators::<Test>::insert(who, true);

    set.push(who);
    TestValidatorRegistry::update_validator_set(set);

    let mut active = TestValidatorRegistry::get_active_validators();
    if !active.contains(&who) {
        active.push(who);
    }
    TestValidatorRegistry::update_active_validators(active);

    let now: u32 = frame_system::Pallet::<Test>::block_number() as u32;
    TestValidatorRegistry::set_validator_join_time(&who, now);
    reg_put(&reg_key(REG_STATUS, &who), &crate::ValidatorStatus::Active);
    TestValidatorRegistry::add_pending_action(&who, crate::ValidatorAction::Join);

    // Admission notifies the framework pallet. The framework pallet opens the
    // validator state record the Standing Engine reads and records the join.
    <crate::Pallet<Test> as pallet_cerulea_pos::ValidatorHandler<u64, u128>>::on_joined(
        &who, min_stake,
    )?;
    Ok(())
}

/// Register a validator in the stake pallet, reading the Admission Engine
/// first.
///
/// The Admission Engine governs entry to the validator set. Admission records
/// the stake and the registration in the stake pallet, so a validator the
/// engine admitted already holds a registration. This helper reads that
/// record and holds that a second registration is refused. A validator the
/// engine never admitted registers here for the first time.
pub fn mock_register_validator(who: u64) -> DispatchResult {
    if pallet_cerulea_pos::Validators::<Test>::contains_key(who) {
        // Admission already registered this validator. A second registration
        // is refused. A full set answers with the ceiling error, and a set
        // with room answers with the duplicate error.
        assert!(
            pallet_cerulea_pos::Pallet::<Test>::register_validator(RuntimeOrigin::signed(who))
                .is_err(),
            "A second registration of validator {} passed. Read the registration admission made instead.",
            who
        );
        return Ok(());
    }
    pallet_cerulea_pos::Pallet::<Test>::register_validator(RuntimeOrigin::signed(who))
}

/// Cast one vote, waiting out the interval the pallet holds between votes.
///
/// The pallet holds a minimum interval between two votes from one account.
/// This helper moves the chain forward far enough to respect it, so a test
/// reads the vote rather than the rate limit.
pub fn mock_cast_vote(voter: u64, proposal_id: u32, approve: bool) -> DispatchResult {
    let interval = crate::RateLimitConfigStorage::<Test>::get().min_vote_interval;
    let now: u32 = frame_system::Pallet::<Test>::block_number() as u32;
    if let Some(last) =
        crate::LastOperationBlock::<Test>::get(voter, crate::DispatchableType::VoteProposal)
    {
        if now.saturating_sub(last) < interval {
            frame_system::Pallet::<Test>::set_block_number((last + interval) as u64);
            let _ = crate::BlockOperationCounts::<Test>::clear(u32::MAX, None);
        }
    }
    crate::Pallet::<Test>::vote_proposal(RuntimeOrigin::signed(voter), proposal_id, approve)
}

/// Carry one proposal to the quorum the framework pallet asks for.
///
/// A proposal passes once half of the active validators have voted and the
/// votes in favour outnumber the votes against. A single vote therefore leaves
/// the proposal pending. This helper votes in favour from the active
/// validators that have yet to vote, until the count reaches the quorum. A
/// test that calls it reads the quorum rule rather than a single vote.
pub fn mock_approve_proposal(proposal_id: u32) -> DispatchResult {
    let active = crate::ActiveValidators::<Test>::get();
    let quorum = (active.len() as u32 + 1) / 2;

    let mut cast = crate::Proposals::<Test>::get(proposal_id)
        .map(|held| held.votes_for + held.votes_against)
        .unwrap_or(0);

    // The pallet holds a minimum interval between two votes from one account.
    // The helper moves the chain forward far enough to respect it.
    let interval = crate::RateLimitConfigStorage::<Test>::get().min_vote_interval;

    for voter in active.iter() {
        if cast >= quorum {
            break;
        }
        if crate::ProposalVotes::<Test>::contains_key(proposal_id, voter) {
            continue;
        }

        let now: u32 = frame_system::Pallet::<Test>::block_number() as u32;
        if let Some(last) =
            crate::LastOperationBlock::<Test>::get(voter, crate::DispatchableType::VoteProposal)
        {
            if now.saturating_sub(last) < interval {
                frame_system::Pallet::<Test>::set_block_number((last + interval) as u64);
                let _ = crate::BlockOperationCounts::<Test>::clear(u32::MAX, None);
            }
        }

        crate::Pallet::<Test>::vote_proposal(RuntimeOrigin::signed(*voter), proposal_id, true)?;
        cast += 1;
    }
    Ok(())
}

/// Admit a validator through the Admission Engine, reading the set first.
///
/// The engine admits a validator once. A candidate already seated is refused,
/// which this helper holds. A candidate outside the set joins here.
pub fn mock_admit_validator(who: u64, name: Option<alloc::vec::Vec<u8>>) -> DispatchResult {
    if TestValidatorRegistry::get_validator_set().contains(&who) {
        frame_support::assert_noop!(
            mock_join_validators(RuntimeOrigin::signed(who), None),
            pallet_cerulea_pos::Error::<Test>::ValidatorAlreadyExists
        );
        return Ok(());
    }
    mock_join_validators(RuntimeOrigin::signed(who), name)
}

/// Record a leave request. The validator leaves at the end of the cooldown.
pub fn mock_leave_validators(origin: RuntimeOrigin) -> DispatchResult {
    let who = frame_system::ensure_signed(origin)?;

    let set = TestValidatorRegistry::get_validator_set();
    frame_support::ensure!(
        set.contains(&who),
        pallet_cerulea_pos::Error::<Test>::ValidatorNotInSet
    );
    frame_support::ensure!(
        TestValidatorRegistry::get_validator_leave_request(&who).is_none(),
        pallet_cerulea_pos::Error::<Test>::LeaveCooldownActive
    );

    let now: u32 = frame_system::Pallet::<Test>::block_number() as u32;
    TestValidatorRegistry::set_validator_leave_request(&who, now);
    reg_put(&reg_key(REG_STATUS, &who), &crate::ValidatorStatus::Leaving);
    TestValidatorRegistry::add_pending_action(&who, crate::ValidatorAction::Leave);

    let mut active = TestValidatorRegistry::get_active_validators();
    active.retain(|held| held != &who);
    TestValidatorRegistry::update_active_validators(active);

    // The Admission Engine announces every status change. A validator that
    // asked to leave moves from active to leaving, and the announcement
    // carries the block the request arrived in.
    crate::Pallet::<Test>::deposit_event_with_evm_compat(crate::Event::ValidatorStatusChanged {
        validator: who,
        old_status: crate::ValidatorStatus::Active,
        new_status: crate::ValidatorStatus::Leaving,
        block_number: now,
    });
    Ok(())
}

/// Complete the exit of one validator at the end of the cooldown.
///
/// The Admission Engine releases the stake once the cooldown passed. The
/// validator leaves the set, the reserve returns to the free balance and the
/// engine announces the final status.
pub fn mock_complete_exit(who: u64) -> DispatchResult {
    let held = pallet_cerulea_pos::Stake::<Test>::get(who);
    <Balances as ReservableCurrency<u64>>::unreserve(&who, held);
    pallet_cerulea_pos::Stake::<Test>::insert(who, 0u128);

    let mut set = TestValidatorRegistry::get_validator_set();
    set.retain(|seated| seated != &who);
    TestValidatorRegistry::update_validator_set(set);
    reg_put(
        &reg_key(REG_STATUS, &who),
        &crate::ValidatorStatus::Inactive,
    );

    let now: u32 = frame_system::Pallet::<Test>::block_number() as u32;
    crate::Pallet::<Test>::deposit_event_with_evm_compat(crate::Event::ValidatorStatusChanged {
        validator: who,
        old_status: crate::ValidatorStatus::Leaving,
        new_status: crate::ValidatorStatus::Inactive,
        block_number: now,
    });
    Ok(())
}

/// Record one offence against a validator.
///
/// The Penalty policy counts offences against a validator. The stake pallet
/// removes a validator once the count reaches the ceiling the deployment set.
/// The amount the deployment takes travels the separate root call, so this
/// helper carries the offence alone.
pub fn mock_slash_validator(validator: &u64, _amount: u128) -> DispatchResult {
    pallet_cerulea_pos::Pallet::<Test>::slash_validator_call(
        RuntimeOrigin::signed(*validator),
        *validator,
    )
}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut storage = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    pallet_balances::GenesisConfig::<Test> {
        balances: vec![
            (1, 10000),
            (2, 10000),
            (3, 10000),
            (4, 10000),
            (10, 100000),
            (11, 100000),
            (12, 100000),
            (13, 100000),
            (14, 100000),
            (15, 100000),
            (16, 100000),
            (17, 100000),
            (18, 100000),
            (19, 100000),
            (20, 100000),
            (21, 100000),
        ],
        dev_accounts: None,
    }
    .assimilate_storage(&mut storage)
    .unwrap();

    crate::GenesisConfig::<Test> {
        validators: vec![1, 2, 3],
        validator_scores: vec![6000, 7000, 8000],
        validator_stakes: vec![1000, 1000, 1000],
        validator_names: vec![],
        current_epoch: 0,
        epoch_config: EpochConfig {
            blocks_per_epoch: 10,
            min_stake: 1000,
            max_validators: 100,
        },
        strict_validation: false,
    }
    .assimilate_storage(&mut storage)
    .unwrap();

    let mut ext: sp_io::TestExternalities = storage.into();
    ext.execute_with(|| {
        registry_seed(alloc::vec![1u64, 2u64, 3u64]);
    });
    ext
}

pub fn new_test_ext_with_genesis(
    genesis_config: crate::GenesisConfig<Test>,
) -> sp_io::TestExternalities {
    let mut storage = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    // Initialize balances for all potential test accounts (including genesis validators)
    let mut balances = vec![(1, 10000), (2, 10000), (3, 10000), (4, 10000), (5, 10000)];

    // Add balances for genesis validators
    for (i, validator) in genesis_config.validators.iter().enumerate() {
        let balance = if i < genesis_config.validator_stakes.len() {
            genesis_config.validator_stakes[i] * 2
        } else {
            2000 // Default balance
        };
        balances.push((*validator, balance));
    }

    pallet_balances::GenesisConfig::<Test> {
        balances,
        dev_accounts: None,
    }
    .assimilate_storage(&mut storage)
    .unwrap();

    // Use the provided genesis record
    let seeded = genesis_config.validators.clone();
    genesis_config.assimilate_storage(&mut storage).unwrap();

    let mut ext: sp_io::TestExternalities = storage.into();
    ext.execute_with(|| {
        registry_seed(seeded);
    });
    ext
}

// Compile-time contract check.
// The block below names every trait the mock runtime claims to satisfy.
// A trait that gains an item stops this crate from compiling here first, so a
// mock can never drift behind its trait again. The check runs on every build
// of the test target, which every commit gate runs.
const _: fn() = || {
    fn settings_hold<T: crate::Config>() {}
    settings_hold::<Test>();

    fn registry_holds<T: crate::traits::ValidatorRegistryProvider<u64, u128, u64>>() {}
    registry_holds::<<Test as crate::Config>::ValidatorRegistry>();

    fn weight_freezer_holds<T: crate::traits::WeightFreezer<u64>>() {}
    weight_freezer_holds::<<Test as crate::Config>::WeightFreezer>();

    fn stake_settings_hold<T: pallet_cerulea_pos::Config>() {}
    stake_settings_hold::<Test>();

    fn inference_settings_hold<T: pallet_cerulea_poi::Config>() {}
    inference_settings_hold::<Test>();
};
