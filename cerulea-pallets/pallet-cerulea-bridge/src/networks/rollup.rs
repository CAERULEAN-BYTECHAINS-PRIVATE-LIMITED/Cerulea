//! The verifier for a Layer 2 rollup.
//!
//! A rollup runs its own state and commits the root of that state to a
//! settlement layer. The settlement layer is a network running Ethereum
//! Virtual Machine bytecode. A reader therefore proves the rollup state in
//! two stages. The first stage proves the output root on the settlement
//! layer through the verifier of that network. The second stage proves the
//! message inside the rollup state the output root covers.
//!
//! # The output root
//!
//! The output root is the hash of four words. The first word names the
//! shape of the commitment. The second word is the state trie root of the
//! rollup. The third word is the storage root of the withdrawal account.
//! The fourth word is the hash of the rollup block. The verifier rebuilds
//! that hash and compares it with the word the settlement layer holds.
//!
//! # The challenge window
//!
//! A rollup that settles through challenge publishes an output root and
//! waits. Anyone may dispute the root inside the window. The endpoint
//! records the window. The verifier reads the moment the output root
//! landed from the settlement layer storage and declines a proof reaching
//! a root whose window stays open.
//!
//! # The two storage proofs
//!
//! The first proof walks the settlement layer state trie to the output
//! contract, then walks the storage trie of that contract to the slot
//! holding the output root and to the slot holding the moment. The second
//! proof walks the rollup state trie to the message contract, then walks
//! the storage trie of that contract to the slot holding the commitment.

use crate::endpoint::{EndpointState, RollupParameters, VerifierParameters};
use crate::envelope::{
    BridgeFault, MessageEnvelope, NetworkKind, TrustAssumption, VerifiedMessage,
};
use crate::mpt;
use crate::networks::evm;
use crate::verify::{accepted_header, confirmations_for, raw_nodes, ProofNodes, Verifier};
use alloc::vec::Vec;
use codec::{Decode, DecodeWithMemTracking, Encode};
use scale_info::TypeInfo;

/// The word naming the shape of the commitment.
pub const OUTPUT_VERSION: [u8; 32] = [0u8; 32];

/// The hash of one output root.
pub fn output_root(
    version: &[u8; 32],
    state_root: &[u8; 32],
    withdrawal_root: &[u8; 32],
    block_hash: &[u8; 32],
) -> [u8; 32] {
    let mut held = [0u8; 128];
    held[..32].copy_from_slice(version);
    held[32..64].copy_from_slice(state_root);
    held[64..96].copy_from_slice(withdrawal_root);
    held[96..].copy_from_slice(block_hash);
    sp_crypto_hashing::keccak_256(&held)
}

/// The slot one output record sits at.
///
/// The contract keeps the output records in an array. The record at one
/// position starts at the hash of the base slot plus twice the position.
/// The first word holds the root. The second word holds the moment.
pub fn output_slot(base: &[u8; 32], index: u64) -> [u8; 32] {
    let start = sp_crypto_hashing::keccak_256(base);
    add_word(&start, index.saturating_mul(2))
}

/// Add a whole number to a thirty two byte word.
pub fn add_word(word: &[u8; 32], value: u64) -> [u8; 32] {
    let mut found = *word;
    let mut carry = value as u128;
    let mut position = 32usize;
    while position > 0 && carry > 0 {
        position -= 1;
        let sum = found[position] as u128 + (carry & 0xff);
        found[position] = (sum & 0xff) as u8;
        carry = (carry >> 8) + (sum >> 8);
    }
    found
}

/// Read a whole number from the low bytes of one word.
pub fn word_number(word: &[u8; 32]) -> u64 {
    let mut found = 0u64;
    for byte in word[24..].iter() {
        found = (found << 8) | (*byte as u64);
    }
    found
}

/// The proof one message from a Layer 2 rollup carries.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo)]
pub struct RollupProof {
    /// The block height on the settlement layer the proof reaches.
    pub settlement_height: u64,
    /// The position of the output record on the settlement layer.
    pub output_index: u64,
    /// The account trie nodes from the settlement state root to the
    /// output contract.
    pub output_account: ProofNodes,
    /// The storage trie nodes reaching the slot holding the output root.
    pub output_storage: ProofNodes,
    /// The storage trie nodes reaching the slot holding the moment.
    pub moment_storage: ProofNodes,
    /// The word naming the shape of the commitment.
    pub version: [u8; 32],
    /// The state trie root of the rollup.
    pub l2_state_root: [u8; 32],
    /// The storage root of the withdrawal account of the rollup.
    pub withdrawal_root: [u8; 32],
    /// The hash of the rollup block.
    pub l2_block_hash: [u8; 32],
    /// The account trie nodes from the rollup state root to the message
    /// contract.
    pub l2_account: ProofNodes,
    /// The storage trie nodes reaching the slot holding the commitment.
    pub l2_storage: ProofNodes,
}

/// The verifier of a Layer 2 rollup.
pub struct RollupVerifier;

impl Verifier for RollupVerifier {
    type Proof = RollupProof;

    fn kind() -> NetworkKind {
        NetworkKind::Rollup
    }

    fn verify(
        envelope: &MessageEnvelope,
        proof: &RollupProof,
        endpoint: &EndpointState<'_>,
    ) -> Result<VerifiedMessage, BridgeFault> {
        let params = match endpoint.params {
            VerifierParameters::Rollup(held) => held,
            _ => return Err(BridgeFault::KindMismatch),
        };
        let header = accepted_header(endpoint, proof.settlement_height)?;
        confirmations_for(
            endpoint,
            proof.settlement_height,
            params.settlement.confirmations,
        )?;

        let storage_root = evm::storage_root_of(
            &header.state_root,
            &params.output_contract,
            &proof.output_account,
        )?;
        let base = output_slot(&params.output_slot, proof.output_index);
        let root_key = sp_crypto_hashing::keccak_256(&base);
        let encoded = mpt::require(&storage_root, &root_key, &raw_nodes(&proof.output_storage))?;
        let committed = mpt::word_of(&encoded)?;

        let expected = output_root(
            &proof.version,
            &proof.l2_state_root,
            &proof.withdrawal_root,
            &proof.l2_block_hash,
        );
        if committed != expected {
            return Err(BridgeFault::CommitmentMismatch);
        }

        let moment_slot = add_word(&base, 1);
        let moment_key = sp_crypto_hashing::keccak_256(&moment_slot);
        let encoded = mpt::require(
            &storage_root,
            &moment_key,
            &raw_nodes(&proof.moment_storage),
        )?;
        let moment = word_number(&mpt::word_of(&encoded)?);
        check_window(params, moment, endpoint.now_millis)?;

        let id = envelope.id();
        let commitment = envelope.commitment();
        evm::check_storage(
            &rollup_settings(params),
            &crate::endpoint::HeaderRecord {
                height: 0,
                hash: proof.l2_block_hash,
                state_root: proof.l2_state_root,
                extra_root: proof.l2_state_root,
                at_millis: 0,
                trust: TrustAssumption::NetworkCryptography,
            },
            &proof.l2_account,
            &proof.l2_storage,
            &id,
            &commitment,
        )?;

        Ok(VerifiedMessage {
            envelope: envelope.clone(),
            id,
            height: proof.settlement_height,
            block_hash: header.hash,
            trust: TrustAssumption::SettledAfterChallenge {
                window_millis: params.challenge_window_millis,
            },
        })
    }
}

/// The settings the second stage reads.
///
/// The second stage walks the rollup state with the contract and the slot
/// the rollup endpoint holds. The settlement settings stay out of the
/// second stage.
fn rollup_settings(params: &RollupParameters) -> crate::endpoint::EvmParameters {
    crate::endpoint::EvmParameters {
        chain_id: params.settlement.chain_id,
        checkpoint: params.settlement.checkpoint,
        contract: params.contract,
        topic: params.settlement.topic,
        slot: params.slot,
        confirmations: 0,
        header_signers: params.settlement.header_signers.clone(),
        header_threshold: params.settlement.header_threshold,
    }
}

/// Check that the challenge window of one output root passed.
///
/// The moment sits on the settlement layer in whole seconds. The endpoint
/// holds the window in milliseconds.
pub fn check_window(
    params: &RollupParameters,
    moment_seconds: u64,
    now_millis: u64,
) -> Result<(), BridgeFault> {
    let landed = moment_seconds.saturating_mul(1_000);
    let opens_at = landed.saturating_add(params.challenge_window_millis);
    if now_millis >= opens_at {
        Ok(())
    } else {
        Err(BridgeFault::ChallengeWindowOpen)
    }
}

/// Build the four words one output root covers.
///
/// The bridge uses the builder while it produces fixtures.
pub fn output_preimage(
    version: &[u8; 32],
    state_root: &[u8; 32],
    withdrawal_root: &[u8; 32],
    block_hash: &[u8; 32],
) -> Vec<u8> {
    let mut found: Vec<u8> = Vec::with_capacity(128);
    found.extend_from_slice(version);
    found.extend_from_slice(state_root);
    found.extend_from_slice(withdrawal_root);
    found.extend_from_slice(block_hash);
    found
}
