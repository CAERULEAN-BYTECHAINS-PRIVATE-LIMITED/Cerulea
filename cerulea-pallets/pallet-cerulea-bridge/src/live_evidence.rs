//! The module verification of the live bridge evidence.
//!
//! The Cerulea APIs live suite reads each bridge target network through a
//! real public test network node, and reads the Polkadot network through a
//! real development node on the build machine. Each read writes one bundle
//! into `live-evidence/`. The tests here load every bundle and run the
//! verifier functions of this module over it, so the evidence a live driver
//! gathered passes the same code a dispatch runs.
//!
//! Every test also changes one byte of the evidence and requires the check to
//! fail, so a pass means the check read the evidence.
//!
//! | Bundle | What the test verifies |
//! |---|---|
//! | `ethereum-sepolia` | The header hash, the relayer attestation, the account and storage trie walks |
//! | `cosmos-provider` | The header field tree, the validator commit, the two store proofs |
//! | `solana-devnet` | The relayer attestation, the transaction signatures |
//! | `polkadot-westend` | The header hash, the settling votes, the storage read proof |
//! | `avalanche-fuji` | The attestation weight, the account and storage trie walks |
//! | `bitcoin-signet` | The header chain, the work, the merkle branch |
//! | `base-sepolia` | The settlement header, the claim record, the output root |
//! | `polkadot-local` | The whole Polkadot verifier over one message |

use crate::endpoint::{
    reaches_percent, reaches_two_thirds, total_power, total_weight, AuthorityEntry,
    BitcoinParameters, Checkpoint, EvmParameters, ListedHeaders, PolkadotParameters,
    RollupParameters, SignatureScheme, ValidatorEntry, VerifierParameters,
};
use crate::envelope::{BridgeFault, MessageEnvelope, MessageFormat};
use crate::merkle;
use crate::mpt;
use crate::networks::{avalanche, bitcoin, cosmos, evm, polkadot, rollup, solana};
use crate::verify::{
    accepted_header, check_ed25519, confirmations_for, IndexedSignature, KeyedSignature,
    ProofNodes, Verifier,
};
use frame_support::BoundedVec;
use pallet_cerulea_audit::contract::WorkClass;
use serde_json::Value;
use std::path::PathBuf;

/// Load one bundle from the evidence directory of the crate.
fn load(slug: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("live-evidence")
        .join(format!("{slug}.json"));
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("The live evidence bundle {} is absent.", path.display()));
    serde_json::from_str(&text).expect("The live evidence bundle holds valid JSON.")
}

/// Read one hexadecimal text into bytes.
fn bytes(value: &Value) -> Vec<u8> {
    let text = value.as_str().expect("A hexadecimal text.");
    let text = text.strip_prefix("0x").unwrap_or(text);
    (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&text[at..at + 2], 16).expect("A hexadecimal pair."))
        .collect()
}

/// Read one hexadecimal text into a fixed width array.
fn fixed<const N: usize>(value: &Value) -> [u8; N] {
    let held = bytes(value);
    let mut found = [0u8; N];
    found.copy_from_slice(&held);
    found
}

/// Read one whole number, written as a number or as decimal text.
fn number(value: &Value) -> u64 {
    match value {
        Value::Number(held) => held.as_u64().expect("A whole number."),
        Value::String(held) => held.parse().expect("A whole number in decimal."),
        _ => panic!("A whole number."),
    }
}

/// Read one list of hexadecimal texts.
fn list(value: &Value) -> Vec<Vec<u8>> {
    value
        .as_array()
        .expect("A list.")
        .iter()
        .map(bytes)
        .collect()
}

/// Read one list of trie nodes in the bounded shape the module takes.
fn nodes(value: &Value) -> ProofNodes {
    let held: Vec<BoundedVec<u8, _>> = list(value)
        .into_iter()
        .map(|node| BoundedVec::try_from(node).expect("A trie node within bounds."))
        .collect();
    BoundedVec::try_from(held).expect("A node list within bounds.")
}

/// Flip the last bit of one byte string.
fn flipped(held: &[u8]) -> Vec<u8> {
    let mut found = held.to_vec();
    let last = found.len() - 1;
    found[last] ^= 1;
    found
}

/// The header batch and the parameters one attested header takes.
fn attested_header(held: &Value) -> (EvmParameters, evm::EvmHeaderBatch) {
    let params = EvmParameters {
        chain_id: number(&held["chainId"]),
        checkpoint: Checkpoint {
            height: number(&held["checkpoint"]["height"]),
            hash: fixed(&held["checkpoint"]["hash"]),
        },
        contract: [0u8; 20],
        topic: [0u8; 32],
        slot: [0u8; 32],
        confirmations: 1,
        header_signers: BoundedVec::try_from(vec![fixed::<32>(&held["feedKey"])]).unwrap(),
        header_threshold: 1,
    };
    let batch = evm::EvmHeaderBatch {
        headers: BoundedVec::try_from(vec![
            BoundedVec::try_from(bytes(&held["header"])).expect("A header within bounds.")
        ])
        .unwrap(),
        signatures: BoundedVec::try_from(vec![KeyedSignature {
            key: fixed(&held["feedKey"]),
            signature: fixed(&held["feedSignature"]),
        }])
        .unwrap(),
    };
    (params, batch)
}

/// Walk an account proof and a storage proof and answer with the stored word.
fn stored_word(
    state_root: &[u8; 32],
    contract: &[u8; 20],
    account: &Value,
    slot: &[u8; 32],
    storage: &Value,
) -> Result<[u8; 32], BridgeFault> {
    let storage_root = evm::storage_root_of(state_root, contract, &nodes(account))?;
    let key = sp_crypto_hashing::keccak_256(slot);
    let encoded = mpt::require(&storage_root, &key, &list(storage))?;
    mpt::word_of(&encoded)
}

#[test]
fn ethereum_sepolia() {
    let evidence = load("ethereum-sepolia");
    let held = &evidence["bundle"];
    let header = evm::read_header(&bytes(&held["header"])).unwrap();
    assert_eq!(header.hash, fixed::<32>(&held["hash"]));
    assert_eq!(header.number, number(&held["number"]));

    let (params, batch) = attested_header(held);
    let records = evm::accept_headers(&params, &batch, 0).unwrap();
    assert_eq!(records[0].state_root, fixed::<32>(&held["stateRoot"]));

    let word = stored_word(
        &records[0].state_root,
        &fixed(&held["contract"]),
        &held["accountProof"],
        &fixed(&held["slot"]),
        &held["storageProof"],
    )
    .unwrap();
    assert_eq!(word, fixed::<32>(&held["storageValue"]));
    assert_ne!(word, [0u8; 32]);

    let mut wrong = held.clone();
    wrong["slot"] = Value::String(format!("0x{:064x}", 99u32));
    assert!(stored_word(
        &records[0].state_root,
        &fixed(&held["contract"]),
        &held["accountProof"],
        &fixed(&wrong["slot"]),
        &held["storageProof"],
    )
    .is_err());
    let mut forged = held.clone();
    forged["feedKey"] = Value::String(format!("0x{}", "11".repeat(32)));
    let (params, batch) = attested_header(&forged);
    assert!(evm::accept_headers(&params, &batch, 0).is_err());
}

/// Fold one store proof whose leaf step carries its own prefix.
fn fold_store(proof: &Value) -> [u8; 32] {
    let key = bytes(&proof["key"]);
    let value_hash = sp_crypto_hashing::sha2_256(&bytes(&proof["value"]));
    let mut leaf = bytes(&proof["leafPrefix"]);
    leaf.extend_from_slice(&cosmos::varint(key.len() as u64));
    leaf.extend_from_slice(&key);
    leaf.extend_from_slice(&cosmos::varint(value_hash.len() as u64));
    leaf.extend_from_slice(&value_hash);
    let mut current = sp_crypto_hashing::sha2_256(&leaf);
    for step in proof["path"].as_array().unwrap() {
        current = cosmos::inner_hash(&bytes(&step["prefix"]), &current, &bytes(&step["suffix"]));
    }
    current
}

/// Build the module existence proof from one store proof of the map spec.
fn root_spec_proof(proof: &Value) -> cosmos::ExistenceProof {
    let path: Vec<cosmos::InnerStep> = proof["path"]
        .as_array()
        .unwrap()
        .iter()
        .map(|step| cosmos::InnerStep {
            prefix: BoundedVec::try_from(bytes(&step["prefix"])).unwrap(),
            suffix: BoundedVec::try_from(bytes(&step["suffix"])).unwrap(),
        })
        .collect();
    cosmos::ExistenceProof {
        key: BoundedVec::try_from(bytes(&proof["key"])).unwrap(),
        value: BoundedVec::try_from(bytes(&proof["value"])).unwrap(),
        spec: cosmos::ProofSpec::Root,
        path: BoundedVec::try_from(path).unwrap(),
    }
}

#[test]
fn cosmos_provider() {
    let evidence = load("cosmos-provider");
    let held = &evidence["bundle"];
    let block_hash: [u8; 32] = fixed(&held["blockHash"]);
    let app_hash: [u8; 32] = fixed(&held["appHash"]);

    // The header fields fold to the block hash, and the application hash
    // field sits at its place among them.
    let leaves = list(&held["leaves"]);
    assert_eq!(leaves.len(), 14);
    assert_eq!(merkle::prefixed_root(&leaves), block_hash);
    let index = number(&held["appHashIndex"]) as usize;
    let mut expected_leaf = vec![0x0a, 0x20];
    expected_leaf.extend_from_slice(&app_hash);
    assert_eq!(leaves[index], expected_leaf);
    let branch = merkle::prefixed_branch(&leaves, index);
    assert_eq!(
        merkle::prefixed_fold(&leaves[index], index, leaves.len(), &branch),
        Some(block_hash)
    );

    // The validator set signed the block with more than two thirds of its power.
    let validators: Vec<ValidatorEntry> = held["validators"]
        .as_array()
        .unwrap()
        .iter()
        .map(|member| ValidatorEntry {
            key: fixed(&member["key"]),
            power: number(&member["power"]),
        })
        .collect();
    let set = BoundedVec::try_from(validators.clone()).expect("A validator set within bounds.");
    let parts = cosmos::PartHeader {
        total: number(&held["parts"]["total"]) as u32,
        hash: fixed(&held["parts"]["hash"]),
    };
    let chain_id = held["chainId"].as_str().unwrap().as_bytes().to_vec();
    let height = number(&held["height"]);
    let round = number(&held["round"]) as u32;
    let mut signed = 0u64;
    let mut seen: Vec<usize> = Vec::new();
    for vote in held["signatures"].as_array().unwrap() {
        let signer = number(&vote["signer"]) as usize;
        assert!(!seen.contains(&signer));
        seen.push(signer);
        let payload = cosmos::vote_payload(
            &chain_id,
            height,
            round,
            &block_hash,
            &parts,
            number(&vote["seconds"]) as i64,
            number(&vote["nanos"]) as u32,
        );
        let signature: [u8; 64] = fixed(&vote["signature"]);
        assert!(check_ed25519(&validators[signer].key, &payload, &signature));
        assert!(!check_ed25519(
            &validators[signer].key,
            &flipped(&payload),
            &signature
        ));
        signed += validators[signer].power;
    }
    assert!(reaches_two_thirds(signed, total_power(&set)));

    // The supply entry folds to the bank store root, which folds to the
    // application hash through the module existence proof.
    let store_root = fold_store(&held["messageProof"]);
    assert_eq!(store_root.to_vec(), bytes(&held["storeProof"]["value"]));
    assert_eq!(bytes(&held["storeProof"]["leafPrefix"]), vec![0u8]);
    let proof = root_spec_proof(&held["storeProof"]);
    assert!(proof.check(&app_hash).is_ok());
    let mut wrong = proof.clone();
    wrong.value = BoundedVec::try_from(flipped(&wrong.value)).unwrap();
    assert_eq!(wrong.check(&app_hash), Err(BridgeFault::NodeHashMismatch));
}

#[test]
fn solana_devnet() {
    let evidence = load("solana-devnet");
    let held = &evidence["bundle"];
    let slot = number(&held["slot"]);
    let blockhash: [u8; 32] = fixed(&held["blockhash"]);
    assert_eq!(blockhash.to_vec(), bytes(&evidence["hash"]));

    let mut attested = solana::SLOT_DOMAIN.to_vec();
    attested.extend_from_slice(&slot.to_be_bytes());
    attested.extend_from_slice(&blockhash);
    let key: [u8; 32] = fixed(&held["relayerKey"]);
    let signature: [u8; 64] = fixed(&held["relayerSignature"]);
    assert!(check_ed25519(&key, &attested, &signature));
    assert!(!check_ed25519(&key, &flipped(&attested), &signature));

    let transactions = held["transactions"].as_array().unwrap();
    assert!(!transactions.is_empty());
    for transaction in transactions {
        let signer: [u8; 32] = fixed(&transaction["signer"]);
        let signature: [u8; 64] = fixed(&transaction["signature"]);
        let message = bytes(&transaction["message"]);
        assert!(check_ed25519(&signer, &message, &signature));
        assert!(!check_ed25519(&signer, &flipped(&message), &signature));
    }
}

/// The proof one settled Polkadot block carries, in the module shape.
fn polkadot_proof(held: &Value, set_id: u64) -> polkadot::PolkadotProof {
    let signatures: Vec<IndexedSignature> = held["signatures"]
        .as_array()
        .unwrap()
        .iter()
        .map(|vote| IndexedSignature {
            signer: number(&vote["signer"]) as u32,
            signature: fixed(&vote["signature"]),
        })
        .collect();
    polkadot::PolkadotProof {
        header: BoundedVec::try_from(bytes(&held["header"])).unwrap(),
        round: number(&held["round"]),
        set_id,
        signatures: BoundedVec::try_from(signatures).unwrap(),
        nodes: nodes(&held["nodes"]),
        value: BoundedVec::try_from(bytes(&held["value"])).unwrap(),
    }
}

/// The authority set one bundle names.
fn authorities(value: &Value) -> crate::endpoint::AuthoritySet {
    let held: Vec<AuthorityEntry> = value
        .as_array()
        .unwrap()
        .iter()
        .map(|member| AuthorityEntry {
            key: fixed(&member["key"]),
            scheme: SignatureScheme::Ed25519,
            weight: number(&member["weight"]),
        })
        .collect();
    BoundedVec::try_from(held).expect("An authority set within bounds.")
}

#[test]
fn polkadot_westend() {
    let evidence = load("polkadot-westend");
    let held = &evidence["bundle"];
    let set_id = number(&held["setId"]);
    let proof = polkadot_proof(held, set_id);
    let header = polkadot::read_header(proof.header.as_slice()).unwrap();
    assert_eq!(header.hash, fixed::<32>(&held["hash"]));
    assert_eq!(header.number as u64, number(&held["number"]));
    assert_eq!(header.parent, fixed::<32>(&held["checkpoint"]["hash"]));

    let set = authorities(&held["authorities"]);
    let signed = polkadot::check_settlement(&set, &header, &proof).unwrap();
    assert!(reaches_two_thirds(signed, total_weight(&set)));

    let key = bytes(&held["key"]);
    assert!(polkadot::check_storage(&header.state_root, &key, &proof).is_ok());
    let stored = u32::from_le_bytes(proof.value.as_slice().try_into().unwrap());
    assert_eq!(stored, header.number);

    let mut wrong = proof.clone();
    wrong.value = BoundedVec::try_from(flipped(&wrong.value)).unwrap();
    assert!(polkadot::check_storage(&header.state_root, &key, &wrong).is_err());
    let mut other_round = proof.clone();
    other_round.round = other_round.round.saturating_add(1);
    assert_eq!(
        polkadot::check_settlement(&set, &header, &other_round),
        Err(BridgeFault::SignatureInvalid)
    );
}

#[test]
fn avalanche_fuji() {
    let evidence = load("avalanche-fuji");
    let held = &evidence["bundle"];
    let state_root: [u8; 32] = fixed(&held["stateRoot"]);
    let chain_id: [u8; 32] = fixed(&held["chainId"]);
    let network_id = number(&held["networkId"]) as u32;
    let sender = bytes(&held["sender"]);

    let hash = avalanche::message_hash(network_id, &chain_id, &sender, &state_root);
    let set = BoundedVec::try_from(vec![ValidatorEntry {
        key: fixed(&held["attestKey"]),
        power: 1,
    }])
    .unwrap();
    let signatures = [IndexedSignature {
        signer: 0,
        signature: fixed(&held["attestSignature"]),
    }];
    let signed = avalanche::count_weight(&set, &hash, &signatures).unwrap();
    assert!(reaches_percent(
        signed,
        total_power(&set),
        number(&held["thresholdPercent"]) as u8
    ));
    let other = avalanche::message_hash(network_id, &chain_id, &sender, &[0u8; 32]);
    assert_eq!(
        avalanche::count_weight(&set, &other, &signatures),
        Err(BridgeFault::SignatureInvalid)
    );

    let word = stored_word(
        &state_root,
        &fixed(&held["contract"]),
        &held["accountProof"],
        &fixed(&held["slot"]),
        &held["storageProof"],
    )
    .unwrap();
    assert_eq!(word, fixed::<32>(&held["storageValue"]));
    assert!(stored_word(
        &[7u8; 32],
        &fixed(&held["contract"]),
        &held["accountProof"],
        &fixed(&held["slot"]),
        &held["storageProof"],
    )
    .is_err());
}

#[test]
fn bitcoin_signet() {
    let evidence = load("bitcoin-signet");
    let held = &evidence["bundle"];
    let headers: Vec<bitcoin::RawBitcoinHeader> = list(&held["headers"])
        .into_iter()
        .map(|header| BoundedVec::try_from(header).unwrap())
        .collect();
    let params = BitcoinParameters {
        checkpoint: Checkpoint {
            height: number(&held["checkpoint"]["height"]),
            hash: fixed(&held["checkpoint"]["hash"]),
        },
        work_limit: fixed(&held["workLimit"]),
        confirmations: number(&held["confirmations"]) as u32,
        anchor_marker: BoundedVec::default(),
    };
    let batch = bitcoin::BitcoinHeaderBatch {
        headers: BoundedVec::try_from(headers.clone()).unwrap(),
    };
    let records = bitcoin::accept_headers(&params, &batch, 0).unwrap();
    let height = number(&held["height"]);
    assert_eq!(records[0].height, height);
    assert_eq!(records[0].hash.to_vec(), bytes(&evidence["hash"]));

    let proof = bitcoin::BitcoinProof {
        height,
        transaction: BoundedVec::try_from(bytes(&held["transaction"])).unwrap(),
        position: number(&held["position"]) as u32,
        branch: BoundedVec::try_from(
            list(&held["branch"])
                .into_iter()
                .map(|step| {
                    let mut found = [0u8; 32];
                    found.copy_from_slice(&step);
                    found
                })
                .collect::<Vec<[u8; 32]>>(),
        )
        .unwrap(),
        output_index: 0,
    };
    let source = ListedHeaders::many(records.clone());
    let registered = VerifierParameters::Bitcoin(params.clone());
    let state =
        crate::endpoint::EndpointState::new(&registered, MessageFormat::RawBytes, &source, 0);
    let header = accepted_header(&state, height).unwrap();
    let txid: [u8; 32] = fixed(&held["txid"]);
    assert!(bitcoin::check_branch(&txid, &proof, &header.state_root).is_ok());
    assert_eq!(
        confirmations_for(&state, height, params.confirmations),
        Ok(params.confirmations)
    );
    let mut other = txid;
    other[0] ^= 1;
    assert!(bitcoin::check_branch(&other, &proof, &header.state_root).is_err());

    let mut broken = headers;
    broken.swap(1, 2);
    let batch = bitcoin::BitcoinHeaderBatch {
        headers: BoundedVec::try_from(broken).unwrap(),
    };
    assert_eq!(
        bitcoin::accept_headers(&params, &batch, 0),
        Err(BridgeFault::ChainBroken)
    );
}

/// A word holding one whole number.
fn word_of_number(value: u64) -> [u8; 32] {
    let mut found = [0u8; 32];
    found[24..].copy_from_slice(&value.to_be_bytes());
    found
}

#[test]
fn base_sepolia() {
    let evidence = load("base-sepolia");
    let held = &evidence["bundle"];

    // The settlement header, attested above its checkpoint.
    let settlement = &held["settlement"];
    let (params, batch) = attested_header(settlement);
    let records = evm::accept_headers(&params, &batch, 0).unwrap();
    assert_eq!(records[0].hash, fixed::<32>(&settlement["hash"]));

    // The claim record sits in the factory map under the claim identifier.
    let root_claim: [u8; 32] = fixed(&held["rootClaim"]);
    let extra = bytes(&held["extraData"]);
    let mut preimage: Vec<u8> = Vec::new();
    preimage.extend_from_slice(&word_of_number(number(&held["gameType"])));
    preimage.extend_from_slice(&root_claim);
    preimage.extend_from_slice(&word_of_number(0x60));
    preimage.extend_from_slice(&word_of_number(extra.len() as u64));
    let mut padded = extra.clone();
    padded.resize(extra.len().div_ceil(32) * 32, 0);
    preimage.extend_from_slice(&padded);
    let claim_id = sp_crypto_hashing::keccak_256(&preimage);
    let slot = mpt::mapping_slot(&claim_id, &word_of_number(number(&held["mapSlot"])));
    let record = stored_word(
        &records[0].state_root,
        &fixed(&held["factory"]),
        &held["factoryAccountProof"],
        &slot,
        &held["recordProof"],
    )
    .unwrap();
    assert_eq!(record, fixed::<32>(&held["recordWord"]));
    assert_eq!(record[12..].to_vec(), bytes(&held["game"]));
    let moment = u64::from_be_bytes(record[4..12].try_into().unwrap());
    assert_eq!(moment, number(&held["gameMoment"]));

    // The output root rebuilds from the rollup header and the withdrawal root.
    let rollup_header = evm::read_header(&bytes(&held["rollupHeader"])).unwrap();
    assert_eq!(rollup_header.hash, fixed::<32>(&held["rollupHash"]));
    assert_eq!(rollup_header.number, number(&held["rollupNumber"]));
    let withdrawal_root = evm::storage_root_of(
        &rollup_header.state_root,
        &fixed(&held["messagePasser"]),
        &nodes(&held["messagePasserProof"]),
    )
    .unwrap();
    assert_eq!(withdrawal_root, fixed::<32>(&held["withdrawalRoot"]));
    let rebuilt = rollup::output_root(
        &rollup::OUTPUT_VERSION,
        &rollup_header.state_root,
        &withdrawal_root,
        &rollup_header.hash,
    );
    assert_eq!(rebuilt, root_claim);

    // The challenge window runs from the moment the claim landed.
    let window = RollupParameters {
        challenge_window_millis: number(&held["windowSeconds"]).saturating_mul(1_000),
        ..Default::default()
    };
    let landed = moment.saturating_mul(1_000);
    let opens = landed.saturating_add(window.challenge_window_millis);
    assert_eq!(rollup::check_window(&window, moment, opens), Ok(()));
    if window.challenge_window_millis > 0 {
        assert_eq!(
            rollup::check_window(&window, moment, opens - 1),
            Err(BridgeFault::ChallengeWindowOpen)
        );
    }

    let other = rollup::output_root(
        &rollup::OUTPUT_VERSION,
        &rollup_header.state_root,
        &[0u8; 32],
        &rollup_header.hash,
    );
    assert_ne!(other, root_claim);
}

/// Build the envelope one bundle names.
fn envelope_of(value: &Value) -> MessageEnvelope {
    let class = match value["class"].as_str().unwrap() {
        "financial" => WorkClass::Financial,
        "structural" => WorkClass::Structural,
        "constitutional" => WorkClass::Constitutional,
        _ => WorkClass::Routine,
    };
    let name = |field: &str| {
        BoundedVec::try_from(value[field].as_str().unwrap().as_bytes().to_vec()).unwrap()
    };
    MessageEnvelope {
        deployment: fixed(&value["deployment"]),
        endpoint: name("endpoint"),
        source: name("source"),
        destination: name("destination"),
        party: BoundedVec::try_from(bytes(&value["party"])).unwrap(),
        nonce: number(&value["nonce"]),
        payload: BoundedVec::try_from(bytes(&value["payload"])).unwrap(),
        class,
        expiry_millis: number(&value["expiryMillis"]),
    }
}

#[test]
fn polkadot_local_node() {
    let evidence = load("polkadot-local");
    let held = &evidence["bundle"];
    let envelope = envelope_of(&held["envelope"]);
    assert_eq!(envelope.id().to_vec(), bytes(&held["messageId"]));
    assert_eq!(envelope.commitment().to_vec(), bytes(&held["commitment"]));

    let parameters = &held["parameters"];
    let set_id = number(&parameters["setId"]);
    let params = VerifierParameters::Polkadot(PolkadotParameters {
        authorities: authorities(&parameters["authorities"]),
        set_id,
        checkpoint: Checkpoint {
            height: number(&parameters["checkpoint"]["height"]),
            hash: fixed(&parameters["checkpoint"]["hash"]),
        },
        storage_key: BoundedVec::try_from(bytes(&parameters["storageKey"])).unwrap(),
    });
    let proof = polkadot_proof(&held["proof"], set_id);
    let source = ListedHeaders::default();
    let state =
        crate::endpoint::EndpointState::new(&params, MessageFormat::CanonicalValue, &source, 0);
    let verified = polkadot::PolkadotVerifier::verify(&envelope, &proof, &state).unwrap();
    assert_eq!(verified.height, evidence["height"].as_u64().unwrap());
    assert_eq!(verified.block_hash.to_vec(), bytes(&evidence["hash"]));

    let mut other = envelope.clone();
    other.nonce = other.nonce.saturating_add(1);
    assert!(polkadot::PolkadotVerifier::verify(&other, &proof, &state).is_err());
}
