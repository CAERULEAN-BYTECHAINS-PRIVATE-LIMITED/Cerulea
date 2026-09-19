//! Mock runtime for Cerulea Runtime testing
//!
//! This module provides a comprehensive mock runtime that can be used for testing
//! the entire Cerulea system including all pallets and their interactions.

use crate::{AccountId, Balance, Balances, BlockNumber, Runtime, RuntimeOrigin, System};
use frame_support::{
    parameter_types,
    traits::{
        ConstU128, ConstU32, ConstU64, ConstU8, Currency, Everything, Hooks, ReservableCurrency,
    },
    weights::Weight,
    PalletId,
};
use sp_core::H256;
use sp_runtime::AccountId32;
use sp_runtime::{
    traits::{BlakeTwo256, IdentityLookup, Verify},
    BuildStorage, Perbill,
};

/// Helper function to create AccountId from u64
pub fn account_id(id: u64) -> AccountId {
    AccountId32::from([id as u8; 32])
}

/// Helper function to create AccountId from u64 (accounts are pre-funded in genesis)
pub fn funded_account_id(id: u64) -> AccountId {
    // Handle special case for test account 999 -> 99
    let account_byte = if id == 999 { 99u8 } else { id as u8 };
    AccountId32::from([account_byte; 32])
}
use sp_io;

// Re-export the main runtime for testing

// Test settings parameters
parameter_types! {
    pub const TestBlockHashCount: u64 = 250;
    pub const TestMaximumBlockWeight: Weight = Weight::from_parts(1024, 0);
    pub const TestMaximumBlockLength: u32 = 2 * 1024;
    pub const TestAvailableBlockRatio: Perbill = Perbill::from_percent(75);
    pub const TestExistentialDeposit: Balance = 500;
    pub const TestMaxLocks: u32 = 50;
    pub const TestMaxReserves: u32 = 50;
}

/// The balance every test account starts with.
///
/// The balances module refuses a genesis balance below the existential
/// deposit the runtime carries, so the endowment sits far above it.
pub const TEST_ENDOWMENT: Balance = 100_000 * crate::Cerulea;

/// The settings one test runtime starts with
pub struct MockRuntimeConfig {
    pub validators: Vec<AccountId>,
    pub validator_stakes: Vec<Balance>,
    pub validator_scores: Vec<u32>,
    pub endowed_accounts: Vec<(AccountId, Balance)>,
    pub sudo_key: AccountId,
}

impl Default for MockRuntimeConfig {
    fn default() -> Self {
        Self {
            validators: vec![
                AccountId32::from([1u8; 32]),
                AccountId32::from([2u8; 32]),
                AccountId32::from([3u8; 32]),
                AccountId32::from([4u8; 32]),
            ],
            validator_stakes: vec![10000, 10000, 10000, 10000],
            validator_scores: vec![60, 70, 80, 90], // Changed to u32
            // Every endowment sits above the existential deposit the
            // runtime carries. A balance below that deposit fails the
            // genesis build of the balances module.
            endowed_accounts: vec![
                (AccountId32::from([1u8; 32]), TEST_ENDOWMENT),
                (AccountId32::from([2u8; 32]), TEST_ENDOWMENT),
                (AccountId32::from([3u8; 32]), TEST_ENDOWMENT),
                (AccountId32::from([4u8; 32]), TEST_ENDOWMENT),
                (AccountId32::from([5u8; 32]), TEST_ENDOWMENT),
                // Add test accounts
                (AccountId32::from([99u8; 32]), TEST_ENDOWMENT), // For test_account
            ],
            sudo_key: AccountId32::from([1u8; 32]),
        }
    }
}

/// Build the externalities of a test chain under the default settings
pub fn new_test_ext() -> sp_io::TestExternalities {
    // The default settings carries four validators, every one of
    // them endowed above the existential deposit.
    new_test_ext_with_config(MockRuntimeConfig::default())
}

/// Build the externalities of a test chain under the given settings
pub fn new_test_ext_with_config(config: MockRuntimeConfig) -> sp_io::TestExternalities {
    let mut storage = frame_system::GenesisConfig::<Runtime>::default()
        .build_storage()
        .unwrap();

    // The balances of every endowed account
    pallet_balances::GenesisConfig::<Runtime> {
        balances: config.endowed_accounts,
        dev_accounts: None,
    }
    .assimilate_storage(&mut storage)
    .unwrap();

    pallet_sudo::GenesisConfig::<Runtime> {
        key: Some(config.sudo_key.clone()),
    }
    .assimilate_storage(&mut storage)
    .unwrap();

    // The framework pallet, the stake pallet, the inference pallet and the
    // validator registry all take the same validator set, so every read
    // agrees with every other read.
    if !config.validators.is_empty() {
        pallet_cerulea_dcf::GenesisConfig::<Runtime> {
            validators: config.validators.clone(),
            validator_scores: config.validator_scores.clone(),
            validator_stakes: config.validator_stakes.clone(),
            current_epoch: 0,
            epoch_config: pallet_cerulea_dcf::EpochConfig {
                blocks_per_epoch: 100,
                min_stake: 1_000,
                max_validators: 100,
            },
            validator_names: alloc::vec![None; config.validators.len()],
            strict_validation: false,
        }
        .assimilate_storage(&mut storage)
        .unwrap();

        // The stake pallet takes the scores alone. A test registers a
        // validator there itself, so the register starts empty.
        pallet_cerulea_pos::GenesisConfig::<Runtime> {
            validators: alloc::vec![],
            validator_scores: alloc::vec![],
            current_epoch: 0,
            slashing_count: alloc::vec![],
        }
        .assimilate_storage(&mut storage)
        .unwrap();

        // The inference pallet takes its results from the validators
        // themselves, so the register starts empty.
        pallet_cerulea_poi::GenesisConfig::<Runtime> {
            inference_results: alloc::vec![],
            challenges: alloc::vec![],
            current_epoch: 0,
        }
        .assimilate_storage(&mut storage)
        .unwrap();

        pallet_cerulea_dvf::GenesisConfig::<Runtime> {
            initial_validator_weights: config
                .validators
                .iter()
                .cloned()
                .zip(config.validator_stakes.iter().cloned())
                .zip(config.validator_scores.iter().cloned())
                .map(|((validator, stake), score)| (validator, stake, score as u128))
                .collect(),
            initial_validator_keys: alloc::vec![],
            strict_validation: false,
        }
        .assimilate_storage(&mut storage)
        .unwrap();
    }

    let mut ext = sp_io::TestExternalities::from(storage);

    // Setup keystore for consensus testing
    // Keystore setup removed for simplicity

    ext.execute_with(|| {
        System::set_block_number(1);
    });

    ext
}

/// Build the externalities of a test chain carrying many validators
pub fn new_test_ext_with_validators(validator_count: u32) -> sp_io::TestExternalities {
    let validators: Vec<AccountId> = (1..=validator_count as u64)
        .map(|i| AccountId32::from([i as u8; 32]))
        .collect();
    let validator_stakes: Vec<Balance> = vec![10000; validator_count as usize];
    // Every score sits inside the hundred point range the validator
    // registry admits at genesis.
    let validator_scores: Vec<u32> = (0..validator_count).map(|i| 60 + (i % 41)).collect();
    let mut endowed_accounts: Vec<(AccountId, Balance)> = validators
        .iter()
        .map(|v| (v.clone(), TEST_ENDOWMENT))
        .collect();

    // Add some additional accounts
    endowed_accounts.extend(vec![
        (AccountId32::from([100u8; 32]), TEST_ENDOWMENT * 10), // Rich account for testing
        (AccountId32::from([101u8; 32]), TEST_ENDOWMENT * 10),
        (AccountId32::from([102u8; 32]), TEST_ENDOWMENT * 10),
    ]);

    let config = MockRuntimeConfig {
        validators,
        validator_stakes,
        validator_scores,
        endowed_accounts,
        sudo_key: AccountId32::from([1u8; 32]),
    };

    new_test_ext_with_config(config)
}

/// Build the externalities of a test chain running under governance
pub fn new_test_ext_for_governance() -> sp_io::TestExternalities {
    let mut ext = new_test_ext();

    ext.execute_with(|| {
        // Enable governance mode
        // Enable governance mode for testing
        pallet_cerulea_dcf::GovernanceModeEnabled::<Runtime>::put(true);
    });

    ext
}

/// Create test externalities for epoch transition testing
pub fn new_test_ext_for_epochs() -> sp_io::TestExternalities {
    let mut ext = new_test_ext();

    ext.execute_with(|| {
        // Setup epoch settings for faster testing
        let epoch_config = pallet_cerulea_dcf::EpochConfig {
            blocks_per_epoch: 100, // Shorter epochs for testing
            min_stake: 1000,
            max_validators: 100,
        };
        pallet_cerulea_dcf::EpochConfigStorage::<Runtime>::put(epoch_config);
    });

    ext
}

/// Helper function to advance to a specific block number
pub fn advance_to_block(n: BlockNumber) {
    while System::block_number() < n {
        let current = System::block_number();
        System::set_block_number(current + 1);

        // Trigger on_initialize for all pallets
        use frame_support::traits::Hooks;
        System::on_initialize(current + 1);
        pallet_cerulea_dcf::Pallet::<Runtime>::on_initialize(current + 1);

        // Trigger on_finalize for all pallets
        pallet_cerulea_dcf::Pallet::<Runtime>::on_finalize(current + 1);
        System::on_finalize(current + 1);
    }
}

/// Helper function to advance by a number of blocks
pub fn advance_blocks(n: BlockNumber) {
    let current = System::block_number();
    advance_to_block(current + n);
}

/// Helper function to advance to the next epoch
pub fn advance_to_next_epoch() {
    let epoch_config = pallet_cerulea_dcf::EpochConfigStorage::<Runtime>::get();
    let current_epoch = pallet_cerulea_dcf::CurrentEpoch::<Runtime>::get();
    let next_epoch_block = (current_epoch + 1) * epoch_config.blocks_per_epoch + 1;
    advance_to_block(next_epoch_block as BlockNumber);
}

/// Helper function to create a test account with balance
pub fn create_funded_account(account_id: AccountId, balance: Balance) {
    let _ = Balances::make_free_balance_be(&account_id, balance);
}

/// Helper function to setup a validator with stake
pub fn setup_validator_with_stake(validator: AccountId, stake: Balance) {
    // The account funds itself only where the balance it already holds
    // falls short of the stake and the existential deposit together. An
    // account endowed at genesis therefore keeps the balance it started
    // with.
    let held = Balances::free_balance(&validator);
    if held < stake.saturating_add(crate::EXISTENTIAL_DEPOSIT) {
        create_funded_account(validator.clone(), TEST_ENDOWMENT + stake);
    }
    let _ = Balances::reserve(&validator, stake);
    pallet_cerulea_pos::Stake::<Runtime>::insert(&validator, stake);
}

/// Helper function to create multiple test validators
pub fn create_test_validators(count: u32) -> Vec<AccountId> {
    let validators: Vec<AccountId> = (100..100 + count as u64)
        .map(|i| AccountId32::from([i as u8; 32]))
        .collect();

    for validator in &validators {
        setup_validator_with_stake(validator.clone(), 10000);

        // Create validator state
        let validator_state = pallet_cerulea_dcf::ValidatorState {
            last_active_epoch: 0,
            current: pallet_cerulea_dcf::EpochStats {
                epoch: 0,
                stake_score: 1000,
                // PoI=0: AI inference not yet integrated; inference_score stays 0 globally.
                // With DefaultPoiWeight=0, final_score = (stake_score * 10000 + 0 * 0) / 10000 = stake_score.
                inference_score: 0,
                final_score: 1000,
                authored_blocks: 0,
                missed_blocks: 0,
            },
            history: frame_support::BoundedVec::default(),
            uptime: 0,
            inference_success_count: 0,
            participation_rate: 100,
            inference_count: 0,
            last_active_block: 0,
            name: None,
            trust_score: 0,
        };

        pallet_cerulea_dcf::ValidatorStates::<Runtime>::insert(validator, validator_state);
    }

    validators
}

/// Test settings for different scenarios
pub enum TestScenario {
    Basic,
    MultiValidator(u32),
    Governance,
    EpochTransition,
    HighStake,
    StressTest,
}

/// Create test externalities for specific scenarios
pub fn new_test_ext_for_scenario(scenario: TestScenario) -> sp_io::TestExternalities {
    match scenario {
        TestScenario::Basic => new_test_ext(),
        TestScenario::MultiValidator(count) => new_test_ext_with_validators(count),
        TestScenario::Governance => new_test_ext_for_governance(),
        TestScenario::EpochTransition => new_test_ext_for_epochs(),
        TestScenario::HighStake => {
            let config = MockRuntimeConfig {
                validators: vec![
                    AccountId32::from([1u8; 32]),
                    AccountId32::from([2u8; 32]),
                    AccountId32::from([3u8; 32]),
                ],
                validator_stakes: vec![100000, 150000, 200000], // High stakes
                validator_scores: vec![80, 90, 95],
                endowed_accounts: vec![
                    (AccountId32::from([1u8; 32]), TEST_ENDOWMENT),
                    (AccountId32::from([2u8; 32]), TEST_ENDOWMENT),
                    (AccountId32::from([3u8; 32]), TEST_ENDOWMENT),
                ],
                sudo_key: AccountId32::from([1u8; 32]),
            };
            new_test_ext_with_config(config)
        }
        TestScenario::StressTest => new_test_ext_with_validators(50),
    }
}

// Re-export commonly used testing utilities
pub use frame_support::{assert_err, assert_noop, assert_ok};
pub use sp_runtime::traits::BadOrigin;
