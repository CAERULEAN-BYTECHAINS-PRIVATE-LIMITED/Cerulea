use frame_support::parameter_types;
use sp_runtime::Perbill;

// Time and blocks
pub const MILLISECS_PER_BLOCK: u64 = 120000;
pub const SLOT_DURATION: u64 = MILLISECS_PER_BLOCK;
pub const EPOCH_DURATION_IN_BLOCKS: u32 = 100 * MINUTES;
pub const EPOCH_DURATION_IN_SLOTS: u32 = EPOCH_DURATION_IN_BLOCKS;

// These time units are defined in number of blocks.
pub const MINUTES: u32 = 60_000 / (MILLISECS_PER_BLOCK as u32);
pub const HOURS: u32 = MINUTES * 60;
pub const DAYS: u32 = HOURS * 24;

// Fee related
pub const EXISTENTIAL_DEPOSIT: u128 = 500;
pub const MAXIMUM_BLOCK_WEIGHT: u32 = 2 * 1024 * 1024;
pub const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75);

// Consensus related
pub const MIN_VALIDATOR_SCORE: u32 = 50;
pub const MIN_ACTIVE_VALIDATORS: u32 = 4;
pub const MAX_VALIDATORS: u32 = 100;
pub const VALIDATOR_SCORE_DECAY: u32 = 10;
pub const MAX_SLASHING_COUNT: u32 = 3;

// Inference related
pub const MIN_INFERENCE_CONFIDENCE: u32 = 70;
pub const MAX_INFERENCE_AGE: u32 = 24 * HOURS;
pub const CHALLENGE_WINDOW: u32 = 1 * HOURS;
pub const INFERENCE_REWARD: u128 = 100;
pub const CHALLENGE_REWARD: u128 = 50; 