//! State writes reach the Custody Engine record tree.
//!
//! The Custody Engine challenges validators to serve records from the
//! committed history. A record enters the record tree of its block when the
//! state module commits the record, through the custody record hook. These
//! tests drive a real Blueprint action on the development runtime, then run
//! the custody hooks by hand and answer the challenge the engine issues.
//!
//! The chain here holds state module records alone. No record enters through
//! the direct custody call.

mod support;

use cerulea_runtime::{Runtime, RuntimeEvent, RuntimeOrigin};
use frame_support::assert_ok;
use frame_support::traits::{OnFinalize, OnInitialize};
use frame_system::pallet_prelude::BlockNumberFor;
use pallet_cerulea_custody::primitives::ChallengeState;
use pallet_cerulea_custody::respond::{assemble_answer, local_record_root};
use pallet_cerulea_custody::verify::verify_against_root;
use pallet_cerulea_custody::{custody_entry, PendingLeaves, PublishedChallenge, RecordRoots};
use pallet_cerulea_schema::primitives::CollectionKey;
use support::*;

/// The fragment the tests install.
const FRAGMENT: &str = "188-land-title-registry.json";

/// The action the tests run.
const ACTION: &str = "land_title_registry_record_parcel";

type Custody = pallet_cerulea_custody::Pallet<Runtime>;

/// The block number type of the runtime.
fn block(number: u32) -> BlockNumberFor<Runtime> {
    number
}

/// The custody entries the state module committed, in commit order.
///
/// A validator rebuilds the same list from its own copy of the block. The
/// list reads the state events in order and the stored bytes of each record.
fn committed_entries() -> Vec<Vec<u8>> {
    frame_system::Pallet::<Runtime>::events()
        .into_iter()
        .filter_map(|record| match record.event {
            RuntimeEvent::State(pallet_cerulea_state::Event::RecordInserted {
                deployment,
                collection,
                key,
                ..
            })
            | RuntimeEvent::State(pallet_cerulea_state::Event::RecordUpdated {
                deployment,
                collection,
                key,
                ..
            }) => {
                let name = frame_support::BoundedVec::try_from(collection.into_inner())
                    .expect("The collection name fits.");
                let slot = CollectionKey::new(deployment, name);
                let bytes = pallet_cerulea_state::Records::<Runtime>::get(&slot, &key)
                    .expect("The store holds the record.");
                Some(custody_entry(
                    &codec::Encode::encode(&slot),
                    key.as_slice(),
                    Some(bytes.as_slice()),
                ))
            }
            _ => None,
        })
        .collect()
}

/// Install the fragment and write one record through the state module.
fn write_one_parcel() {
    let library = fragment_library();
    assert!(
        !library.is_empty(),
        "The content repository sits beside the chain. Clone it before the run."
    );
    let blueprint = resolve(FRAGMENT, &library).expect("The fragment resolves.");
    let deployment = install(&blueprint).expect("The Blueprint installs.");
    seed_identity_and_permissions(deployment, &blueprint);
    let action = blueprint
        .actions
        .iter()
        .find(|held| held.id.as_str() == ACTION)
        .expect("The fragment declares the action.");
    let arguments =
        sample_arguments(action, &blueprint.types).expect("Every parameter takes a value.");
    run_action(deployment, action, &arguments).expect("The registrar records one parcel.");
}

/// A record written through the state module enters the record tree.
#[test]
fn a_state_write_enters_the_custody_record_tree() {
    development_ext().execute_with(|| {
        assert!(PendingLeaves::<Runtime>::get().is_empty());
        write_one_parcel();

        let entries = committed_entries();
        assert!(!entries.is_empty(), "The action committed a record.");
        let leaves = PendingLeaves::<Runtime>::get();
        assert_eq!(
            leaves.len(),
            entries.len(),
            "Every committed record added one leaf."
        );
        for (position, entry) in entries.iter().enumerate() {
            assert_eq!(
                leaves[position],
                pallet_cerulea_custody::verify::leaf_hash(position as u32, entry),
                "Leaf {position} carries the entry of the committed record."
            );
        }

        <Custody as OnFinalize<BlockNumberFor<Runtime>>>::on_finalize(block(1));
        assert_eq!(
            RecordRoots::<Runtime>::get(block(1)),
            Some(local_record_root(&entries)),
            "The block root covers the state records."
        );
    });
}

/// A challenge issues over state records and the validator answers it.
///
/// The chain holds state module records alone. The engine issues a challenge
/// into that history. The challenged validator rebuilds the entries of the
/// block, assembles the proof and answers inside the deadline.
#[test]
fn a_challenge_over_state_records_is_answered_with_a_proof_that_verifies() {
    development_ext().execute_with(|| {
        write_one_parcel();
        let entries = committed_entries();
        <Custody as OnFinalize<BlockNumberFor<Runtime>>>::on_finalize(block(1));
        let root = RecordRoots::<Runtime>::get(block(1)).expect("The block holds a root.");

        frame_system::Pallet::<Runtime>::set_block_number(block(10));
        <Custody as OnInitialize<BlockNumberFor<Runtime>>>::on_initialize(block(10));
        let (id, challenge) =
            PublishedChallenge::<Runtime>::get().expect("A challenge issued over state records.");
        assert_eq!(challenge.record.block, block(1));

        let answer = assemble_answer(&entries, challenge.record.index)
            .expect("The validator assembles the answer from its copy.");
        assert_eq!(
            verify_against_root(
                root,
                challenge.record.index,
                answer.value.as_slice(),
                answer.proof.as_slice()
            ),
            Ok(()),
            "The proof folds to the block root."
        );
        assert_ok!(Custody::answer_challenge(
            RuntimeOrigin::signed(challenge.target.clone()),
            id,
            answer
        ));
        assert_eq!(
            pallet_cerulea_custody::ChallengeStates::<Runtime>::get(id),
            Some(ChallengeState::AnsweredInTime)
        );
    });
}
