//! The far network fixtures the tests run against.
//!
//! Every fixture is a real cryptographic artefact. A trie fixture is a
//! real Merkle Patricia trie built with the same hash the far network
//! uses. A signature fixture is a real signature over the real payload
//! bytes. A Bitcoin fixture carries a real header whose hash meets its own
//! target. No fixture stands in for cryptography that was skipped.
//!
//! The local driver of Cerulea APIs builds the same structures from the
//! same rules. The round trip test drives a message out through the chain,
//! rebuilds the far network side here and brings the reply back with its
//! proof.

use crate::endpoint::{
    AuthorityEntry, AvalancheParameters, BitcoinParameters, Checkpoint, CosmosParameters,
    EvmParameters, PolkadotParameters, RollupParameters, SignatureScheme, SolanaParameters,
    ValidatorEntry, VerifierParameters,
};
use crate::envelope::{MessageEnvelope, NetworkKind};
use crate::merkle;
use crate::mpt;
use crate::networks::{avalanche, bitcoin, cosmos, evm, polkadot, rollup, solana};
use crate::rlp;
use crate::verify::{IndexedSignature, KeyedSignature, Proof, ProofNode, ProofNodes};
use crate::HeaderBatch;
use alloc::{vec, vec::Vec};
use frame_support::BoundedVec;
use sp_core::{ed25519, sr25519, Pair};
use sp_runtime::traits::BlakeTwo256;
use sp_trie::{LayoutV1, MemoryDB, TrieDBMutBuilder, TrieMut};

/// The height the far network checkpoint of every fixture starts at.
pub const START_HEIGHT: u64 = 100;

/// One far network side prepared for one envelope.
pub struct Prepared {
    /// The verifier parameters the endpoint registers.
    pub params: VerifierParameters,
    /// The header batch the endpoint accepts first.
    pub headers: Option<HeaderBatch>,
    /// The proof the message travels with.
    pub proof: Proof,
}

/// The block every fixture of one network kind extends from first.
pub fn first_base(kind: NetworkKind) -> Checkpoint {
    let hash = match kind {
        NetworkKind::EthereumVirtualMachine => [0xaa; 32],
        NetworkKind::Cosmos => [0xbb; 32],
        NetworkKind::Solana => [0xcc; 32],
        NetworkKind::Polkadot => [0xee; 32],
        NetworkKind::Avalanche => [0x99; 32],
        NetworkKind::Bitcoin => [0x1f; 32],
        NetworkKind::Rollup => [0xab; 32],
    };
    Checkpoint {
        height: START_HEIGHT,
        hash,
    }
}

/// Build the far network side of one envelope for one network kind.
pub fn prepare(kind: NetworkKind, envelope: &MessageEnvelope, now_millis: u64) -> Prepared {
    prepare_from(kind, envelope, now_millis, first_base(kind))
}

/// Build the far network side of one envelope above one trusted block.
///
/// A second message on the same endpoint extends the chain the first
/// message left behind, so the fixture takes the block the endpoint
/// trusts now rather than the block it trusted at registration.
pub fn prepare_from(
    kind: NetworkKind,
    envelope: &MessageEnvelope,
    now_millis: u64,
    base: Checkpoint,
) -> Prepared {
    match kind {
        NetworkKind::EthereumVirtualMachine => evm_side(envelope, false, base),
        NetworkKind::Cosmos => cosmos_side(envelope, base),
        NetworkKind::Solana => solana_side(envelope, base),
        NetworkKind::Polkadot => polkadot_side(envelope, base),
        NetworkKind::Avalanche => avalanche_side(envelope, base),
        NetworkKind::Bitcoin => bitcoin_side(envelope, base),
        NetworkKind::Rollup => rollup_side(envelope, now_millis, base),
    }
}

/// Turn a list of node encodings into the bounded shape a proof carries.
pub fn bounded_nodes(nodes: &[Vec<u8>]) -> ProofNodes {
    let held: Vec<ProofNode> = nodes
        .iter()
        .map(|node| BoundedVec::try_from(node.clone()).expect("a trie node sits inside the bound"))
        .collect();
    BoundedVec::try_from(held).expect("a proof sits inside the node limit")
}

/// One Edwards curve key pair built from a seed byte.
pub fn ed_pair(seed: u8) -> ed25519::Pair {
    ed25519::Pair::from_seed(&[seed; 32])
}

/// One Ristretto Schnorr key pair built from a seed byte.
pub fn sr_pair(seed: u8) -> sr25519::Pair {
    sr25519::Pair::from_seed(&[seed; 32])
}

/// The public key bytes of one Edwards curve pair.
pub fn ed_key(pair: &ed25519::Pair) -> [u8; 32] {
    pair.public().0
}

/// Sign one message with one Edwards curve pair.
pub fn ed_sign(pair: &ed25519::Pair, message: &[u8]) -> [u8; 64] {
    pair.sign(message).0
}

// --- The Merkle Patricia trie builder -----------------------------------

/// The path encoding one leaf node or one extension node carries.
pub fn hex_prefix(path: &[u8], leaf: bool) -> Vec<u8> {
    let odd = path.len() % 2 == 1;
    let flag = (if leaf { 2u8 } else { 0u8 }) + (if odd { 1u8 } else { 0u8 });
    let mut found: Vec<u8> = Vec::with_capacity(path.len() / 2 + 1);
    let start = if odd {
        found.push((flag << 4) | path[0]);
        1usize
    } else {
        found.push(flag << 4);
        0usize
    };
    let mut cursor = start;
    while cursor + 1 < path.len() {
        found.push((path[cursor] << 4) | path[cursor + 1]);
        cursor += 2;
    }
    found
}

/// Build one Merkle Patricia trie and answer with the root and the nodes.
///
/// The builder writes the same nodes a node of the far network writes, so
/// a walk of the nodes reaches the same root.
pub fn build_trie(entries: &[(Vec<u8>, Vec<u8>)]) -> ([u8; 32], Vec<Vec<u8>>) {
    let mut items: Vec<(Vec<u8>, Vec<u8>)> = entries
        .iter()
        .map(|(key, value)| (mpt::nibbles(key), value.clone()))
        .collect();
    items.sort();
    let mut nodes: Vec<Vec<u8>> = Vec::new();
    let encoded = build_node(&items, &mut nodes);
    let root = sp_crypto_hashing::keccak_256(&encoded);
    nodes.push(encoded);
    (root, nodes)
}

/// The reference a parent node holds for one child.
fn node_ref(encoded: Vec<u8>, nodes: &mut Vec<Vec<u8>>) -> Vec<u8> {
    if encoded.len() < 32 {
        return encoded;
    }
    let hash = sp_crypto_hashing::keccak_256(&encoded);
    nodes.push(encoded);
    rlp::encode_bytes(&hash)
}

/// Build one node of the trie over a set of nibble paths.
fn build_node(items: &[(Vec<u8>, Vec<u8>)], nodes: &mut Vec<Vec<u8>>) -> Vec<u8> {
    if items.len() == 1 {
        let (path, value) = &items[0];
        return rlp::encode_list(&[
            rlp::encode_bytes(&hex_prefix(path, true)),
            rlp::encode_bytes(value),
        ]);
    }
    let common = common_prefix(items);
    if common > 0 {
        let stripped: Vec<(Vec<u8>, Vec<u8>)> = items
            .iter()
            .map(|(path, value)| (path[common..].to_vec(), value.clone()))
            .collect();
        let child = build_node(&stripped, nodes);
        let reference = node_ref(child, nodes);
        return rlp::encode_list(&[
            rlp::encode_bytes(&hex_prefix(&items[0].0[..common], false)),
            reference,
        ]);
    }
    let mut slots: Vec<Vec<u8>> = vec![rlp::encode_bytes(&[]); 17];
    for nibble in 0u8..16 {
        let group: Vec<(Vec<u8>, Vec<u8>)> = items
            .iter()
            .filter(|(path, _)| path.first() == Some(&nibble))
            .map(|(path, value)| (path[1..].to_vec(), value.clone()))
            .collect();
        if !group.is_empty() {
            let child = build_node(&group, nodes);
            slots[nibble as usize] = node_ref(child, nodes);
        }
    }
    if let Some((_, value)) = items.iter().find(|(path, _)| path.is_empty()) {
        slots[16] = rlp::encode_bytes(value);
    }
    rlp::encode_list(&slots)
}

/// The length of the nibble prefix every path shares.
fn common_prefix(items: &[(Vec<u8>, Vec<u8>)]) -> usize {
    let first = &items[0].0;
    let mut length = first.len();
    for (path, _) in items.iter().skip(1) {
        let mut held = 0usize;
        while held < length && held < path.len() && path[held] == first[held] {
            held += 1;
        }
        length = held;
    }
    length
}

/// The encoding one storage word takes inside a storage trie.
///
/// A storage value sits in the trie with its leading zero bytes removed.
pub fn storage_value(word: &[u8; 32]) -> Vec<u8> {
    let start = word.iter().position(|byte| *byte != 0).unwrap_or(31);
    rlp::encode_bytes(&word[start..])
}

/// The encoding one account takes inside a state trie.
pub fn account_value(nonce: u64, storage_root: &[u8; 32]) -> Vec<u8> {
    rlp::encode_list(&[
        rlp::encode_number(nonce),
        rlp::encode_number(1_000),
        rlp::encode_bytes(storage_root),
        rlp::encode_bytes(&sp_crypto_hashing::keccak_256(b"cerulea.bridge.code")),
    ])
}

// --- The network that runs Ethereum Virtual Machine bytecode ------------

/// The contract every fixture of that network writes from.
pub const EVM_CONTRACT: [u8; 20] = [0x11; 20];

/// The base slot every fixture of that network writes into.
pub const EVM_SLOT: [u8; 32] = [0x22; 32];

/// The log topic every fixture of that network carries.
pub const EVM_TOPIC: [u8; 32] = [0x33; 32];

/// Build the far network side of one envelope for such a network.
///
/// The receipt flag picks the receipt path over the storage path.
pub fn evm_side(envelope: &MessageEnvelope, receipt: bool, base: Checkpoint) -> Prepared {
    let id = envelope.id();
    let commitment = envelope.commitment();

    let slot = mpt::mapping_slot(&id, &EVM_SLOT);
    let storage_entries = vec![
        (
            sp_crypto_hashing::keccak_256(&slot).to_vec(),
            storage_value(&commitment),
        ),
        (
            sp_crypto_hashing::keccak_256(&[0x44u8; 32]).to_vec(),
            storage_value(&[0x55u8; 32]),
        ),
    ];
    let (storage_root, storage_nodes) = build_trie(&storage_entries);

    let account_entries = vec![
        (
            sp_crypto_hashing::keccak_256(&EVM_CONTRACT).to_vec(),
            account_value(7, &storage_root),
        ),
        (
            sp_crypto_hashing::keccak_256(&[0x66u8; 20]).to_vec(),
            account_value(1, &[0u8; 32]),
        ),
    ];
    let (state_root, account_nodes) = build_trie(&account_entries);

    let log = evm::EvmLog {
        address: EVM_CONTRACT,
        topics: vec![EVM_TOPIC, id],
        data: commitment.to_vec(),
    };
    let receipt_entries = vec![
        (
            rlp::encode_number(0),
            evm::encode_receipt(1, 21_000, &[log]),
        ),
        (rlp::encode_number(1), evm::encode_receipt(1, 21_000, &[])),
    ];
    let (receipts_root, receipt_nodes) = build_trie(&receipt_entries);

    let height = base.height.saturating_add(1);
    let header = evm::encode_header(&base.hash, &state_root, &receipts_root, height);
    let read = evm::read_header(&header).expect("the fixture header reads");

    let signers = [ed_pair(1), ed_pair(2), ed_pair(3)];
    let params = EvmParameters {
        chain_id: 31_337,
        checkpoint: base,
        contract: EVM_CONTRACT,
        topic: EVM_TOPIC,
        slot: EVM_SLOT,
        confirmations: 1,
        header_signers: BoundedVec::try_from(signers.iter().map(ed_key).collect::<Vec<[u8; 32]>>())
            .expect("three keys sit inside the bound"),
        header_threshold: 2,
    };
    let payload = evm::feed_payload(params.chain_id, &read);
    let signatures: Vec<KeyedSignature> = signers
        .iter()
        .map(|pair| KeyedSignature {
            key: ed_key(pair),
            signature: ed_sign(pair, &payload),
        })
        .collect();
    let batch = evm::EvmHeaderBatch {
        headers: BoundedVec::try_from(vec![
            BoundedVec::try_from(header).expect("the header sits inside the bound")
        ])
        .expect("one header sits inside the bound"),
        signatures: BoundedVec::try_from(signatures)
            .expect("three signatures sit inside the bound"),
    };

    let proof = if receipt {
        Proof::EthereumVirtualMachine(evm::EvmProof::Receipt {
            height,
            index: 0,
            nodes: bounded_nodes(&receipt_nodes),
            log_index: 0,
        })
    } else {
        Proof::EthereumVirtualMachine(evm::EvmProof::Storage {
            height,
            account: bounded_nodes(&account_nodes),
            storage: bounded_nodes(&storage_nodes),
        })
    };

    Prepared {
        params: VerifierParameters::EthereumVirtualMachine(params),
        headers: Some(HeaderBatch::EthereumVirtualMachine(batch)),
        proof,
    }
}

// --- The Cosmos network -------------------------------------------------

/// The chain name every Cosmos fixture carries.
pub const COSMOS_CHAIN: &[u8] = b"cerulea-cosmos-local";

/// The store name every Cosmos fixture carries.
pub const COSMOS_STORE: &[u8] = b"cerulea";

/// The key prefix every Cosmos fixture carries.
pub const COSMOS_PREFIX: &[u8] = b"bridge/";

/// Build the far network side of one envelope for a Cosmos network.
pub fn cosmos_side(envelope: &MessageEnvelope, base: Checkpoint) -> Prepared {
    let id = envelope.id();
    let commitment = envelope.commitment();

    let message_key = cosmos::message_key(COSMOS_PREFIX, &id);
    let store_entries = vec![
        (message_key.clone(), commitment.to_vec()),
        (b"bridge/other".to_vec(), vec![9u8; 32]),
    ];
    let (message_proof, store_root) =
        cosmos::build_store_proof(&store_entries, 0).expect("the store proof builds");

    let outer_entries = vec![
        (COSMOS_STORE.to_vec(), store_root.to_vec()),
        (b"params".to_vec(), vec![3u8; 32]),
    ];
    let (store_proof, app_hash) =
        cosmos::build_root_proof(&outer_entries, 0).expect("the root proof builds");

    let mut fields: Vec<Vec<u8>> = (0u8..14).map(|held| vec![held; 32]).collect();
    let app_index = 10usize;
    fields[app_index] = app_hash.to_vec();
    let block_hash = merkle::prefixed_root(&fields);
    let branch = merkle::prefixed_branch(&fields, app_index);

    let pairs = [ed_pair(11), ed_pair(12), ed_pair(13), ed_pair(14)];
    let validators: Vec<ValidatorEntry> = pairs
        .iter()
        .map(|pair| ValidatorEntry {
            key: ed_key(pair),
            power: 10,
        })
        .collect();

    let parts = cosmos::PartHeader {
        total: 1,
        hash: [7u8; 32],
    };
    let height = base.height.saturating_add(1);
    let payload = cosmos::vote_payload(
        COSMOS_CHAIN,
        height,
        0,
        &block_hash,
        &parts,
        1_700_000_000,
        0,
    );
    let signatures: Vec<IndexedSignature> = pairs
        .iter()
        .enumerate()
        .map(|(position, pair)| IndexedSignature {
            signer: position as u32,
            signature: ed_sign(pair, &payload),
        })
        .collect();

    let params = CosmosParameters {
        chain_id: BoundedVec::try_from(COSMOS_CHAIN.to_vec()).expect("the chain name fits"),
        validators: BoundedVec::try_from(validators).expect("four validators fit"),
        checkpoint: base,
        store_prefix: BoundedVec::try_from(COSMOS_STORE.to_vec()).expect("the store name fits"),
        key_prefix: BoundedVec::try_from(COSMOS_PREFIX.to_vec()).expect("the key prefix fits"),
    };

    let proof = cosmos::CosmosProof {
        height,
        block_hash,
        round: 0,
        parts,
        seconds: 1_700_000_000,
        nanos: 0,
        signatures: BoundedVec::try_from(signatures).expect("four signatures fit"),
        app_hash,
        app_hash_index: app_index as u32,
        app_hash_total: fields.len() as u32,
        app_hash_branch: BoundedVec::try_from(branch).expect("the branch fits"),
        store_proof,
        message_proof,
    };

    Prepared {
        params: VerifierParameters::Cosmos(params),
        headers: None,
        proof: Proof::Cosmos(proof),
    }
}

// --- The Solana network -------------------------------------------------

/// The program every Solana fixture writes from.
pub const SOLANA_PROGRAM: [u8; 32] = [0x77; 32];

/// The account every Solana fixture writes into.
pub const SOLANA_ACCOUNT: [u8; 32] = [0x88; 32];

/// Build the far network side of one envelope for a Solana network.
pub fn solana_side(envelope: &MessageEnvelope, base: Checkpoint) -> Prepared {
    let id = envelope.id();
    let commitment = envelope.commitment();

    let account = solana::AccountState {
        address: SOLANA_ACCOUNT,
        lamports: 1_000_000,
        rent_epoch: 12,
        owner: SOLANA_PROGRAM,
        executable: false,
        data: BoundedVec::try_from(solana::write_commitment(&id, &commitment))
            .expect("the account data fits"),
    };
    let leaf = solana::account_hash(&account);

    let mut leaves: Vec<[u8; 32]> = (0u8..20)
        .map(|held| merkle::wide_leaf(&[held; 8]))
        .collect();
    let position = 3usize;
    leaves[position] = leaf;
    let first = merkle::wide_node(&leaves[..16]);
    let second = merkle::wide_node(&leaves[16..]);
    let accounts_delta_hash = merkle::wide_node(&[first, second]);
    let steps = vec![
        solana::AccountStep {
            children: BoundedVec::try_from(leaves[..16].to_vec()).expect("sixteen children fit"),
            position: position as u32,
        },
        solana::AccountStep {
            children: BoundedVec::try_from(vec![first, second]).expect("two children fit"),
            position: 0,
        },
    ];

    let slot = base.height.saturating_add(1);
    let header = solana::SlotHeader {
        slot,
        parent_bank_hash: base.hash,
        accounts_delta_hash,
        signature_count: 42,
        last_blockhash: [0xdd; 32],
    };
    let payload = header.payload();

    let relayers = [ed_pair(21), ed_pair(22), ed_pair(23)];
    let leader = ed_pair(24);
    let signatures: Vec<KeyedSignature> = relayers
        .iter()
        .map(|pair| KeyedSignature {
            key: ed_key(pair),
            signature: ed_sign(pair, &payload),
        })
        .collect();

    let params = SolanaParameters {
        program: SOLANA_PROGRAM,
        account: SOLANA_ACCOUNT,
        leaders: BoundedVec::try_from(vec![ed_key(&leader)]).expect("one leader fits"),
        relayers: BoundedVec::try_from(relayers.iter().map(ed_key).collect::<Vec<[u8; 32]>>())
            .expect("three relayers fit"),
        relayer_threshold: 2,
        checkpoint: base,
    };

    let batch = solana::SolanaHeaderBatch {
        headers: BoundedVec::try_from(vec![header]).expect("one header fits"),
        leader: Some(KeyedSignature {
            key: ed_key(&leader),
            signature: ed_sign(&leader, &payload),
        }),
        signatures: BoundedVec::try_from(signatures).expect("three signatures fit"),
    };

    let proof = solana::SolanaProof {
        slot,
        account,
        steps: BoundedVec::try_from(steps).expect("two steps fit"),
    };

    Prepared {
        params: VerifierParameters::Solana(params),
        headers: Some(HeaderBatch::Solana(batch)),
        proof: Proof::Solana(proof),
    }
}

// --- The Polkadot network -----------------------------------------------

/// The storage prefix every Polkadot fixture writes under.
pub const POLKADOT_PREFIX: &[u8] = b"cerulea.bridge.messages";

/// The identifier of the authority set every Polkadot fixture names.
pub const POLKADOT_SET_ID: u64 = 9;

/// Build the far network side of one envelope for a Polkadot network.
pub fn polkadot_side(envelope: &MessageEnvelope, base: Checkpoint) -> Prepared {
    let id = envelope.id();
    let commitment = envelope.commitment();

    let ed_pairs = [ed_pair(31), ed_pair(32)];
    let sr_pairs = [sr_pair(33), sr_pair(34)];
    let mut authorities: Vec<AuthorityEntry> = ed_pairs
        .iter()
        .map(|pair| AuthorityEntry {
            key: ed_key(pair),
            scheme: SignatureScheme::Ed25519,
            weight: 1,
        })
        .collect();
    for pair in sr_pairs.iter() {
        authorities.push(AuthorityEntry {
            key: pair.public().0,
            scheme: SignatureScheme::Sr25519,
            weight: 1,
        });
    }

    let params = PolkadotParameters {
        authorities: BoundedVec::try_from(authorities).expect("four authorities fit"),
        set_id: POLKADOT_SET_ID,
        checkpoint: base,
        storage_key: BoundedVec::try_from(POLKADOT_PREFIX.to_vec()).expect("the prefix fits"),
    };
    let key = polkadot::storage_key_for(&params, &id);
    let value = commitment.to_vec();

    let mut db = MemoryDB::<BlakeTwo256>::default();
    let mut root = sp_core::H256::default();
    {
        let mut trie = TrieDBMutBuilder::<LayoutV1<BlakeTwo256>>::new(&mut db, &mut root).build();
        trie.insert(&key, &value).expect("the trie takes the entry");
        trie.insert(b"cerulea.bridge.other", &[9u8; 32])
            .expect("the trie takes the entry");
    }
    let nodes = sp_trie::generate_trie_proof::<LayoutV1<BlakeTwo256>, _, _, _>(
        &db,
        root,
        [&key],
    )
    .expect("the trie proof builds");

    let number = u32::try_from(base.height.saturating_add(1)).expect("the number fits");
    let header = polkadot::encode_header(
        &base.hash,
        number,
        root.as_bytes().try_into().expect("the root fits"),
        &[0x12; 32],
    );
    let read = polkadot::read_header(&header).expect("the fixture header reads");
    let payload = polkadot::settlement_payload(&read.hash, number, 4, POLKADOT_SET_ID);

    let mut signatures: Vec<IndexedSignature> = ed_pairs
        .iter()
        .enumerate()
        .map(|(position, pair)| IndexedSignature {
            signer: position as u32,
            signature: ed_sign(pair, &payload),
        })
        .collect();
    for (position, pair) in sr_pairs.iter().enumerate() {
        signatures.push(IndexedSignature {
            signer: (position + 2) as u32,
            signature: pair.sign(&payload).0,
        });
    }

    let proof = polkadot::PolkadotProof {
        header: BoundedVec::try_from(header).expect("the header fits"),
        round: 4,
        set_id: POLKADOT_SET_ID,
        signatures: BoundedVec::try_from(signatures).expect("four signatures fit"),
        nodes: bounded_nodes(&nodes),
        value: BoundedVec::try_from(value).expect("the value fits"),
    };

    Prepared {
        params: VerifierParameters::Polkadot(params),
        headers: None,
        proof: Proof::Polkadot(proof),
    }
}

// --- The Avalanche network ----------------------------------------------

/// The identifier of the chain every Avalanche fixture names.
pub const AVALANCHE_CHAIN: [u8; 32] = [0x99; 32];

/// The identifier of the network every Avalanche fixture names.
pub const AVALANCHE_NETWORK: u32 = 1_337;

/// Build the far network side of one envelope for an Avalanche network.
pub fn avalanche_side(envelope: &MessageEnvelope, base: Checkpoint) -> Prepared {
    let commitment = envelope.commitment();
    let sender = vec![0xab; 20];
    let pairs = [ed_pair(41), ed_pair(42), ed_pair(43), ed_pair(44)];
    let validators: Vec<ValidatorEntry> = pairs
        .iter()
        .map(|pair| ValidatorEntry {
            key: ed_key(pair),
            power: 25,
        })
        .collect();

    let height = base.height.saturating_add(1);
    let params = AvalancheParameters {
        chain_id: AVALANCHE_CHAIN,
        validators: BoundedVec::try_from(validators).expect("four validators fit"),
        threshold_percent: 67,
        sender: BoundedVec::try_from(sender.clone()).expect("the sender address fits"),
    };
    let hash = avalanche::message_hash(AVALANCHE_NETWORK, &AVALANCHE_CHAIN, &sender, &commitment);
    let signatures: Vec<IndexedSignature> = pairs
        .iter()
        .enumerate()
        .take(3)
        .map(|(position, pair)| IndexedSignature {
            signer: position as u32,
            signature: ed_sign(pair, &hash),
        })
        .collect();

    let proof = avalanche::AvalancheProof {
        network_id: AVALANCHE_NETWORK,
        source_chain: AVALANCHE_CHAIN,
        height,
        signatures: BoundedVec::try_from(signatures).expect("three signatures fit"),
    };

    Prepared {
        params: VerifierParameters::Avalanche(params),
        headers: None,
        proof: Proof::Avalanche(proof),
    }
}

// --- The Bitcoin network ------------------------------------------------

/// The marker every Bitcoin fixture writes ahead of the value.
pub const BITCOIN_MARKER: &[u8] = b"CRLA";

/// The packed target every Bitcoin fixture carries.
///
/// The value names a target of seven eighths of the whole range, so a
/// counter meeting the target is found in a few attempts. A test therefore
/// runs the real work check against a real header.
pub const BITCOIN_BITS: u32 = 0x207f_ffff;

/// Build the far network side of one envelope for the Bitcoin network.
pub fn bitcoin_side(envelope: &MessageEnvelope, base: Checkpoint) -> Prepared {
    let commitment = envelope.commitment();
    let transaction = bitcoin::encode_anchor_transaction(
        &[0x5a; 32],
        0,
        BITCOIN_MARKER,
        &commitment,
        50_000,
        &[0x76, 0xa9, 0x14],
    );
    let read = bitcoin::read_transaction(&transaction).expect("the fixture transaction reads");

    let mut leaves: Vec<[u8; 32]> = (0u8..4).map(|held| [held.wrapping_add(1); 32]).collect();
    let position = 1usize;
    let mut internal = read.txid;
    internal.reverse();
    leaves[position] = internal;
    let mut merkle_root = merkle::doubled_root(&leaves).expect("the root builds");
    merkle_root.reverse();

    let mut branch: Vec<[u8; 32]> = Vec::new();
    let mut sibling = leaves[0];
    sibling.reverse();
    branch.push(sibling);
    let mut upper = merkle::doubled_pair(&leaves[2], &leaves[3]);
    upper.reverse();
    branch.push(upper);

    let params = BitcoinParameters {
        checkpoint: base,
        work_limit: [0xff; 32],
        confirmations: 2,
        anchor_marker: BoundedVec::try_from(BITCOIN_MARKER.to_vec()).expect("the marker fits"),
    };

    let first = mine(&params.checkpoint.hash, &merkle_root, 1_700_000_000);
    let first_read = bitcoin::read_header(&first).expect("the fixture header reads");
    let second = mine(&first_read.hash, &[0x2f; 32], 1_700_000_600);

    let batch = bitcoin::BitcoinHeaderBatch {
        headers: BoundedVec::try_from(vec![
            BoundedVec::try_from(first).expect("the header fits"),
            BoundedVec::try_from(second).expect("the header fits"),
        ])
        .expect("two headers fit"),
    };

    let proof = bitcoin::BitcoinProof {
        height: base.height.saturating_add(1),
        transaction: BoundedVec::try_from(transaction).expect("the transaction fits"),
        position: position as u32,
        branch: BoundedVec::try_from(branch).expect("the branch fits"),
        output_index: 0,
    };

    Prepared {
        params: VerifierParameters::Bitcoin(params),
        headers: Some(HeaderBatch::Bitcoin(batch)),
        proof: Proof::Bitcoin(proof),
    }
}

/// Search for a counter that brings one header under its own target.
///
/// The search runs the real work check. A fixture header therefore carries
/// real work rather than a claimed target.
pub fn mine(parent: &[u8; 32], merkle_root: &[u8; 32], moment: u32) -> Vec<u8> {
    let target = bitcoin::unpack_target(BITCOIN_BITS).expect("the target unpacks");
    let mut counter = 0u32;
    loop {
        let encoded = bitcoin::encode_header(parent, merkle_root, moment, BITCOIN_BITS, counter);
        let read = bitcoin::read_header(&encoded).expect("the header reads");
        if bitcoin::meets_target(&read.hash, &target) {
            return encoded;
        }
        counter = counter.saturating_add(1);
    }
}

// --- The Layer 2 rollup -------------------------------------------------

/// The contract on the settlement layer holding the output roots.
pub const ROLLUP_OUTPUT_CONTRACT: [u8; 20] = [0x31; 20];

/// The storage slot the output roots live under.
pub const ROLLUP_OUTPUT_SLOT: [u8; 32] = [0x32; 32];

/// The contract inside the rollup that writes the commitments.
pub const ROLLUP_CONTRACT: [u8; 20] = [0x41; 20];

/// The storage slot the commitments live under inside the rollup.
pub const ROLLUP_SLOT: [u8; 32] = [0x42; 32];

/// The challenge window every rollup fixture carries, in milliseconds.
pub const ROLLUP_WINDOW: u64 = 3_600_000;

/// Build the far network side of one envelope for a Layer 2 rollup.
pub fn rollup_side(envelope: &MessageEnvelope, now_millis: u64, base: Checkpoint) -> Prepared {
    let id = envelope.id();
    let commitment = envelope.commitment();

    let l2_slot = mpt::mapping_slot(&id, &ROLLUP_SLOT);
    let l2_storage_entries = vec![
        (
            sp_crypto_hashing::keccak_256(&l2_slot).to_vec(),
            storage_value(&commitment),
        ),
        (
            sp_crypto_hashing::keccak_256(&[0x43u8; 32]).to_vec(),
            storage_value(&[0x44u8; 32]),
        ),
    ];
    let (l2_storage_root, l2_storage_nodes) = build_trie(&l2_storage_entries);
    let l2_account_entries = vec![
        (
            sp_crypto_hashing::keccak_256(&ROLLUP_CONTRACT).to_vec(),
            account_value(3, &l2_storage_root),
        ),
        (
            sp_crypto_hashing::keccak_256(&[0x45u8; 20]).to_vec(),
            account_value(1, &[0u8; 32]),
        ),
    ];
    let (l2_state_root, l2_account_nodes) = build_trie(&l2_account_entries);

    let withdrawal_root = [0x46u8; 32];
    let l2_block_hash = [0x47u8; 32];
    let version = rollup::OUTPUT_VERSION;
    let root = rollup::output_root(&version, &l2_state_root, &withdrawal_root, &l2_block_hash);

    let landed_seconds = now_millis.saturating_sub(ROLLUP_WINDOW) / 1_000;
    let mut moment_word = [0u8; 32];
    moment_word[24..].copy_from_slice(&landed_seconds.to_be_bytes());

    let output_base = rollup::output_slot(&ROLLUP_OUTPUT_SLOT, 0);
    let moment_slot = rollup::add_word(&output_base, 1);
    let settlement_storage_entries = vec![
        (
            sp_crypto_hashing::keccak_256(&output_base).to_vec(),
            storage_value(&root),
        ),
        (
            sp_crypto_hashing::keccak_256(&moment_slot).to_vec(),
            storage_value(&moment_word),
        ),
    ];
    let (settlement_storage_root, settlement_storage_nodes) =
        build_trie(&settlement_storage_entries);
    let settlement_account_entries = vec![
        (
            sp_crypto_hashing::keccak_256(&ROLLUP_OUTPUT_CONTRACT).to_vec(),
            account_value(5, &settlement_storage_root),
        ),
        (
            sp_crypto_hashing::keccak_256(&[0x48u8; 20]).to_vec(),
            account_value(1, &[0u8; 32]),
        ),
    ];
    let (settlement_state_root, settlement_account_nodes) = build_trie(&settlement_account_entries);

    let height = base.height.saturating_add(1);
    let header = evm::encode_header(&base.hash, &settlement_state_root, &[0u8; 32], height);
    let read = evm::read_header(&header).expect("the fixture header reads");
    let signers = [ed_pair(51), ed_pair(52)];
    let settlement = EvmParameters {
        chain_id: 31_338,
        checkpoint: base,
        contract: ROLLUP_OUTPUT_CONTRACT,
        topic: EVM_TOPIC,
        slot: ROLLUP_OUTPUT_SLOT,
        confirmations: 1,
        header_signers: BoundedVec::try_from(signers.iter().map(ed_key).collect::<Vec<[u8; 32]>>())
            .expect("two keys fit"),
        header_threshold: 2,
    };
    let payload = evm::feed_payload(settlement.chain_id, &read);
    let signatures: Vec<KeyedSignature> = signers
        .iter()
        .map(|pair| KeyedSignature {
            key: ed_key(pair),
            signature: ed_sign(pair, &payload),
        })
        .collect();
    let batch = evm::EvmHeaderBatch {
        headers: BoundedVec::try_from(vec![BoundedVec::try_from(header).expect("the header fits")])
            .expect("one header fits"),
        signatures: BoundedVec::try_from(signatures).expect("two signatures fit"),
    };

    let params = RollupParameters {
        settlement,
        output_contract: ROLLUP_OUTPUT_CONTRACT,
        output_slot: ROLLUP_OUTPUT_SLOT,
        contract: ROLLUP_CONTRACT,
        slot: ROLLUP_SLOT,
        challenge_window_millis: ROLLUP_WINDOW,
    };

    let proof = rollup::RollupProof {
        settlement_height: height,
        output_index: 0,
        output_account: bounded_nodes(&settlement_account_nodes),
        output_storage: bounded_nodes(&settlement_storage_nodes),
        moment_storage: bounded_nodes(&settlement_storage_nodes),
        version,
        l2_state_root,
        withdrawal_root,
        l2_block_hash,
        l2_account: bounded_nodes(&l2_account_nodes),
        l2_storage: bounded_nodes(&l2_storage_nodes),
    };

    Prepared {
        params: VerifierParameters::Rollup(params),
        headers: Some(HeaderBatch::Rollup(batch)),
        proof: Proof::Rollup(proof),
    }
}
