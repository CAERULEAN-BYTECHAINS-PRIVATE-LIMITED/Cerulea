// This file is part of the CBC Runtime.
// It defines functions for generating JSON-based genesis configurations used by the blockchain node
// when starting a development or local testnet chain.

use crate::{AccountId, BalancesConfig, RuntimeGenesisConfig, SudoConfig, CBC}; // Runtime-specific types
use alloc::{vec, vec::Vec}; // Alloc crate for dynamic arrays
use frame_support::build_struct_json_patch; // Macro to build partial JSON patches for genesis config
use serde_json::Value; // JSON value type
use sp_genesis_builder::{self, PresetId}; // Genesis builder utilities and PresetId for pre-defined configs
use sp_keyring::Ed25519Keyring; // Keyring to easily access dev accounts

/// Returns a genesis configuration in JSON format with the specified authorities,
/// endowed accounts, and sudo (root) key.
fn testnet_genesis(
	initial_validators: Vec<AccountId>, // Validator accounts
	endowed_accounts: Vec<AccountId>,   // Accounts pre-funded with balance
	root: AccountId,                    // Root (sudo) key
) -> Value {
	testnet_genesis_with_stakes(
		initial_validators,
		endowed_accounts,
		root,
		None, // Use default stakes
	)
}

/// Returns a genesis configuration with custom validator stakes
fn testnet_genesis_with_stakes(
	initial_validators: Vec<AccountId>, // Validator accounts
	endowed_accounts: Vec<AccountId>,   // Accounts pre-funded with balance
	root: AccountId,                    // Root (sudo) key
	validator_stakes: Option<Vec<u128>>, // Optional custom stakes
) -> Value {
	testnet_genesis_with_stakes_and_names(
		initial_validators,
		endowed_accounts,
		root,
		validator_stakes,
		None, // No names by default
	)
}

/// Returns a genesis configuration with custom validator stakes and names
fn testnet_genesis_with_stakes_and_names(
	initial_validators: Vec<AccountId>, // Validator accounts
	endowed_accounts: Vec<AccountId>,   // Accounts pre-funded with balance
	root: AccountId,                    // Root (sudo) key
	validator_stakes: Option<Vec<u128>>, // Optional custom stakes
	validator_names: Option<Vec<Option<Vec<u8>>>>, // Optional validator names
) -> Value {
	// Create initial validator scores
	let validator_scores = vec![80; initial_validators.len()]; // 80% initial score

	// Create validator stakes - use provided stakes or defaults in CBC
	let stakes = validator_stakes.unwrap_or_else(|| {
		// Default stakes: 10M, 8M, 6M, 5M, 3M CBC
		initial_validators.iter().enumerate().map(|(i, _)| {
			match i {
				0 => 10_000_000 * CBC, // Alice: 10M CBC
				1 => 8_000_000 * CBC,  // Bob: 8M CBC
				2 => 6_000_000 * CBC,  // Charlie: 6M CBC
				3 => 5_000_000 * CBC,  // Dave: 5M CBC
				_ => 3_000_000 * CBC,  // Others: 3M CBC
			}
		}).collect()
	});

	// Create validator names - use provided names or defaults
	let names = validator_names.unwrap_or_else(|| {
		vec![None; initial_validators.len()] // No names by default
	});

	// Create initial inference results
	let inference_results = initial_validators
		.iter()
		.map(|acc| (acc.clone(), 80)) // Consistent with validator_scores
		.collect::<Vec<_>>();

	build_struct_json_patch!(RuntimeGenesisConfig {
		// Configure initial balances for all endowed accounts with large amounts of tokens
		balances: BalancesConfig {
			balances: endowed_accounts
				.iter()
				.cloned()
				.map(|k| (k, 100_000_000 * CBC)) // Each gets 100M units
				.collect::<Vec<_>>(),
		},
		// Assign the sudo (root) key to the provided account
		sudo: SudoConfig { key: Some(root) },
		system: frame_system::GenesisConfig::default(),
		transaction_payment: pallet_transaction_payment::GenesisConfig::default(),
		dcf: pallet_cbc_dcf::GenesisConfig {
			validators: initial_validators.clone(),
			validator_scores: validator_scores.clone(),
			validator_stakes: stakes.clone(),
			current_epoch: 0,
			epoch_config: pallet_cbc_dcf::EpochConfig {
				blocks_per_epoch: 100,     // More realistic epoch length
				min_stake: 1_000_000,      // 1M minimum stake
				max_validators: 100,       // Support up to 100 validators
			},
			validator_names: names,     // Use provided names or defaults
			strict_validation: true,    // Enable strict validation
		},
		// Configure initial validators
		pallet_cbc_pos: pallet_cbc_pos::GenesisConfig {
			validators: initial_validators.clone(),
			validator_scores,
			current_epoch: 0,
			slashing_count: vec![],
		},
		// Configure initial inference results
		pallet_cbc_poi: pallet_cbc_poi::GenesisConfig {
			inference_results,
			challenges: vec![],
			current_epoch: 0,
		},
	})
}

/// Returns a basic development configuration suitable for running a single-node dev chain.
/// - Uses Alice as the sole validator and sudo.
/// - Endows Alice, Bob, and their stash accounts with tokens.
pub fn development_config_genesis() -> Value {
	let initial_validators = vec![Ed25519Keyring::Alice.to_account_id()];
	let endowed_accounts = vec![
		Ed25519Keyring::Alice.to_account_id(),
		Ed25519Keyring::Bob.to_account_id(),
	];
	testnet_genesis(
		initial_validators,
		endowed_accounts,
		Ed25519Keyring::Alice.to_account_id(),
	)
}

/// Returns a local testnet configuration:
/// - Alice and Bob are set as validators.
/// - Endows all keyring accounts (except One and Two) with tokens.
/// - Alice is the sudo key.
pub fn local_config_genesis() -> Value {
	let initial_validators = vec![
		Ed25519Keyring::Alice.to_account_id(),
		Ed25519Keyring::Bob.to_account_id(),
		Ed25519Keyring::Charlie.to_account_id(),
	];
	let endowed_accounts = Ed25519Keyring::iter()
		.filter(|v| v != &Ed25519Keyring::One && v != &Ed25519Keyring::Two)
		.map(|v| v.to_account_id())
		.collect::<Vec<_>>();
	testnet_genesis(
		initial_validators,
		endowed_accounts,
		Ed25519Keyring::Alice.to_account_id(),
	)
}

/// Fetches the JSON representation of the genesis config for the given `PresetId`.
/// - Supports "development", "local", "multi_validator", and "high_stake" presets.
/// - Returns None for unknown presets.
pub fn get_preset(id: &Option<PresetId>) -> Option<Vec<u8>> {
    match id.as_deref() {
        None => Some(serde_json::to_vec(&development_config_genesis()).unwrap()), // Default to dev
        Some("development") => Some(serde_json::to_vec(&development_config_genesis()).unwrap()),
        Some("local") => Some(serde_json::to_vec(&local_config_genesis()).unwrap()),
        Some("multi_validator") => Some(serde_json::to_vec(&multi_validator_config_genesis()).unwrap()),
        Some("high_stake") => Some(serde_json::to_vec(&high_stake_config_genesis()).unwrap()),
        Some("bob_sudo") => Some(serde_json::to_vec(&local_config_genesis()).unwrap()),
        Some("local_testnet") => Some(serde_json::to_vec(&local_config_genesis()).unwrap()),
        _ => None,
    }
}

/// Returns a multi-validator testnet configuration with custom stakes
pub fn multi_validator_config_genesis() -> Value {
	let initial_validators = vec![
		Ed25519Keyring::Alice.to_account_id(),
		Ed25519Keyring::Bob.to_account_id(),
		Ed25519Keyring::Charlie.to_account_id(),
		Ed25519Keyring::Dave.to_account_id(),
		Ed25519Keyring::Eve.to_account_id(),
	];

	// Custom stakes for different validator profiles
	let validator_stakes = vec![
		15_000_000, // Alice: High stake validator
		12_000_000, // Bob: Medium-high stake
		8_000_000,  // Charlie: Medium stake
		5_000_000,  // Dave: Low-medium stake
		3_000_000,  // Eve: Minimum viable stake
	];

	// Custom validator names for better identification
	let validator_names = vec![
		Some(b"Alice-Validator".to_vec()),
		Some(b"Bob-Validator".to_vec()),
		Some(b"Charlie-Validator".to_vec()),
		Some(b"Dave-Validator".to_vec()),
		Some(b"Eve-Validator".to_vec()),
	];

	let endowed_accounts = Ed25519Keyring::iter()
		.map(|v| v.to_account_id())
		.collect::<Vec<_>>();

	testnet_genesis_with_stakes_and_names(
		initial_validators,
		endowed_accounts,
		Ed25519Keyring::Alice.to_account_id(),
		Some(validator_stakes),
		Some(validator_names),
	)
}

/// Returns a high-stake validator configuration for stress testing
pub fn high_stake_config_genesis() -> Value {
	let initial_validators = vec![
		Ed25519Keyring::Alice.to_account_id(),
		Ed25519Keyring::Bob.to_account_id(),
		Ed25519Keyring::Charlie.to_account_id(),
	];

	// High stakes for all validators
	let validator_stakes = vec![
		50_000_000, // Alice: 50M units
		45_000_000, // Bob: 45M units
		40_000_000, // Charlie: 40M units
	];

	// High-stake validator names
	let validator_names = vec![
		Some(b"Alice-HighStake".to_vec()),
		Some(b"Bob-HighStake".to_vec()),
		Some(b"Charlie-HighStake".to_vec()),
	];

	let endowed_accounts = Ed25519Keyring::iter()
		.map(|v| v.to_account_id())
		.collect::<Vec<_>>();

	testnet_genesis_with_stakes_and_names(
		initial_validators,
		endowed_accounts,
		Ed25519Keyring::Alice.to_account_id(),
		Some(validator_stakes),
		Some(validator_names),
	)
}

/// Returns the list of preset names that are supported by this runtime.
/// These identifiers can be used when launching the chain with a specific genesis preset.
pub fn preset_names() -> Vec<PresetId> {
	vec![
		"development".into(),
		"local".into(),
		"multi_validator".into(),
		"high_stake".into(),
	]
}
