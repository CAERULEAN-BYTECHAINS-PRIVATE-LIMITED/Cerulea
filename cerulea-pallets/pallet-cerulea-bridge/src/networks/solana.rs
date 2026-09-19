//! The verifier for a Solana network.
//!
//! A Solana network settles a slot through votes that never travel with
//! the account state. A reader holding only headers therefore reaches no
//! account. Running the whole validator set inside a Blockchain runtime
//! stays out of reach on ordinary hardware. The endpoint takes the
//! attested header path instead, and states that choice in its record.
//!
//! # The trust assumption, stated plainly
//!
//! A threshold of relayers registered on the endpoint signs the slot
//! header. The chain accepts the bank hash of that slot once the threshold
//! signs. The slot leader signature is checked as well where the endpoint
//! registers the leader key. An account proof then runs against the bank
//! hash with real cryptography. The weak point is the relayer set. A
//! reader of the endpoint record sees the relayer count and the threshold,
//! so the strength of the evidence is visible.
//!
//! # The bank hash
//!
//! A slot carries a bank hash over the bank hash of the slot below, the
//! accounts delta hash of the slot, the count of signatures in the slot
//! and the last block hash. The verifier recomputes that hash and compares
//! it with the accepted header.
//!
//! # The account proof
//!
//! An account hashes its balance, its rent epoch, its data, its executable
//! marker, its owner and its own address. Those account hashes sit in a
//! tree grouping sixteen children under each parent. The root of that tree
//! is the accounts delta hash. The verifier folds the branch and compares
//! the result with the accounts delta hash the bank hash covers.

use crate::endpoint::{EndpointState, HeaderRecord, SolanaParameters, VerifierParameters};
use crate::envelope::{
    BridgeFault, MessageEnvelope, NetworkKind, TrustAssumption, VerifiedMessage,
};
use crate::merkle::{self, WideStep};
use crate::verify::{
    accepted_header, check_ed25519, KeyedSignature, Verifier, MAX_BRANCH, MAX_SIGNATURES,
    MAX_VALUE_BYTES,
};
use alloc::vec::Vec;
use codec::{Decode, DecodeWithMemTracking, Encode};
use frame_support::{pallet_prelude::ConstU32, BoundedVec};
use scale_info::TypeInfo;

/// The label the relayer set and the slot leader sign over.
pub const SLOT_DOMAIN: &[u8] = b"cerulea.bridge.solana.slot.v1";

/// The bank hash of one slot.
///
/// The hash covers the bank hash of the slot below, the accounts delta
/// hash of this slot, the count of signatures in this slot and the last
/// block hash. The count sits in the preimage as eight bytes with the low
/// byte first.
pub fn bank_hash(
    parent: &[u8; 32],
    accounts_delta: &[u8; 32],
    signature_count: u64,
    last_blockhash: &[u8; 32],
) -> [u8; 32] {
    let mut held = [0u8; 104];
    held[..32].copy_from_slice(parent);
    held[32..64].copy_from_slice(accounts_delta);
    held[64..72].copy_from_slice(&signature_count.to_le_bytes());
    held[72..].copy_from_slice(last_blockhash);
    sp_crypto_hashing::sha2_256(&held)
}

/// One account as the far network holds it.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo)]
pub struct AccountState {
    /// The address of the account.
    pub address: [u8; 32],
    /// The balance the account holds.
    pub lamports: u64,
    /// The epoch the account pays rent through.
    pub rent_epoch: u64,
    /// The program that owns the account.
    pub owner: [u8; 32],
    /// The account holds runnable code while this holds.
    pub executable: bool,
    /// The bytes the account holds.
    pub data: BoundedVec<u8, ConstU32<MAX_VALUE_BYTES>>,
}

/// The hash of one account.
///
/// The preimage carries the balance, the rent epoch, the data, the
/// executable marker, the owner and the address, in that order. The two
/// whole numbers sit in the preimage with the low byte first.
pub fn account_hash(account: &AccountState) -> [u8; 32] {
    let mut held: Vec<u8> = Vec::with_capacity(account.data.len() + 81);
    held.extend_from_slice(&account.lamports.to_le_bytes());
    held.extend_from_slice(&account.rent_epoch.to_le_bytes());
    held.extend_from_slice(account.data.as_slice());
    held.push(u8::from(account.executable));
    held.extend_from_slice(&account.owner);
    held.extend_from_slice(&account.address);
    sp_crypto_hashing::sha2_256(&held)
}

/// One level of an account branch.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo)]
pub struct AccountStep {
    /// The hashes of the children of the parent, in order.
    pub children: BoundedVec<[u8; 32], ConstU32<{ merkle::SOLANA_FANOUT as u32 }>>,
    /// The position the path takes among the children.
    pub position: u32,
}

/// One attested slot header.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo)]
pub struct SlotHeader {
    /// The slot the header covers.
    pub slot: u64,
    /// The bank hash of the slot below.
    pub parent_bank_hash: [u8; 32],
    /// The accounts delta hash of the slot.
    pub accounts_delta_hash: [u8; 32],
    /// The count of signatures in the slot.
    pub signature_count: u64,
    /// The last block hash of the slot.
    pub last_blockhash: [u8; 32],
}

impl SlotHeader {
    /// The bank hash the header folds to.
    pub fn bank_hash(&self) -> [u8; 32] {
        bank_hash(
            &self.parent_bank_hash,
            &self.accounts_delta_hash,
            self.signature_count,
            &self.last_blockhash,
        )
    }

    /// The bytes a relayer or a slot leader signs for this header.
    pub fn payload(&self) -> Vec<u8> {
        let mut held: Vec<u8> = Vec::with_capacity(SLOT_DOMAIN.len() + 40);
        held.extend_from_slice(SLOT_DOMAIN);
        held.extend_from_slice(&self.slot.to_be_bytes());
        held.extend_from_slice(&self.bank_hash());
        held
    }
}

/// One batch of attested slot headers.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo)]
pub struct SolanaHeaderBatch {
    /// The slot headers, the lowest slot first.
    pub headers: BoundedVec<SlotHeader, ConstU32<{ crate::verify::MAX_HEADERS }>>,
    /// The signature of the slot leader over the highest header.
    pub leader: Option<KeyedSignature>,
    /// The relayer signatures over the highest header.
    pub signatures: BoundedVec<KeyedSignature, ConstU32<MAX_SIGNATURES>>,
}

/// Check one batch of slot headers and answer with the headers to record.
pub fn accept_headers(
    params: &SolanaParameters,
    batch: &SolanaHeaderBatch,
    now_millis: u64,
) -> Result<Vec<HeaderRecord>, BridgeFault> {
    if batch.headers.is_empty() {
        return Err(BridgeFault::ChainBroken);
    }
    let mut found: Vec<HeaderRecord> = Vec::with_capacity(batch.headers.len());
    let mut previous: Option<SlotHeader> = None;
    for header in batch.headers.iter() {
        match &previous {
            Some(below) => {
                if header.parent_bank_hash != below.bank_hash() || header.slot <= below.slot {
                    return Err(BridgeFault::ChainBroken);
                }
            }
            None => {
                let trusted = params.checkpoint;
                if trusted.height > 0 && header.parent_bank_hash != trusted.hash {
                    return Err(BridgeFault::ChainBroken);
                }
            }
        }
        let signed = count_relayers(params, header, &batch.signatures)?;
        check_leader(params, header, &batch.leader)?;
        found.push(HeaderRecord {
            height: header.slot,
            hash: header.bank_hash(),
            state_root: header.bank_hash(),
            extra_root: header.accounts_delta_hash,
            at_millis: now_millis,
            trust: TrustAssumption::RelayerAttestation {
                signed,
                registered: params.relayers.len() as u32,
            },
        });
        previous = Some(header.clone());
    }
    Ok(found)
}

/// Count the registered relayers that signed one slot header.
fn count_relayers(
    params: &SolanaParameters,
    header: &SlotHeader,
    signatures: &[KeyedSignature],
) -> Result<u32, BridgeFault> {
    let payload = header.payload();
    let mut seen: Vec<[u8; 32]> = Vec::new();
    let mut signed = 0u32;
    for held in signatures.iter() {
        if !params.relayers.iter().any(|key| key == &held.key) {
            return Err(BridgeFault::SignerUnknown);
        }
        if seen.contains(&held.key) {
            return Err(BridgeFault::SignerRepeated);
        }
        seen.push(held.key);
        if !check_ed25519(&held.key, &payload, &held.signature) {
            return Err(BridgeFault::SignatureInvalid);
        }
        signed = signed.saturating_add(1);
    }
    if signed < params.relayer_threshold.max(1) {
        return Err(BridgeFault::ThresholdUnmet);
    }
    Ok(signed)
}

/// Check the slot leader signature where the endpoint registers leaders.
///
/// An endpoint registering no leader key skips this check and rests on the
/// relayer threshold alone.
fn check_leader(
    params: &SolanaParameters,
    header: &SlotHeader,
    leader: &Option<KeyedSignature>,
) -> Result<(), BridgeFault> {
    if params.leaders.is_empty() {
        return Ok(());
    }
    let held = leader.as_ref().ok_or(BridgeFault::ThresholdUnmet)?;
    if !params.leaders.iter().any(|key| key == &held.key) {
        return Err(BridgeFault::SignerUnknown);
    }
    if check_ed25519(&held.key, &header.payload(), &held.signature) {
        Ok(())
    } else {
        Err(BridgeFault::SignatureInvalid)
    }
}

/// The proof one message from a Solana network carries.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo)]
pub struct SolanaProof {
    /// The slot the message sits in.
    pub slot: u64,
    /// The account the program wrote the commitment into.
    pub account: AccountState,
    /// The branch from the account hash to the accounts delta hash.
    pub steps: BoundedVec<AccountStep, ConstU32<MAX_BRANCH>>,
}

/// The verifier of a Solana network.
pub struct SolanaVerifier;

impl Verifier for SolanaVerifier {
    type Proof = SolanaProof;

    fn kind() -> NetworkKind {
        NetworkKind::Solana
    }

    fn verify(
        envelope: &MessageEnvelope,
        proof: &SolanaProof,
        endpoint: &EndpointState<'_>,
    ) -> Result<VerifiedMessage, BridgeFault> {
        let params = match endpoint.params {
            VerifierParameters::Solana(held) => held,
            _ => return Err(BridgeFault::KindMismatch),
        };
        let header = accepted_header(endpoint, proof.slot)?;
        if proof.account.address != params.account {
            return Err(BridgeFault::CommitmentMismatch);
        }
        if proof.account.owner != params.program {
            return Err(BridgeFault::CommitmentMismatch);
        }

        let leaf = account_hash(&proof.account);
        let steps: Vec<WideStep> = proof
            .steps
            .iter()
            .map(|held| WideStep {
                children: held.children.to_vec(),
                position: held.position,
            })
            .collect();
        let folded = merkle::wide_fold(&leaf, &steps).ok_or(BridgeFault::NodeHashMismatch)?;
        if folded != header.extra_root {
            return Err(BridgeFault::NodeHashMismatch);
        }

        let id = envelope.id();
        let commitment = envelope.commitment();
        let held =
            read_commitment(proof.account.data.as_slice(), &id).ok_or(BridgeFault::ValueAbsent)?;
        if held != commitment {
            return Err(BridgeFault::CommitmentMismatch);
        }

        Ok(VerifiedMessage {
            envelope: envelope.clone(),
            id,
            height: proof.slot,
            block_hash: header.hash,
            trust: TrustAssumption::RelayerAttestation {
                signed: params.relayer_threshold,
                registered: params.relayers.len() as u32,
            },
        })
    }
}

/// Read the commitment one account holds for one message.
///
/// The program writes a list of records. Each record is the message
/// identifier followed by the commitment. The reader walks the list in
/// sixty four byte steps and answers with the commitment of the named
/// message.
pub fn read_commitment(data: &[u8], id: &[u8; 32]) -> Option<[u8; 32]> {
    let mut cursor = 0usize;
    while cursor + 64 <= data.len() {
        if &data[cursor..cursor + 32] == id.as_slice() {
            let mut found = [0u8; 32];
            found.copy_from_slice(&data[cursor + 32..cursor + 64]);
            return Some(found);
        }
        cursor += 64;
    }
    None
}

/// Write the account data one commitment record takes.
///
/// The bridge writes these bytes while it produces fixtures. A program on
/// the far network writes the same bytes.
pub fn write_commitment(id: &[u8; 32], commitment: &[u8; 32]) -> Vec<u8> {
    let mut found: Vec<u8> = Vec::with_capacity(64);
    found.extend_from_slice(id);
    found.extend_from_slice(commitment);
    found
}
