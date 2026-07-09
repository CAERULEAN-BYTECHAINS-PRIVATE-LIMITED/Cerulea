use frame_support::weights::Weight;

// System weights
pub const SYSTEM_WEIGHT: Weight = Weight::from_parts(10_000, 0);
pub const BALANCES_WEIGHT: Weight = Weight::from_parts(10_000, 0);
pub const TIMESTAMP_WEIGHT: Weight = Weight::from_parts(10_000, 0);
pub const SUDO_WEIGHT: Weight = Weight::from_parts(10_000, 0);

// Custom pallet weights
pub const CBC_DCF_WEIGHT: Weight = Weight::from_parts(50_000, 0);
pub const CBC_POI_WEIGHT: Weight = Weight::from_parts(20_000, 0);
pub const CBC_POS_WEIGHT: Weight = Weight::from_parts(15_000, 0);

// DCF-specific operation weights
pub const DCF_JOIN_VALIDATORS_WEIGHT: Weight = Weight::from_parts(50_000, 0);
pub const DCF_LEAVE_VALIDATORS_WEIGHT: Weight = Weight::from_parts(40_000, 0);
pub const DCF_SLASH_VALIDATOR_WEIGHT: Weight = Weight::from_parts(45_000, 0);
pub const DCF_EPOCH_TRANSITION_WEIGHT: Weight = Weight::from_parts(120_000, 0);
pub const DCF_PROPOSE_REWARD_WEIGHT: Weight = Weight::from_parts(35_000, 0);
pub const DCF_EXECUTE_PROPOSAL_WEIGHT: Weight = Weight::from_parts(75_000, 0);

// PoS-specific operation weights
pub const POS_REGISTER_VALIDATOR_WEIGHT: Weight = Weight::from_parts(10_000, 0);
pub const POS_SUBMIT_SCORE_WEIGHT: Weight = Weight::from_parts(15_000, 0);
pub const POS_SLASH_VALIDATOR_WEIGHT: Weight = Weight::from_parts(20_000, 0);

// PoI-specific operation weights
pub const POI_SUBMIT_INFERENCE_WEIGHT: Weight = Weight::from_parts(20_000, 0);
pub const POI_CHALLENGE_INFERENCE_WEIGHT: Weight = Weight::from_parts(30_000, 0);
pub const POI_RESOLVE_CHALLENGE_WEIGHT: Weight = Weight::from_parts(25_000, 0); 