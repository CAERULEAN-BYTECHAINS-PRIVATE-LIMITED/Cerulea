//! Custody challenges answered on a network of real validators.
//!
//! Three validator nodes run on loopback from the genesis of the high stake
//! chain. A steady stream of records lands on chain through the governance
//! account, so the Custody Engine has history to challenge. Every ten blocks
//! the chain challenges one validator for one record of one earlier block.
//! The named validator reads the entries of that block from its local store,
//! builds the inclusion proof and submits the signed answer inside the three
//! block deadline. The test asserts that every challenge issued is answered
//! and verified, none lapses or faults, and the custody share of the Trust
//! Score moves.

mod loopback;

use cerulea_node::custody_answer::transaction_parts;
use cerulea_runtime as runtime;
use codec::{Decode, Encode};
use loopback::{request, Network};
use sp_core::crypto::Ss58Codec;
use sp_core::Pair;
use sp_keyring::Ed25519Keyring;
use std::time::{Duration, Instant};

/// The validators of the high stake chain.
const NAMES: [&str; 3] = ["alice", "bob", "charlie"];

/// The challenges the test waits for.
const CHALLENGES: u32 = 3;

fn hash_of(value: &serde_json::Value) -> Option<runtime::Hash> {
    let text = value.as_str()?;
    let bytes = hex::decode(text.trim_start_matches("0x")).ok()?;
    (bytes.len() == 32).then(|| runtime::Hash::from_slice(&bytes))
}

/// Submit one record through the governance account, signed by Alice.
fn submit_record(port: u16, sequence: u32) -> Result<(), String> {
    let alice = Ed25519Keyring::Alice;
    let account: runtime::AccountId = alice.to_account_id();
    let nonce = request(
        port,
        "system_accountNextIndex",
        &format!(r#"["{}"]"#, account.to_ss58check()),
    )
    .and_then(|reply| reply["result"].as_u64())
    .ok_or("the nonce failed to read")? as u32;
    let genesis = request(port, "chain_getBlockHash", "[0]")
        .and_then(|reply| hash_of(&reply["result"]))
        .ok_or("the genesis hash failed to read")?;
    let best = request(port, "chain_getBlockHash", "[]")
        .and_then(|reply| hash_of(&reply["result"]))
        .ok_or("the best hash failed to read")?;
    let best_number = loopback::best_height(port).ok_or("the best height failed to read")?;

    let record = runtime::RuntimeCall::Custody(pallet_cerulea_custody::Call::commit_record {
        collection: [7u8; 16],
        key: sequence
            .to_be_bytes()
            .to_vec()
            .try_into()
            .expect("a short key"),
        value: format!("custody record {sequence}")
            .into_bytes()
            .try_into()
            .expect("a short value"),
    });
    let call = runtime::RuntimeCall::Sudo(pallet_sudo::Call::sudo {
        call: Box::new(record),
    });
    let (payload, extension) = transaction_parts(call.clone(), nonce, genesis, best, best_number);
    let signature = payload.using_encoded(|bytes| alice.pair().sign(bytes));
    let extrinsic = runtime::UncheckedExtrinsic::new_signed(
        call,
        account.into(),
        runtime::Signature::Ed25519(signature),
        extension,
    );
    let reply = request(
        port,
        "author_submitExtrinsic",
        &format!(r#"["0x{}"]"#, hex::encode(extrinsic.encode())),
    )
    .ok_or("the submission failed to reach the node")?;
    if reply["result"].is_string() {
        Ok(())
    } else {
        Err(format!("the node refused the record: {}", reply))
    }
}

/// The custody figures of one validator: share, answered, missed, faults.
fn custody_of(port: u16, name: &str) -> Option<(u64, u64, u64, u64)> {
    let keyring = match name {
        "alice" => Ed25519Keyring::Alice,
        "bob" => Ed25519Keyring::Bob,
        _ => Ed25519Keyring::Charlie,
    };
    let account = keyring.to_account_id().to_ss58check();
    let reply = request(
        port,
        "cerulea_getValidatorDetail",
        &format!(r#"["{account}"]"#),
    )?;
    let detail = &reply["result"];
    Some((
        detail["custody"].as_u64()?,
        detail["custodyAnswered"].as_u64()?,
        detail["custodyMissed"].as_u64()?,
        detail["custodyFaults"].as_u64()?,
    ))
}

#[test]
fn three_validators_answer_every_custody_challenge() {
    // The folder stays on disk when the test fails, so the node logs can be
    // read; the test names it in its failure message.
    let folder = tempfile::Builder::new()
        .prefix("cerulea-custody-")
        .tempdir()
        .expect("a temporary folder opens");
    let kept = folder.path().to_path_buf();
    // The transaction pool and the transaction protocol log at debug, so a
    // failure shows where an answer stopped on its way into a block.
    let mut network = Network::start_on(folder.path(), "high_stake", &NAMES, |_| {
        vec![("RUST_LOG", "txpool=debug,sync=debug".to_string())]
    });
    let port = network.nodes[0].rpc;

    // The chain runs before the records start.
    let started = Instant::now();
    while loopback::best_height(port).unwrap_or(0) < 3 {
        assert!(
            started.elapsed() < Duration::from_secs(120),
            "the chain produced no blocks\n{}",
            network.report()
        );
        std::thread::sleep(Duration::from_secs(1));
    }
    let before: Vec<_> = NAMES
        .iter()
        .map(|name| custody_of(port, name).expect("the validator detail reads"))
        .collect();

    // Records land while the challenges run, until enough challenges settle.
    let mut sequence = 0u32;
    let mut accepted = 0u32;
    let mut refused: Vec<String> = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(420);
    let after: Vec<(u64, u64, u64, u64)> = loop {
        match submit_record(port, sequence) {
            Ok(()) => accepted += 1,
            Err(reason) => refused.push(format!("record {sequence}: {reason}")),
        }
        sequence += 1;
        std::thread::sleep(Duration::from_secs(3));
        let now: Vec<_> = NAMES
            .iter()
            .map(|name| custody_of(port, name).unwrap_or((0, 0, 0, 0)))
            .collect();
        let settled: u64 = now
            .iter()
            .map(|(_, answered, missed, faults)| answered + missed + faults)
            .sum();
        if settled >= CHALLENGES as u64 || Instant::now() > deadline {
            break now;
        }
    };
    // The chain state behind the figures: the newest block holding records
    // and the number of challenges issued so far.
    let history_head = loopback::storage_value(port, b"Custody", b"HistoryHead")
        .and_then(|bytes| u32::decode(&mut &bytes[..]).ok());
    let challenges_issued = loopback::storage_value(port, b"Custody", b"NextChallengeId")
        .and_then(|bytes| u64::decode(&mut &bytes[..]).ok());
    let best = loopback::best_height(port);
    network.stop();

    let answered: u64 = after.iter().map(|figures| figures.1).sum();
    let missed: u64 = after.iter().map(|figures| figures.2).sum();
    let faults: u64 = after.iter().map(|figures| figures.3).sum();
    let state = format!(
        "records accepted {accepted}, refused {} ({}), best block {best:?}, history head {history_head:?}, challenges issued {challenges_issued:?}; node logs kept under {}",
        refused.len(),
        refused.iter().take(3).cloned().collect::<Vec<_>>().join("; "),
        kept.display()
    );
    let green = answered >= CHALLENGES as u64 && missed == 0 && faults == 0;
    if !green {
        // Keep the folder for the reader of the failure.
        std::mem::forget(folder);
    }
    assert!(
        answered >= CHALLENGES as u64,
        "answered {answered}, missed {missed}, faults {faults}; before {before:?} after {after:?}\n{state}\n{}\n{}",
        loopback::lines_with(&network.nodes, "Custody Engine"),
        network.report()
    );
    assert_eq!(
        missed,
        0,
        "a challenge lapsed: {after:?}\n{state}\n{}",
        loopback::lines_with(&network.nodes, "Custody Engine")
    );
    assert_eq!(faults, 0, "a proof failed: {after:?}\n{state}");
    assert!(
        before
            .iter()
            .zip(after.iter())
            .any(|(was, now)| was.0 != now.0),
        "the custody share stayed at {before:?}"
    );
}
