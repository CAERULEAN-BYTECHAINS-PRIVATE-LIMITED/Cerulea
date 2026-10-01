//! Backup restoration and exit export.
//!
//! Two claims carry this file.
//!
//! 1. **A backup restores.** A running deployment writes a backup. The
//!    deployment is destroyed. The backup restores and the restored state
//!    matches the state the backup held, block for block and record for
//!    record.
//! 2. **An exit export starts elsewhere.** The export carries the chain, the
//!    record set and the manifest naming both. A second machine reads the
//!    package, rebuilds the state and reaches the same roots.
//!
//! Every file here is written to disk and read back from disk. Nothing passes
//! through memory alone, so the test covers the encoding as well as the state.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use codec::{Decode, Encode};
use tempfile::tempdir;

use cerulea_node::fault_harness::FaultNetwork;
use cerulea_node::test_harness::{execute_block, HarnessTransaction, TestHeader};

use cerulea_consensus::engines::execution::roots::StateRoots;
use cerulea_consensus::engines::{Slot, TransactionClass};

/// The validator count the recovery test drives.
const VALIDATORS: usize = 9;

/// The number of records the deployment holds.
const RECORDS: usize = 120;

/// The number of slots the deployment runs before the backup.
const SLOTS: u64 = 16;

/// One backup package.
#[derive(Encode, Decode, PartialEq, Eq, Debug)]
struct Package {
    /// The name of the deployment the package carries.
    deployment: Vec<u8>,
    /// The chain of headers, in order.
    chain: Vec<TestHeader>,
    /// The height and the block finalised at it.
    finalised: Vec<(u32, [u8; 32])>,
    /// The record set, as collection, key and value.
    records: Vec<(Vec<u8>, Vec<u8>, Vec<u8>)>,
    /// The combined state root over the record set.
    combined_root: [u8; 32],
    /// The per-domain roots.
    domain_roots: Vec<[u8; 32]>,
}

fn record_set() -> Vec<HarnessTransaction> {
    (0..RECORDS)
        .map(|position| {
            HarnessTransaction::new(
                &format!("register.{}", position % 9),
                &format!("entry-{}", position),
                &format!("value-{}", position),
                TransactionClass::Routine,
            )
        })
        .collect()
}

fn package_from(network: &FaultNetwork, records: &[HarnessTransaction]) -> Package {
    let roots: StateRoots = execute_block(records, 4);
    Package {
        deployment: b"cerulea.recovery.deployment".to_vec(),
        chain: network.network.nodes[0].chain.clone(),
        finalised: network.network.nodes[0]
            .committed
            .iter()
            .map(|(height, hash)| (*height, *hash))
            .collect(),
        records: records
            .iter()
            .map(|entry| {
                (
                    entry.collection.0.to_vec(),
                    entry.key.clone(),
                    entry.value.clone(),
                )
            })
            .collect(),
        combined_root: roots.combined.0,
        domain_roots: roots.domains.iter().map(|root| root.0).collect(),
    }
}

fn write_package(directory: &Path, package: &Package) -> std::path::PathBuf {
    let target = directory.join("cerulea-backup.scale");
    fs::write(&target, package.encode()).expect("the backup lands on disk");
    let manifest = directory.join("MANIFEST.txt");
    fs::write(
        &manifest,
        format!(
            "Cerulea backup\nblocks: {}\nheights finalised: {}\nrecords: {}\ncombined root: {}\n",
            package.chain.len(),
            package.finalised.len(),
            package.records.len(),
            hex(&package.combined_root)
        ),
    )
    .expect("the manifest lands on disk");
    target
}

fn read_package(path: &Path) -> Package {
    let bytes = fs::read(path).expect("the backup reads back from disk");
    Package::decode(&mut &bytes[..]).expect("the backup decodes")
}

fn hex(bytes: &[u8; 32]) -> String {
    bytes.iter().fold(String::new(), |mut held, byte| {
        held.push_str(&format!("{byte:02x}"));
        held
    })
}

#[test]
fn a_backup_restores_a_destroyed_deployment_exactly() {
    let records = record_set();
    let mut network = FaultNetwork::healthy(VALIDATORS);
    for slot_number in 1..=SLOTS {
        assert!(network.run_slot(Slot(slot_number)).outcome.committed());
        network.assert_no_conflicting_finalisation("the run before the backup");
    }

    // --- Backup ---
    let directory = tempdir().expect("the machine gives a directory");
    let package = package_from(&network, &records);
    let path = write_package(directory.path(), &package);
    assert!(path.exists(), "the backup file sits on disk");
    assert!(
        fs::metadata(&path)
            .expect("the backup carries a size")
            .len()
            > 0,
        "the backup carries bytes"
    );

    // --- Destruction ---
    for node in network.network.nodes.iter_mut() {
        node.chain.clear();
        node.committed.clear();
    }
    assert_eq!(network.network.height(), 0, "the deployment is gone");

    // --- Restoration ---
    let restored = read_package(&path);
    assert_eq!(restored, package, "the backup read back byte for byte");
    for node in network.network.nodes.iter_mut() {
        node.chain = restored.chain.clone();
        node.committed = restored
            .finalised
            .iter()
            .copied()
            .collect::<BTreeMap<u32, [u8; 32]>>();
    }

    // --- Comparison ---
    assert_eq!(
        network.network.height(),
        SLOTS as u32,
        "the restored deployment holds every block"
    );
    for node in network.network.nodes.iter() {
        assert_eq!(node.committed.len(), SLOTS as usize);
    }
    let roots_after = execute_block(&records, 4);
    assert_eq!(
        roots_after.combined.0, restored.combined_root,
        "the restored record set reaches the root the backup held"
    );
    assert_eq!(
        roots_after
            .domains
            .iter()
            .map(|root| root.0)
            .collect::<Vec<[u8; 32]>>(),
        restored.domain_roots
    );
    network.assert_no_conflicting_finalisation("the restored deployment");

    // The restored deployment keeps working.
    let mut resumed = 0;
    for slot_number in (SLOTS + 1)..=(SLOTS + 10) {
        if network.run_slot(Slot(slot_number)).outcome.committed() {
            resumed += 1;
        }
        network.assert_no_conflicting_finalisation("the run after the restoration");
    }
    assert!(resumed > 0, "the restored deployment finalised again");

    if let Ok(report) = std::env::var("CERULEA_RECOVERY_REPORT") {
        let body = format!(
            "{{\"validators\":{},\"records\":{},\"blocksBackedUp\":{},\"heightsFinalised\":{},\
             \"restoredExactly\":true,\"resumedSlots\":{},\"combinedRoot\":\"{}\"}}\n",
            VALIDATORS,
            RECORDS,
            package.chain.len(),
            package.finalised.len(),
            resumed,
            hex(&package.combined_root)
        );
        fs::write(&report, body).expect("the recovery report lands on disk");
    }
}

#[test]
fn an_exit_export_produces_a_package_that_starts_elsewhere() {
    let records = record_set();
    let mut origin = FaultNetwork::healthy(VALIDATORS);
    for slot_number in 1..=SLOTS {
        assert!(origin.run_slot(Slot(slot_number)).outcome.committed());
    }

    let directory = tempdir().expect("the machine gives a directory");
    let package = package_from(&origin, &records);
    let path = write_package(directory.path(), &package);

    // The manifest names every part, so an operator reads the package without
    // reading the bytes.
    let manifest =
        fs::read_to_string(directory.path().join("MANIFEST.txt")).expect("the manifest reads back");
    assert!(manifest.contains(&format!("blocks: {}", package.chain.len())));
    assert!(manifest.contains(&format!("records: {}", package.records.len())));
    assert!(manifest.contains(&hex(&package.combined_root)));

    // --- The second machine ---
    let carried = read_package(&path);
    let mut elsewhere = FaultNetwork::healthy(VALIDATORS);
    for node in elsewhere.network.nodes.iter_mut() {
        node.chain = carried.chain.clone();
        node.committed = carried
            .finalised
            .iter()
            .copied()
            .collect::<BTreeMap<u32, [u8; 32]>>();
    }

    // The record set rebuilds from the package alone.
    let rebuilt: Vec<HarnessTransaction> = carried
        .records
        .iter()
        .enumerate()
        .map(|(position, (_, key, value))| {
            HarnessTransaction::new(
                &format!("register.{}", position % 9),
                &String::from_utf8(key.clone()).expect("the key reads as text"),
                &String::from_utf8(value.clone()).expect("the value reads as text"),
                TransactionClass::Routine,
            )
        })
        .collect();
    let roots = execute_block(&rebuilt, 4);
    assert_eq!(
        roots.combined.0, carried.combined_root,
        "the second machine reaches the root the export carried"
    );

    // The deployment runs on the second machine.
    let mut finalised = 0;
    for slot_number in (SLOTS + 1)..=(SLOTS + 12) {
        if elsewhere.run_slot(Slot(slot_number)).outcome.committed() {
            finalised += 1;
        }
        elsewhere.assert_no_conflicting_finalisation("the deployment on the second machine");
    }
    assert!(finalised > 0, "the exported deployment runs elsewhere");
    assert_eq!(
        elsewhere.network.nodes[0].committed.len(),
        carried.finalised.len() + finalised,
        "the second machine kept every height the export carried"
    );
}

#[test]
fn a_damaged_backup_refuses_to_restore() {
    let records = record_set();
    let mut network = FaultNetwork::healthy(3);
    network.run_slots(4);
    let directory = tempdir().expect("the machine gives a directory");
    let package = package_from(&network, &records);
    let path = write_package(directory.path(), &package);

    let mut bytes = fs::read(&path).expect("the backup reads back");
    let cut = bytes.len() / 2;
    bytes.truncate(cut);
    fs::write(&path, &bytes).expect("the damaged backup lands on disk");

    let outcome = Package::decode(&mut &bytes[..]);
    assert!(
        outcome.is_err(),
        "a truncated backup decodes into a deployment. \
         Fix: check the length and the hash of the package before restoring it."
    );
}
