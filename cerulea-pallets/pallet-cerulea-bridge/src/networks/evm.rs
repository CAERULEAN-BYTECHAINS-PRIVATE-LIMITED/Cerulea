//! The verifier for a network running Ethereum Virtual Machine bytecode.
//!
//! The far network commits to a message in two places. A contract writes
//! the commitment into its storage. The same contract emits a log, and the
//! transaction receipt carries that log. Either place proves the message.
//!
//! # The storage path
//!
//! 1. The proof names the block height. The endpoint looks the height up
//!    in the headers it accepted and reads the state trie root.
//! 2. The account trie proof walks from the state root to the account of
//!    the contract. The key is the hash of the twenty byte address.
//! 3. The account value carries the storage trie root of the contract.
//! 4. The storage trie proof walks from the storage root to the slot the
//!    message sits at. The slot follows the mapping rule of the virtual
//!    machine, which places an entry at the hash of the key word followed
//!    by the base slot word.
//! 5. The stored word equals the commitment the envelope carries.
//!
//! # The receipt path
//!
//! 1. The proof names the block height and the position of the
//!    transaction. The endpoint reads the receipts trie root of that
//!    height.
//! 2. The receipts trie proof walks from the receipts root to the receipt
//!    of that position. The key is the encoding of the position.
//! 3. The receipt carries the logs. The named log sits at the contract the
//!    endpoint holds, carries the topic the endpoint holds, carries the
//!    message identifier as its second topic and carries the commitment in
//!    its data.
//!
//! # The header chain
//!
//! A header feed submits headers above the trusted checkpoint. Each header
//! links to the previous header through its parent hash. A threshold of
//! registered feed keys signs the header hash. The endpoint then records
//! the state root and the receipts root of that header and moves the
//! checkpoint forward.

use crate::endpoint::{EndpointState, HeaderRecord, VerifierParameters};
use crate::envelope::{
    BridgeFault, MessageEnvelope, NetworkKind, TrustAssumption, VerifiedMessage,
};
use crate::mpt;
use crate::rlp;
use crate::verify::{
    accepted_header, check_ed25519, confirmations_for, raw_nodes, KeyedSignature, ProofNodes,
    RawHeader, Verifier,
};
use alloc::{vec, vec::Vec};
use codec::{Decode, DecodeWithMemTracking, Encode};
use frame_support::{pallet_prelude::ConstU32, BoundedVec};
use scale_info::TypeInfo;

/// The position of the parent hash in the header field list.
pub const FIELD_PARENT: usize = 0;

/// The position of the state root in the header field list.
pub const FIELD_STATE_ROOT: usize = 3;

/// The position of the receipts root in the header field list.
pub const FIELD_RECEIPTS_ROOT: usize = 5;

/// The position of the block number in the header field list.
pub const FIELD_NUMBER: usize = 8;

/// The smallest number of fields a header holds.
pub const MIN_HEADER_FIELDS: usize = 15;

/// The label the header feed signs over.
pub const FEED_DOMAIN: &[u8] = b"cerulea.bridge.evm.header.v1";

/// The fields one header carries.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EvmHeader {
    /// The hash of the header.
    pub hash: [u8; 32],
    /// The hash of the header below.
    pub parent: [u8; 32],
    /// The root of the state trie at this height.
    pub state_root: [u8; 32],
    /// The root of the receipts trie at this height.
    pub receipts_root: [u8; 32],
    /// The block number.
    pub number: u64,
}

/// Read the fields of one encoded header.
///
/// The hash of the header is the hash of the encoded bytes as they stand.
/// Two nodes therefore agree on the hash of the same header.
pub fn read_header(encoded: &[u8]) -> Result<EvmHeader, BridgeFault> {
    let item = rlp::decode(encoded).map_err(|_| BridgeFault::MalformedEncoding)?;
    let fields = item.list().map_err(|_| BridgeFault::MalformedEncoding)?;
    if fields.len() < MIN_HEADER_FIELDS {
        return Err(BridgeFault::MalformedEncoding);
    }
    Ok(EvmHeader {
        hash: sp_crypto_hashing::keccak_256(encoded),
        parent: item
            .hash_at(FIELD_PARENT)
            .map_err(|_| BridgeFault::MalformedEncoding)?,
        state_root: item
            .hash_at(FIELD_STATE_ROOT)
            .map_err(|_| BridgeFault::MalformedEncoding)?,
        receipts_root: item
            .hash_at(FIELD_RECEIPTS_ROOT)
            .map_err(|_| BridgeFault::MalformedEncoding)?,
        number: item
            .number_at(FIELD_NUMBER)
            .map_err(|_| BridgeFault::MalformedEncoding)?,
    })
}

/// The bytes the header feed signs for one header.
pub fn feed_payload(chain_id: u64, header: &EvmHeader) -> Vec<u8> {
    let mut held: Vec<u8> = Vec::with_capacity(FEED_DOMAIN.len() + 48);
    held.extend_from_slice(FEED_DOMAIN);
    held.extend_from_slice(&chain_id.to_be_bytes());
    held.extend_from_slice(&header.number.to_be_bytes());
    held.extend_from_slice(&header.hash);
    held
}

/// One batch of headers the feed submits.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo)]
pub struct EvmHeaderBatch {
    /// The encoded headers, lowest first.
    pub headers: BoundedVec<RawHeader, ConstU32<{ crate::verify::MAX_HEADERS }>>,
    /// The feed signatures over the highest header of the batch.
    pub signatures: BoundedVec<KeyedSignature, ConstU32<{ crate::verify::MAX_SIGNATURES }>>,
}

/// Check one header batch and answer with the headers to record.
///
/// The lowest header of the batch links to the trusted checkpoint. Every
/// later header links to the header below. A threshold of registered feed
/// keys signs each header of the batch.
pub fn accept_headers(
    params: &crate::endpoint::EvmParameters,
    batch: &EvmHeaderBatch,
    now_millis: u64,
) -> Result<Vec<HeaderRecord>, BridgeFault> {
    if batch.headers.is_empty() {
        return Err(BridgeFault::ChainBroken);
    }
    let mut found: Vec<HeaderRecord> = Vec::with_capacity(batch.headers.len());
    let mut previous: Option<EvmHeader> = None;
    for encoded in batch.headers.iter() {
        let header = read_header(encoded)?;
        match previous {
            Some(below) => {
                if header.parent != below.hash || header.number != below.number.saturating_add(1) {
                    return Err(BridgeFault::ChainBroken);
                }
            }
            None => {
                let trusted = params.checkpoint;
                let declared = trusted.height > 0 || trusted.hash != [0u8; 32];
                let linked = header.parent == trusted.hash
                    && header.number == trusted.height.saturating_add(1);
                if declared && !linked {
                    return Err(BridgeFault::ChainBroken);
                }
            }
        }
        let signed = count_feed_signatures(params, &header, &batch.signatures)?;
        found.push(HeaderRecord {
            height: header.number,
            hash: header.hash,
            state_root: header.state_root,
            extra_root: header.receipts_root,
            at_millis: now_millis,
            trust: TrustAssumption::RelayerAttestation {
                signed,
                registered: params.header_signers.len() as u32,
            },
        });
        previous = Some(header);
    }
    Ok(found)
}

/// Count the registered feed keys that signed one header.
fn count_feed_signatures(
    params: &crate::endpoint::EvmParameters,
    header: &EvmHeader,
    signatures: &[KeyedSignature],
) -> Result<u32, BridgeFault> {
    let payload = feed_payload(params.chain_id, header);
    let mut seen: Vec<[u8; 32]> = Vec::new();
    let mut signed = 0u32;
    for held in signatures.iter() {
        if !params.header_signers.iter().any(|key| key == &held.key) {
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
    if signed < params.header_threshold.max(1) {
        return Err(BridgeFault::ThresholdUnmet);
    }
    Ok(signed)
}

/// The proof one message from such a network carries.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo)]
pub enum EvmProof {
    /// The contract storage holds the commitment.
    Storage {
        /// The block height the proof reaches.
        height: u64,
        /// The account trie nodes from the state root to the contract.
        account: ProofNodes,
        /// The storage trie nodes from the storage root to the slot.
        storage: ProofNodes,
    },
    /// A transaction receipt carries the commitment in a log.
    Receipt {
        /// The block height the proof reaches.
        height: u64,
        /// The position of the transaction in the block.
        index: u64,
        /// The receipts trie nodes from the receipts root to the receipt.
        nodes: ProofNodes,
        /// The position of the log inside the receipt.
        log_index: u32,
    },
}

impl EvmProof {
    /// The block height the proof reaches.
    pub fn height(&self) -> u64 {
        match self {
            EvmProof::Storage { height, .. } => *height,
            EvmProof::Receipt { height, .. } => *height,
        }
    }
}

/// The verifier of a network running Ethereum Virtual Machine bytecode.
pub struct EvmVerifier;

impl Verifier for EvmVerifier {
    type Proof = EvmProof;

    fn kind() -> NetworkKind {
        NetworkKind::EthereumVirtualMachine
    }

    fn verify(
        envelope: &MessageEnvelope,
        proof: &EvmProof,
        endpoint: &EndpointState<'_>,
    ) -> Result<VerifiedMessage, BridgeFault> {
        let params = match endpoint.params {
            VerifierParameters::EthereumVirtualMachine(held) => held,
            _ => return Err(BridgeFault::KindMismatch),
        };
        let header = accepted_header(endpoint, proof.height())?;
        confirmations_for(endpoint, proof.height(), params.confirmations)?;
        let commitment = envelope.commitment();
        let id = envelope.id();

        match proof {
            EvmProof::Storage {
                account, storage, ..
            } => {
                check_storage(params, &header, account, storage, &id, &commitment)?;
            }
            EvmProof::Receipt {
                index,
                nodes,
                log_index,
                ..
            } => {
                check_receipt(params, &header, *index, nodes, *log_index, &id, &commitment)?;
            }
        }

        Ok(VerifiedMessage {
            envelope: envelope.clone(),
            id,
            height: proof.height(),
            block_hash: header.hash,
            trust: TrustAssumption::RelayerAttestation {
                signed: params.header_threshold,
                registered: params.header_signers.len() as u32,
            },
        })
    }
}

/// Walk the account trie and the storage trie to the commitment.
pub fn check_storage(
    params: &crate::endpoint::EvmParameters,
    header: &HeaderRecord,
    account: &ProofNodes,
    storage: &ProofNodes,
    id: &[u8; 32],
    commitment: &[u8; 32],
) -> Result<(), BridgeFault> {
    let storage_root = storage_root_of(&header.state_root, &params.contract, account)?;
    let slot = mpt::mapping_slot(id, &params.slot);
    let key = sp_crypto_hashing::keccak_256(&slot);
    let encoded = mpt::require(&storage_root, &key, &raw_nodes(storage))?;
    let word = mpt::word_of(&encoded)?;
    if &word != commitment {
        return Err(BridgeFault::CommitmentMismatch);
    }
    Ok(())
}

/// Read the storage trie root of one contract from the state trie.
pub fn storage_root_of(
    state_root: &[u8; 32],
    contract: &[u8; 20],
    account: &ProofNodes,
) -> Result<[u8; 32], BridgeFault> {
    let key = sp_crypto_hashing::keccak_256(contract);
    let encoded = mpt::require(state_root, &key, &raw_nodes(account))?;
    Ok(mpt::account_of(&encoded)?.storage_root)
}

/// Walk the receipts trie to the log carrying the commitment.
pub fn check_receipt(
    params: &crate::endpoint::EvmParameters,
    header: &HeaderRecord,
    index: u64,
    nodes: &ProofNodes,
    log_index: u32,
    id: &[u8; 32],
    commitment: &[u8; 32],
) -> Result<(), BridgeFault> {
    let key = rlp::encode_number(index);
    let encoded = mpt::require(&header.extra_root, &key, &raw_nodes(nodes))?;
    let log = read_log(&encoded, log_index)?;
    if log.address != params.contract {
        return Err(BridgeFault::CommitmentMismatch);
    }
    if log.topics.first() != Some(&params.topic) {
        return Err(BridgeFault::CommitmentMismatch);
    }
    if log.topics.get(1) != Some(id) {
        return Err(BridgeFault::CommitmentMismatch);
    }
    if log.data.get(..32) != Some(commitment.as_slice()) {
        return Err(BridgeFault::CommitmentMismatch);
    }
    Ok(())
}

/// One log entry of one receipt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EvmLog {
    /// The contract that emitted the log.
    pub address: [u8; 20],
    /// The indexed topics of the log.
    pub topics: Vec<[u8; 32]>,
    /// The unindexed bytes of the log.
    pub data: Vec<u8>,
}

/// Read one log of one receipt.
///
/// A receipt of the original form is a list of four items. A receipt of a
/// later form carries one leading byte naming the form, then the same list
/// of four items. The reader handles both.
pub fn read_log(encoded: &[u8], log_index: u32) -> Result<EvmLog, BridgeFault> {
    let body = match encoded.first() {
        Some(marker) if *marker < 0x80 => encoded.get(1..).ok_or(BridgeFault::MalformedEncoding)?,
        Some(_) => encoded,
        None => return Err(BridgeFault::MalformedEncoding),
    };
    let item = rlp::decode(body).map_err(|_| BridgeFault::MalformedEncoding)?;
    let fields = item.list().map_err(|_| BridgeFault::MalformedEncoding)?;
    if fields.len() != 4 {
        return Err(BridgeFault::MalformedEncoding);
    }
    let logs = fields[3]
        .list()
        .map_err(|_| BridgeFault::MalformedEncoding)?;
    let held = logs
        .get(log_index as usize)
        .ok_or(BridgeFault::ValueAbsent)?;
    let parts = held.list().map_err(|_| BridgeFault::MalformedEncoding)?;
    if parts.len() != 3 {
        return Err(BridgeFault::MalformedEncoding);
    }
    let raw_address = parts[0]
        .bytes()
        .map_err(|_| BridgeFault::MalformedEncoding)?;
    if raw_address.len() != 20 {
        return Err(BridgeFault::MalformedEncoding);
    }
    let mut address = [0u8; 20];
    address.copy_from_slice(raw_address);
    let raw_topics = parts[1]
        .list()
        .map_err(|_| BridgeFault::MalformedEncoding)?;
    let mut topics: Vec<[u8; 32]> = Vec::with_capacity(raw_topics.len());
    for topic in raw_topics.iter() {
        let held = topic.bytes().map_err(|_| BridgeFault::MalformedEncoding)?;
        if held.len() != 32 {
            return Err(BridgeFault::MalformedEncoding);
        }
        let mut word = [0u8; 32];
        word.copy_from_slice(held);
        topics.push(word);
    }
    let data = parts[2]
        .bytes()
        .map_err(|_| BridgeFault::MalformedEncoding)?
        .to_vec();
    Ok(EvmLog {
        address,
        topics,
        data,
    })
}

/// Build the encoding of one log entry.
///
/// The bridge builds logs while it produces fixtures and while it tests
/// the receipt path. A node of the far network builds the same bytes.
pub fn encode_log(log: &EvmLog) -> Vec<u8> {
    let topics: Vec<Vec<u8>> = log
        .topics
        .iter()
        .map(|held| rlp::encode_bytes(held))
        .collect();
    rlp::encode_list(&[
        rlp::encode_bytes(&log.address),
        rlp::encode_list(&topics),
        rlp::encode_bytes(&log.data),
    ])
}

/// Build the encoding of one receipt of the original form.
pub fn encode_receipt(status: u64, gas_used: u64, logs: &[EvmLog]) -> Vec<u8> {
    let encoded: Vec<Vec<u8>> = logs.iter().map(encode_log).collect();
    rlp::encode_list(&[
        rlp::encode_number(status),
        rlp::encode_number(gas_used),
        rlp::encode_bytes(&[0u8; 256]),
        rlp::encode_list(&encoded),
    ])
}

/// Build the encoding of one header from its fields.
///
/// The builder writes the fifteen fields a header of the original form
/// carries. The bridge uses the builder while it produces fixtures.
pub fn encode_header(
    parent: &[u8; 32],
    state_root: &[u8; 32],
    receipts_root: &[u8; 32],
    number: u64,
) -> Vec<u8> {
    let mut fields: Vec<Vec<u8>> = vec![
        rlp::encode_bytes(parent),
        rlp::encode_bytes(&[0u8; 32]),
        rlp::encode_bytes(&[0u8; 20]),
        rlp::encode_bytes(state_root),
        rlp::encode_bytes(&[0u8; 32]),
        rlp::encode_bytes(receipts_root),
        rlp::encode_bytes(&[0u8; 256]),
        rlp::encode_number(1),
        rlp::encode_number(number),
    ];
    fields.push(rlp::encode_number(30_000_000));
    fields.push(rlp::encode_number(21_000));
    fields.push(rlp::encode_number(1_700_000_000));
    fields.push(rlp::encode_bytes(b"cerulea"));
    fields.push(rlp::encode_bytes(&[0u8; 32]));
    fields.push(rlp::encode_bytes(&[0u8; 8]));
    rlp::encode_list(&fields)
}
