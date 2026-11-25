//! The endpoint registry.
//!
//! One endpoint joins one deployment to one network. The record holds the
//! network kind, the verifier parameters of that kind, the message format
//! and the action an arriving message runs. The registry is the single
//! place a person reads to learn what the deployment accepts.
//!
//! # The verifier parameters
//!
//! Every target network needs starting evidence before a proof means
//! anything. A network with a validator set needs the keys and the weight
//! of that set. A network with accumulated work needs a trusted block and
//! the work limit. A network reached through relayers needs the relayer
//! keys and the signing threshold. The parameters hold that evidence and
//! move forward as the endpoint accepts far network headers.
//!
//! # The trust assumption
//!
//! The record states in plain terms what an accepted proof rests on. An
//! operator reading the endpoint sees the strength of the evidence without
//! reading the verifier code.

use crate::envelope::{MessageFormat, NetworkAddress, NetworkKind, TrustAssumption};
use alloc::vec::Vec;
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use frame_support::{pallet_prelude::ConstU32, BoundedVec};
use pallet_cerulea_audit::contract::WorkClass;
use pallet_cerulea_schema::primitives::{BoundedName, DeploymentId};
use scale_info::TypeInfo;

/// The largest validator set one endpoint holds.
pub const MAX_VALIDATORS: u32 = 128;

/// The largest relayer set one endpoint holds.
pub const MAX_RELAYERS: u32 = 32;

/// The largest store prefix one endpoint holds, in bytes.
pub const MAX_PREFIX_BYTES: u32 = 128;

/// The largest storage key one endpoint holds, in bytes.
pub const MAX_STORAGE_KEY_BYTES: u32 = 128;

/// One endpoint inside one deployment.
///
/// The pair is the key of every endpoint map the module holds.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    DecodeWithMemTracking,
    TypeInfo,
    MaxEncodedLen,
)]
pub struct EndpointKey {
    /// The deployment the endpoint belongs to.
    pub deployment: DeploymentId,
    /// The endpoint name.
    pub name: BoundedName,
}

impl EndpointKey {
    /// Build an endpoint key.
    pub fn new(deployment: DeploymentId, name: BoundedName) -> Self {
        EndpointKey { deployment, name }
    }
}

/// One trusted point on a far network chain.
///
/// The height and the hash together name one block. A header chain that
/// reaches this block and carries the evidence the network needs extends
/// the trust forward.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    Encode,
    Decode,
    DecodeWithMemTracking,
    TypeInfo,
    MaxEncodedLen,
)]
pub struct Checkpoint {
    /// The height, block number or slot of the trusted block.
    pub height: u64,
    /// The hash of the trusted block.
    pub hash: [u8; 32],
}

/// One accepted far network header.
///
/// The endpoint records a header once the evidence of that network passes.
/// A later proof names a height and verifies against the roots here.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    Encode,
    Decode,
    DecodeWithMemTracking,
    TypeInfo,
    MaxEncodedLen,
)]
pub struct HeaderRecord {
    /// The height of the header.
    pub height: u64,
    /// The hash of the header.
    pub hash: [u8; 32],
    /// The root committing to the state of the far network at this height.
    ///
    /// A network running Ethereum Virtual Machine bytecode puts its state
    /// trie root here. A Cosmos network puts its application hash here. A
    /// Solana network puts its bank hash here. A Polkadot network puts its
    /// state trie root here. Bitcoin puts its transaction merkle root here.
    pub state_root: [u8; 32],
    /// The second root the header carries.
    ///
    /// A network running Ethereum Virtual Machine bytecode puts its
    /// receipts trie root here. A Solana network puts its accounts delta
    /// hash here. A network carrying one root repeats the state root.
    pub extra_root: [u8; 32],
    /// The moment the endpoint accepted the header, in milliseconds.
    pub at_millis: u64,
    /// The trust the acceptance of the header rests on.
    pub trust: TrustAssumption,
}

/// One signer of a far network validator set.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    Encode,
    Decode,
    DecodeWithMemTracking,
    TypeInfo,
    MaxEncodedLen,
)]
pub struct ValidatorEntry {
    /// The public key of the signer.
    pub key: [u8; 32],
    /// The weight the signer carries in the set.
    pub power: u64,
}

/// The signature scheme one authority key carries.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    Encode,
    Decode,
    DecodeWithMemTracking,
    TypeInfo,
    MaxEncodedLen,
)]
pub enum SignatureScheme {
    /// An Edwards curve signature over the twenty five five curve.
    #[default]
    Ed25519,
    /// A Ristretto Schnorr signature over the twenty five five curve.
    Sr25519,
}

/// One member of a registered authority set.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    Encode,
    Decode,
    DecodeWithMemTracking,
    TypeInfo,
    MaxEncodedLen,
)]
pub struct AuthorityEntry {
    /// The public key of the authority.
    pub key: [u8; 32],
    /// The scheme the authority signs under.
    pub scheme: SignatureScheme,
    /// The weight the authority carries in the set.
    pub weight: u64,
}

/// The bounded validator set of one endpoint.
pub type ValidatorSet = BoundedVec<ValidatorEntry, ConstU32<MAX_VALIDATORS>>;

/// The bounded authority set of one endpoint.
pub type AuthoritySet = BoundedVec<AuthorityEntry, ConstU32<MAX_VALIDATORS>>;

/// The bounded relayer set of one endpoint.
pub type RelayerSet = BoundedVec<[u8; 32], ConstU32<MAX_RELAYERS>>;

/// The bounded store prefix of one endpoint.
pub type StorePrefix = BoundedVec<u8, ConstU32<MAX_PREFIX_BYTES>>;

/// The bounded storage key of one endpoint.
pub type StorageKey = BoundedVec<u8, ConstU32<MAX_STORAGE_KEY_BYTES>>;

/// The parameters of an endpoint reaching a network that runs Ethereum
/// Virtual Machine bytecode.
#[derive(
    Clone,
    Debug,
    Default,
    PartialEq,
    Eq,
    Encode,
    Decode,
    DecodeWithMemTracking,
    TypeInfo,
    MaxEncodedLen,
)]
pub struct EvmParameters {
    /// The identifier the far network answers with.
    pub chain_id: u64,
    /// The trusted block the header chain extends from.
    pub checkpoint: Checkpoint,
    /// The contract on the far network that writes the commitments.
    pub contract: [u8; 20],
    /// The first topic of the log the commitment appears under.
    pub topic: [u8; 32],
    /// The storage slot the contract writes the commitment into.
    ///
    /// The slot of one message is the Ethereum Virtual Machine mapping
    /// slot. The verifier computes it from this base and the message
    /// identifier.
    pub slot: [u8; 32],
    /// The confirmations the endpoint requires above the block.
    pub confirmations: u32,
    /// The keys of the header feed that attests each header.
    pub header_signers: RelayerSet,
    /// The number of header feed signatures the endpoint requires.
    pub header_threshold: u32,
}

/// The parameters of an endpoint reaching a Cosmos network.
#[derive(
    Clone,
    Debug,
    Default,
    PartialEq,
    Eq,
    Encode,
    Decode,
    DecodeWithMemTracking,
    TypeInfo,
    MaxEncodedLen,
)]
pub struct CosmosParameters {
    /// The chain identifier the far network signs into every commit.
    pub chain_id: BoundedName,
    /// The validator set of the far network with the weight of each member.
    pub validators: ValidatorSet,
    /// The trusted block the commits extend from.
    pub checkpoint: Checkpoint,
    /// The store name the messages live under.
    pub store_prefix: StorePrefix,
    /// The key prefix inside the store.
    pub key_prefix: StorePrefix,
}

/// The parameters of an endpoint reaching a Solana network.
#[derive(
    Clone,
    Debug,
    Default,
    PartialEq,
    Eq,
    Encode,
    Decode,
    DecodeWithMemTracking,
    TypeInfo,
    MaxEncodedLen,
)]
pub struct SolanaParameters {
    /// The program that writes the commitments.
    pub program: [u8; 32],
    /// The account the program writes the commitments into.
    pub account: [u8; 32],
    /// The keys of the slot leaders the endpoint accepts.
    pub leaders: RelayerSet,
    /// The keys of the registered relayers that attest a slot header.
    pub relayers: RelayerSet,
    /// The number of relayer signatures the endpoint requires.
    pub relayer_threshold: u32,
    /// The trusted slot the attested headers extend from.
    pub checkpoint: Checkpoint,
}

/// The parameters of an endpoint reaching a Polkadot network.
#[derive(
    Clone,
    Debug,
    Default,
    PartialEq,
    Eq,
    Encode,
    Decode,
    DecodeWithMemTracking,
    TypeInfo,
    MaxEncodedLen,
)]
pub struct PolkadotParameters {
    /// The authority set that signs the finality justification.
    pub authorities: AuthoritySet,
    /// The identifier of the current authority set.
    pub set_id: u64,
    /// The trusted block the justifications extend from.
    pub checkpoint: Checkpoint,
    /// The storage key the commitments live under.
    pub storage_key: StorageKey,
}

/// The parameters of an endpoint reaching an Avalanche network.
#[derive(
    Clone,
    Debug,
    Default,
    PartialEq,
    Eq,
    Encode,
    Decode,
    DecodeWithMemTracking,
    TypeInfo,
    MaxEncodedLen,
)]
pub struct AvalancheParameters {
    /// The identifier of the far chain inside the network.
    pub chain_id: [u8; 32],
    /// The validator set with the weight of each member.
    pub validators: ValidatorSet,
    /// The share of the total weight the endpoint requires, in percent.
    pub threshold_percent: u8,
    /// The address on the far network that sends the messages.
    pub sender: NetworkAddress,
}

/// The parameters of an endpoint reaching the Bitcoin network.
#[derive(
    Clone,
    Debug,
    Default,
    PartialEq,
    Eq,
    Encode,
    Decode,
    DecodeWithMemTracking,
    TypeInfo,
    MaxEncodedLen,
)]
pub struct BitcoinParameters {
    /// The trusted block the header chain extends from.
    pub checkpoint: Checkpoint,
    /// The largest target a header may carry, as a big endian number.
    ///
    /// A header whose hash sits above this number carries too little work.
    pub work_limit: [u8; 32],
    /// The confirmations the endpoint requires above the block.
    pub confirmations: u32,
    /// The marker the anchoring output carries ahead of the state root.
    pub anchor_marker: BoundedVec<u8, ConstU32<8>>,
}

/// The parameters of an endpoint reaching a Layer 2 rollup.
#[derive(
    Clone,
    Debug,
    Default,
    PartialEq,
    Eq,
    Encode,
    Decode,
    DecodeWithMemTracking,
    TypeInfo,
    MaxEncodedLen,
)]
pub struct RollupParameters {
    /// The settlement layer the rollup commits its output roots to.
    pub settlement: EvmParameters,
    /// The contract on the settlement layer holding the output roots.
    pub output_contract: [u8; 20],
    /// The storage slot the output roots live under.
    pub output_slot: [u8; 32],
    /// The contract inside the rollup that writes the commitments.
    pub contract: [u8; 20],
    /// The storage slot the contract writes the commitment into.
    pub slot: [u8; 32],
    /// The challenge window the output root passes before acceptance, in
    /// milliseconds.
    pub challenge_window_millis: u64,
}

/// The verifier parameters of one endpoint.
///
/// The variant matches the network kind of the endpoint. A proof of one
/// kind against parameters of another kind is declined.
#[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo, MaxEncodedLen,
)]
pub enum VerifierParameters {
    /// A network running Ethereum Virtual Machine bytecode.
    EthereumVirtualMachine(EvmParameters),
    /// A Cosmos network.
    Cosmos(CosmosParameters),
    /// A Solana network.
    Solana(SolanaParameters),
    /// A Polkadot network.
    Polkadot(PolkadotParameters),
    /// An Avalanche network.
    Avalanche(AvalancheParameters),
    /// The Bitcoin network.
    Bitcoin(BitcoinParameters),
    /// A Layer 2 rollup.
    Rollup(RollupParameters),
}

impl VerifierParameters {
    /// The network kind the parameters belong to.
    pub fn kind(&self) -> NetworkKind {
        match self {
            VerifierParameters::EthereumVirtualMachine(_) => NetworkKind::EthereumVirtualMachine,
            VerifierParameters::Cosmos(_) => NetworkKind::Cosmos,
            VerifierParameters::Solana(_) => NetworkKind::Solana,
            VerifierParameters::Polkadot(_) => NetworkKind::Polkadot,
            VerifierParameters::Avalanche(_) => NetworkKind::Avalanche,
            VerifierParameters::Bitcoin(_) => NetworkKind::Bitcoin,
            VerifierParameters::Rollup(_) => NetworkKind::Rollup,
        }
    }

    /// The trusted checkpoint the parameters hold.
    ///
    /// An Avalanche endpoint reaches its messages through validator
    /// signatures alone, so that endpoint holds no checkpoint.
    pub fn checkpoint(&self) -> Option<Checkpoint> {
        match self {
            VerifierParameters::EthereumVirtualMachine(held) => Some(held.checkpoint),
            VerifierParameters::Cosmos(held) => Some(held.checkpoint),
            VerifierParameters::Solana(held) => Some(held.checkpoint),
            VerifierParameters::Polkadot(held) => Some(held.checkpoint),
            VerifierParameters::Avalanche(_) => None,
            VerifierParameters::Bitcoin(held) => Some(held.checkpoint),
            VerifierParameters::Rollup(held) => Some(held.settlement.checkpoint),
        }
    }

    /// Move the trusted checkpoint forward.
    ///
    /// The call answers true when the checkpoint moved. A checkpoint at or
    /// below the height already trusted stays where it is.
    pub fn advance(&mut self, checkpoint: Checkpoint) -> bool {
        let current = self.checkpoint();
        if let Some(held) = current {
            if checkpoint.height <= held.height {
                return false;
            }
        }
        match self {
            VerifierParameters::EthereumVirtualMachine(held) => held.checkpoint = checkpoint,
            VerifierParameters::Cosmos(held) => held.checkpoint = checkpoint,
            VerifierParameters::Solana(held) => held.checkpoint = checkpoint,
            VerifierParameters::Polkadot(held) => held.checkpoint = checkpoint,
            VerifierParameters::Avalanche(_) => return false,
            VerifierParameters::Bitcoin(held) => held.checkpoint = checkpoint,
            VerifierParameters::Rollup(held) => held.settlement.checkpoint = checkpoint,
        }
        true
    }

    /// The trust an accepted proof against these parameters rests on.
    pub fn trust(&self) -> TrustAssumption {
        match self {
            VerifierParameters::EthereumVirtualMachine(held) => {
                TrustAssumption::RelayerAttestation {
                    signed: held.header_threshold,
                    registered: held.header_signers.len() as u32,
                }
            }
            VerifierParameters::Cosmos(held) => TrustAssumption::ValidatorThreshold {
                signed: 0,
                total: total_power(&held.validators),
            },
            VerifierParameters::Solana(held) => TrustAssumption::RelayerAttestation {
                signed: held.relayer_threshold,
                registered: held.relayers.len() as u32,
            },
            VerifierParameters::Polkadot(held) => TrustAssumption::ValidatorThreshold {
                signed: 0,
                total: total_weight(&held.authorities),
            },
            VerifierParameters::Avalanche(held) => TrustAssumption::ValidatorThreshold {
                signed: 0,
                total: total_power(&held.validators),
            },
            VerifierParameters::Bitcoin(held) => TrustAssumption::AccumulatedWork {
                confirmations: held.confirmations,
            },
            VerifierParameters::Rollup(held) => TrustAssumption::SettledAfterChallenge {
                window_millis: held.challenge_window_millis,
            },
        }
    }
}

/// The total weight one validator set carries.
pub fn total_power(set: &ValidatorSet) -> u64 {
    set.iter()
        .fold(0u64, |sum, held| sum.saturating_add(held.power))
}

/// The total weight one authority set carries.
pub fn total_weight(set: &AuthoritySet) -> u64 {
    set.iter()
        .fold(0u64, |sum, held| sum.saturating_add(held.weight))
}

/// Report whether a signing weight reaches two thirds plus one of a total.
///
/// The comparison uses whole numbers throughout, so no rounding enters the
/// answer. A signing weight passes when three times the weight runs past
/// twice the total.
pub fn reaches_two_thirds(signed: u64, total: u64) -> bool {
    if total == 0 {
        return false;
    }
    let left = (signed as u128).saturating_mul(3);
    let right = (total as u128).saturating_mul(2);
    left > right
}

/// Report whether a signing weight reaches a share of a total in percent.
pub fn reaches_percent(signed: u64, total: u64, percent: u8) -> bool {
    if total == 0 {
        return false;
    }
    let left = (signed as u128).saturating_mul(100);
    let right = (total as u128).saturating_mul(percent as u128);
    left >= right
}

/// One registered endpoint.
#[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo, MaxEncodedLen,
)]
pub struct EndpointRecord<AccountId, BlockNumber> {
    /// The kind of network on the far side.
    pub kind: NetworkKind,
    /// The name the operator gave the network on the far side.
    pub network: BoundedName,
    /// The direction messages travel through the endpoint.
    pub direction: crate::envelope::Travel,
    /// The endpoint takes messages in both directions.
    pub both_ways: bool,
    /// The collection the endpoint reads and writes.
    pub collection: BoundedName,
    /// The action an arriving message runs.
    pub action: BoundedName,
    /// The format of the payload bytes.
    pub format: MessageFormat,
    /// The transaction class an arriving message runs under.
    ///
    /// The action of the Blueprint carries its own class. The endpoint
    /// class applies where the action leaves the class open.
    pub class: WorkClass,
    /// The lifetime an outbound message takes, in milliseconds.
    pub lifetime_millis: u64,
    /// The verifier parameters of the network kind.
    pub params: VerifierParameters,
    /// The account that registered the endpoint.
    pub registrar: AccountId,
    /// The account an arriving message runs as.
    ///
    /// The far network sender holds no account on this chain. The
    /// deployment names one account that acts for the endpoint, and the
    /// audit log records the far network sender as the actor context.
    pub executor: AccountId,
    /// The block the endpoint landed at.
    pub registered_at: BlockNumber,
    /// The endpoint takes messages while this holds.
    pub live: bool,
}

impl<AccountId, BlockNumber> EndpointRecord<AccountId, BlockNumber> {
    /// Report whether the endpoint takes a message travelling this way.
    pub fn takes(&self, travel: crate::envelope::Travel) -> bool {
        self.live && (self.both_ways || self.direction == travel)
    }

    /// The trust an accepted proof on this endpoint rests on.
    pub fn trust(&self) -> TrustAssumption {
        self.params.trust()
    }
}

/// The far network headers one verifier reads.
///
/// The module implements this over its header storage. A unit test
/// implements it over a fixed list, so a verifier runs without storage.
pub trait HeaderSource {
    /// The header the endpoint accepted at one height.
    fn header(&self, height: u64) -> Option<HeaderRecord>;

    /// The highest height the endpoint accepted.
    fn tip_height(&self) -> u64;
}

/// A header source over a fixed list.
///
/// The list stays in the order given. A lookup walks the list, so the list
/// holds the headers one proof needs rather than a whole chain.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ListedHeaders {
    /// The headers the source answers with.
    pub headers: Vec<HeaderRecord>,
}

impl ListedHeaders {
    /// Build a source over one header.
    pub fn one(header: HeaderRecord) -> Self {
        ListedHeaders {
            headers: alloc::vec![header],
        }
    }

    /// Build a source over several headers.
    pub fn many(headers: Vec<HeaderRecord>) -> Self {
        ListedHeaders { headers }
    }
}

impl HeaderSource for ListedHeaders {
    fn header(&self, height: u64) -> Option<HeaderRecord> {
        self.headers
            .iter()
            .find(|held| held.height == height)
            .copied()
    }

    fn tip_height(&self) -> u64 {
        self.headers
            .iter()
            .fold(0u64, |top, held| top.max(held.height))
    }
}

/// The endpoint a verifier reads while it checks one proof.
pub struct EndpointState<'a> {
    /// The kind of network on the far side.
    pub kind: NetworkKind,
    /// The verifier parameters of that kind.
    pub params: &'a VerifierParameters,
    /// The format of the payload bytes.
    pub format: MessageFormat,
    /// The headers the endpoint accepted.
    pub headers: &'a dyn HeaderSource,
    /// The moment the check runs at, in milliseconds.
    pub now_millis: u64,
}

impl<'a> EndpointState<'a> {
    /// Build the state a verifier reads.
    pub fn new(
        params: &'a VerifierParameters,
        format: MessageFormat,
        headers: &'a dyn HeaderSource,
        now_millis: u64,
    ) -> Self {
        EndpointState {
            kind: params.kind(),
            params,
            format,
            headers,
            now_millis,
        }
    }
}
