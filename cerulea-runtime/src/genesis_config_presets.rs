// This file is part of the Cerulea Runtime.
// It defines functions for generating JSON-based genesis settings used by the blockchain node
// when starting a development or local testnet chain.

use crate::{AccountId, BalancesConfig, RuntimeGenesisConfig, SudoConfig, CERULEA}; // Runtime-specific types
use cerulea_blueprint::{Blueprint, BlueprintMeta};
use codec::{Decode, Encode};
use frame_support::BoundedVec;
use pallet_cerulea_schema::primitives::{name_of, DeploymentId};
use pallet_cerulea_tier::{Application, Licence, Tier};
use alloc::{vec, vec::Vec}; // Alloc crate for dynamic arrays
use frame_support::build_struct_json_patch; // Macro to build partial JSON patches for genesis config
use serde_json::Value; // JSON value type
use sp_genesis_builder::{self, PresetId}; // Genesis builder utilities and PresetId for pre-defined configs
use sp_keyring::{Ed25519Keyring, Sr25519Keyring}; // Keyring to easily access dev accounts

/// The Blueprint every Cerulea chain starts with.
///
/// The Blueprint carries the header alone. The header names the
/// deployment, states what the deployment does and names its owner. Every
/// section beside the header stays empty, so the first deployment of a
/// fresh chain holds no customer data.
pub fn starting_blueprint() -> Blueprint {
    Blueprint::new(BlueprintMeta::new(
        "Cerulea deployment",
        "The deployment a fresh Cerulea chain starts with.",
        "Caerulean Bytechains Private Limited",
    ))
}

/// The deployment identifier of the starting Blueprint.
///
/// The identifier is the content address of the Blueprint, so every chain
/// built from the same Blueprint names the same deployment.
pub fn starting_deployment() -> DeploymentId {
    starting_blueprint().id().0
}

/// The account holding the Cerulea operator role on a development chain.
///
/// Every customer deployment carries the role. The role upgrades the
/// platform version, verifies the licence and stops block production on a
/// lapse. The role reaches no customer record.
pub fn cerulea_operator() -> AccountId {
    Ed25519Keyring::Ferdie.to_account_id()
}

/// The signature the control plane produced over the development licence.
///
/// Signing needs a private key, which the Blockchain runtime holds nowhere.
/// The control plane signs once and the chain specification carries the
/// result. The test `the_development_licence_signature_holds` rebuilds the
/// message and checks these bytes against the control plane key.
pub const DEVELOPMENT_LICENCE_SIGNATURE: [u8; 64] = [
    23, 48, 166, 107, 173, 113, 14, 51,
    120, 167, 28, 95, 144, 118, 167, 230,
    127, 20, 89, 233, 247, 56, 29, 169,
    93, 32, 124, 121, 251, 37, 104, 89,
    125, 58, 80, 155, 229, 1, 113, 89,
    129, 196, 179, 228, 89, 163, 96, 79,
    111, 160, 87, 168, 106, 245, 103, 182,
    238, 86, 107, 238, 210, 102, 45, 12,
];

/// The licence the starting deployment runs under, with its signature.
///
/// The control plane signs every licence with an ed25519 key. A
/// development chain takes the well known Alice key as its control plane
/// key, so the chain verifies its own licence from the first block.
fn starting_licence() -> (DeploymentId, Vec<u8>, Vec<u8>, [u8; 32]) {
    let deployment = starting_deployment();
    let features: Vec<_> = [
        "schema",
        "state",
        "collections",
        "identity",
        "permission",
        "logic",
        "workflow",
        "rules",
        "governance",
        "evolution",
        "oracle",
        "relayer",
        "tier",
        "audit",
        "operator",
        "custody",
        pallet_cerulea_relayer::RELAY_FEATURE,
    ]
    .iter()
    .filter_map(|held| name_of(held).ok())
    .collect();
    let licence: Licence<
        <crate::Runtime as pallet_cerulea_tier::Config>::MaxFeatures,
        <crate::Runtime as pallet_cerulea_tier::Config>::MaxAddons,
    > = Licence {
        deployment,
        tier: Tier {
            application: Application::Enterprise,
            level: 1,
        },
        features: BoundedVec::truncate_from(features),
        seats: 64,
        addons: BoundedVec::truncate_from(Vec::new()),
        issued_at: 0,
        // The development licence covers one hundred years of wall clock
        // time, so a development chain never stops producing blocks while
        // a person works with it.
        expires_at: 3_155_760_000_000,
        sequence: 1,
    };
    (
        deployment,
        licence.encode(),
        DEVELOPMENT_LICENCE_SIGNATURE.to_vec(),
        Ed25519Keyring::Alice.public().0,
    )
}

/// The licence message the development signature covers.
///
/// A test in this crate rebuilds the message and checks the signature
/// against the control plane key, so the constant above stays true.
pub fn starting_licence_message() -> Vec<u8> {
    let (_, attestation, _, _) = starting_licence();
    let licence = LicenceOf::decode(&mut attestation.as_slice())
        .expect("the licence the preset builds decodes again");
    licence.message()
}

/// The licence the starting deployment runs under, as the tier module
/// reads it.
type LicenceOf = Licence<
    <crate::Runtime as pallet_cerulea_tier::Config>::MaxFeatures,
    <crate::Runtime as pallet_cerulea_tier::Config>::MaxAddons,
>;

/// Returns a genesis settings in JSON format with the specified authorities,
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

/// Returns a genesis settings with custom validator stakes
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

/// Returns a genesis settings with custom validator stakes and names
fn testnet_genesis_with_stakes_and_names(
	initial_validators: Vec<AccountId>, // Validator accounts
	endowed_accounts: Vec<AccountId>,   // Accounts pre-funded with balance
	root: AccountId,                    // Root (sudo) key
	validator_stakes: Option<Vec<u128>>, // Optional custom stakes
	validator_names: Option<Vec<Option<Vec<u8>>>>, // Optional validator names
) -> Value {
	// Create initial validator scores
	let validator_scores = vec![80; initial_validators.len()]; // 80% initial score

	// Create validator stakes - use provided stakes or defaults in units
	let stakes = validator_stakes.unwrap_or_else(|| {
		// Default stakes: 10M, 8M, 6M, 5M, 3M units
		initial_validators.iter().enumerate().map(|(i, _)| {
			match i {
				0 => 10_000_000 * CERULEA, // Alice: 10M units
				1 => 8_000_000 * CERULEA,  // Bob: 8M units
				2 => 6_000_000 * CERULEA,  // Charlie: 6M units
				3 => 5_000_000 * CERULEA,  // Dave: 5M units
				_ => 3_000_000 * CERULEA,  // Others: 3M units
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

	// The Authoring Engine reads a draw key and a seal key for each validator.
	// The seal key of a development validator is the key its account came from.
	// The draw key is the sr25519 key of the same well known name.
	let validator_keys: Vec<(AccountId, pallet_cerulea_dvf::keys::ValidatorKeySet)> =
		initial_validators
			.iter()
			.filter_map(|account| {
				let seal: [u8; 32] = *account.as_ref();
				Ed25519Keyring::iter()
					.zip(Sr25519Keyring::iter())
					.find(|(sealer, _)| sealer.to_account_id() == *account)
					.map(|(_, drawer)| {
						(
							account.clone(),
							pallet_cerulea_dvf::keys::ValidatorKeySet::new(
								drawer.public().0,
								seal,
							),
						)
					})
			})
			.collect();

	// Create Registry initial weights synchronized with DCF validators
	// This ensures Registry voting weights match DCF validator data at genesis
	let dvf_initial_weights: Vec<(AccountId, u128, u128)> = initial_validators
		.iter()
		.zip(stakes.iter())
		.zip(validator_scores.iter())
		.map(|((validator, stake), score)| (validator.clone(), *stake, *score as u128))
		.collect();

	let (deployment, attestation, signature, control_plane_key) = starting_licence();

	build_struct_json_patch!(RuntimeGenesisConfig {
		// Set initial balances for all endowed accounts with large amounts of tokens
		balances: BalancesConfig {
			balances: endowed_accounts
				.iter()
				.cloned()
				.map(|k| (k, 100_000_000 * CERULEA)) // Each gets 100M units
				.collect::<Vec<_>>(),
		},
		// Assign the sudo (root) key to the provided account
		sudo: SudoConfig { key: Some(root) },
		system: frame_system::GenesisConfig::default(),
		transaction_payment: pallet_transaction_payment::GenesisConfig::default(),
		dcf: pallet_cerulea_dcf::GenesisConfig {
			validators: initial_validators.clone(),
			validator_scores: validator_scores.clone(),
			validator_stakes: stakes.clone(),
			current_epoch: 0,
			epoch_config: pallet_cerulea_dcf::EpochConfig {
				blocks_per_epoch: 100,     // More realistic epoch length
				min_stake: 1_000_000,      // 1M minimum stake
				max_validators: 100,       // Support up to 100 validators
			},
			validator_names: names,     // Use provided names or defaults
			strict_validation: true,    // Enable strict validation
		},
		// Set initial validators
		pos: pallet_cerulea_pos::GenesisConfig {
			validators: initial_validators.clone(),
			validator_scores,
			current_epoch: 0,
			slashing_count: vec![],
		},
		// Set initial inference results
		poi: pallet_cerulea_poi::GenesisConfig {
			inference_results,
			challenges: vec![],
			current_epoch: 0,
		},
		// Set Registry initial validator weights
		// Synchronized with DCF validators to ensure consistency between
		// validator set membership and voting weights at genesis
		registry: pallet_cerulea_dvf::GenesisConfig {
			initial_validator_weights: dvf_initial_weights,
			initial_validator_keys: validator_keys,
			// A production chain spec names its genesis validators, so
			// genesis stops on a spec that names none.
			strict_validation: true,
		},
		// Install the starting Blueprint. The logic module validates it,
		// registers every type and every collection it declares and stores
		// it under its content address.
		logic: pallet_cerulea_logic::GenesisConfig {
			blueprints: vec![starting_blueprint().encode()],
		},
		// Install the control plane key and the licence the deployment
		// starts under. The tier module checks the signature during the
		// genesis build, so a chain starts under a verified licence.
		tier: pallet_cerulea_tier::GenesisConfig {
			control_plane_keys: vec![(deployment, control_plane_key)],
			licences: vec![(deployment, attestation, signature)],
		},
		// Appoint the Cerulea operator role. The role is mandatory in
		// every deployment and strictly limited.
		operator: pallet_cerulea_operator::GenesisConfig {
			operators: vec![(deployment, cerulea_operator())],
		},
	})
}

/// Returns a basic development settings suitable for running a single-node dev chain.
/// - Uses Alice as the sole validator and sudo.
/// - Endows Alice, Bob, and their stash accounts with tokens.
pub fn development_config_genesis() -> Value {
	let initial_validators = vec![Ed25519Keyring::Alice.to_account_id()];
	let endowed_accounts = vec![
		Ed25519Keyring::Alice.to_account_id(),
		Ed25519Keyring::Bob.to_account_id(),
		cerulea_operator(),
	];
	testnet_genesis(
		initial_validators,
		endowed_accounts,
		Ed25519Keyring::Alice.to_account_id(),
	)
}

/// Returns a local testnet settings:
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

/// Returns a multi-validator testnet settings with custom stakes
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

/// Returns a high-stake validator settings for stress testing
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
