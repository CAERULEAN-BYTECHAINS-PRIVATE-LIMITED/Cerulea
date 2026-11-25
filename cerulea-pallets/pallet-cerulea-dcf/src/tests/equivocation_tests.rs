//! On-chain tests for equivocation detection and penalty.
//!
//! Equivocation means one validator sealing two different headers for one slot
//! at one rank. A slot is one fixed span of time. These tests build real sealed
//! heads with a real signing key, submit the evidence extrinsic and read the
//! effect out of pallet storage.

use crate::engines::standing::{FaultKey, Misconduct};
use crate::mock::*;
use crate::{
    EquivocationFaults, Error, Event, PenalisedEquivocations, ServiceRecords, StandingResets,
    StandingShares,
};
use cerulea_primitives::equivocation::{
    EquivocationProof, SealClaimBytes, SealedHead, DRAW_KEY_OFFSET, RANK_OFFSET, SEAL_CLAIM_BYTES,
    SEAL_KEY_OFFSET, SLOT_OFFSET,
};
use frame_support::{assert_noop, assert_ok};
use sp_core::ed25519::Pair;
use sp_core::Pair as PairTrait;

/// The account the tests report against.
const OFFENDER: u64 = 1;

/// The account that submits the evidence.
const REPORTER: u64 = 2;

/// The draw key the offender registered.
const DRAW_KEY: [u8; 32] = [7u8; 32];

fn author_pair() -> Pair {
    Pair::from_seed(&[42u8; 32])
}

/// Build one encoded seal claim the way the Authoring Engine encodes it.
fn claim_bytes(slot: u64, rank: u8, seal_key: [u8; 32]) -> SealClaimBytes {
    let mut claim = [0u8; SEAL_CLAIM_BYTES];
    claim[SLOT_OFFSET..SLOT_OFFSET + 8].copy_from_slice(&slot.to_le_bytes());
    claim[RANK_OFFSET] = rank;
    claim[SEAL_KEY_OFFSET..SEAL_KEY_OFFSET + 32].copy_from_slice(&seal_key);
    claim[DRAW_KEY_OFFSET..DRAW_KEY_OFFSET + 32].copy_from_slice(&DRAW_KEY);
    claim
}

/// Seal one head with the author key.
fn head(pair: &Pair, pre_seal_hash: [u8; 32], slot: u64, rank: u8) -> SealedHead {
    let mut sealed = SealedHead::new(
        pre_seal_hash,
        claim_bytes(slot, rank, pair.public().0),
        [0u8; 64],
    );
    sealed.signature = pair.sign(&sealed.payload()).0;
    sealed
}

/// Two headers one validator sealed for one slot at one rank.
fn proof(pair: &Pair, slot: u64, rank: u8) -> EquivocationProof {
    EquivocationProof::new(
        head(pair, [1u8; 32], slot, rank),
        head(pair, [2u8; 32], slot, rank),
    )
}

/// Seed the registry with the two keys of the offender.
fn seed_keys(pair: &Pair) {
    registry_record_keys(OFFENDER, DRAW_KEY, pair.public().0);
}

/// Give the offender a service history worth destroying.
fn seed_service(epochs: u32) {
    let mut record = crate::engines::standing::enter(0);
    for epoch in 1..=epochs {
        crate::engines::standing::record_epoch(
            &mut record,
            epoch,
            crate::engines::standing::ServiceEvidence {
                custody_held: true,
                participated: true,
                sampling_accurate: true,
            },
        );
    }
    ServiceRecords::<Test>::insert(OFFENDER, record);
    StandingShares::<Test>::insert(OFFENDER, sp_runtime::Perbill::from_percent(80));
}

/// Design assertion 2 of the security domain.
///
/// Equivocation is detected and penalised within one epoch.
#[test]
fn proven_equivocation_returns_standing_to_zero_inside_one_epoch() {
    new_test_ext().execute_with(|| {
        let pair = author_pair();
        seed_keys(&pair);
        seed_service(90);
        let epoch_at_report = DcfPallet::current_epoch();

        assert_ok!(DcfPallet::report_equivocation(
            RuntimeOrigin::signed(REPORTER),
            Box::new(proof(&pair, 9, 1))
        ));

        assert_eq!(
            StandingShares::<Test>::get(OFFENDER),
            Some(sp_runtime::Perbill::zero())
        );
        assert_eq!(ServiceRecords::<Test>::get(OFFENDER).epochs_served, 0);

        let reset = StandingResets::<Test>::get(OFFENDER).expect("the chain recorded the reset");
        assert_eq!(reset.misconduct, Misconduct::TwoBlocksAtOneSlot);
        assert_eq!(reset.epochs_destroyed, 90);
        assert_eq!(
            reset.epoch, epoch_at_report,
            "the penalty lands in the epoch the report arrived"
        );
    });
}

#[test]
fn the_chain_records_the_fault_against_the_validator() {
    new_test_ext().execute_with(|| {
        let pair = author_pair();
        seed_keys(&pair);
        seed_service(40);

        assert_ok!(DcfPallet::report_equivocation(
            RuntimeOrigin::signed(REPORTER),
            Box::new(proof(&pair, 21, 3))
        ));

        let fault = EquivocationFaults::<Test>::get(OFFENDER).expect("the chain recorded a fault");
        assert_eq!(fault.slot, 21);
        assert_eq!(fault.rank, 3);
        assert_eq!(fault.first_hash, [1u8; 32]);
        assert_eq!(fault.second_hash, [2u8; 32]);
        assert_eq!(fault.epochs_destroyed, 40);

        let key = FaultKey::new(21, 3, pair.public().0);
        assert_eq!(
            PenalisedEquivocations::<Test>::get(key),
            Some(fault),
            "the chain remembers the equivocation it penalised"
        );
    });
}

#[test]
fn the_chain_raises_the_equivocation_event() {
    new_test_ext().execute_with(|| {
        frame_system::Pallet::<Test>::set_block_number(5);
        let pair = author_pair();
        seed_keys(&pair);
        seed_service(12);

        assert_ok!(DcfPallet::report_equivocation(
            RuntimeOrigin::signed(REPORTER),
            Box::new(proof(&pair, 4, 1))
        ));

        let raised = frame_system::Pallet::<Test>::events()
            .into_iter()
            .any(|record| {
                matches!(
                    record.event,
                    RuntimeEvent::DcfPallet(Event::EquivocationProven {
                        validator: OFFENDER,
                        slot: 4,
                        rank: 1,
                        epochs_destroyed: 12,
                    })
                )
            });
        assert!(raised, "the chain names the validator and the slot");
    });
}

#[test]
fn one_equivocation_takes_one_penalty() {
    new_test_ext().execute_with(|| {
        let pair = author_pair();
        seed_keys(&pair);
        seed_service(30);

        assert_ok!(DcfPallet::report_equivocation(
            RuntimeOrigin::signed(REPORTER),
            Box::new(proof(&pair, 9, 1))
        ));
        assert_noop!(
            DcfPallet::report_equivocation(
                RuntimeOrigin::signed(REPORTER),
                Box::new(proof(&pair, 9, 1))
            ),
            Error::<Test>::EquivocationAlreadyPenalised
        );
    });
}

#[test]
fn one_header_reported_twice_carries_no_penalty() {
    new_test_ext().execute_with(|| {
        let pair = author_pair();
        seed_keys(&pair);
        seed_service(30);
        let sealed = head(&pair, [1u8; 32], 9, 1);

        assert_noop!(
            DcfPallet::report_equivocation(
                RuntimeOrigin::signed(REPORTER),
                Box::new(EquivocationProof::new(sealed, sealed))
            ),
            Error::<Test>::EquivocationEvidenceRejected
        );
        assert_eq!(ServiceRecords::<Test>::get(OFFENDER).epochs_served, 30);
    });
}

#[test]
fn a_broken_signature_carries_no_penalty() {
    new_test_ext().execute_with(|| {
        let pair = author_pair();
        seed_keys(&pair);
        seed_service(30);
        let mut evidence = proof(&pair, 9, 1);
        evidence.second.signature[0] ^= 0xff;

        assert_noop!(
            DcfPallet::report_equivocation(RuntimeOrigin::signed(REPORTER), Box::new(evidence)),
            Error::<Test>::EquivocationEvidenceRejected
        );
        assert!(EquivocationFaults::<Test>::get(OFFENDER).is_none());
    });
}

#[test]
fn two_headers_at_two_slots_carry_no_penalty() {
    new_test_ext().execute_with(|| {
        let pair = author_pair();
        seed_keys(&pair);
        seed_service(30);
        let evidence =
            EquivocationProof::new(head(&pair, [1u8; 32], 9, 1), head(&pair, [2u8; 32], 10, 1));

        assert_noop!(
            DcfPallet::report_equivocation(RuntimeOrigin::signed(REPORTER), Box::new(evidence)),
            Error::<Test>::EquivocationEvidenceRejected
        );
    });
}

#[test]
fn evidence_against_an_unregistered_key_carries_no_penalty() {
    new_test_ext().execute_with(|| {
        let pair = author_pair();
        seed_service(30);

        assert_noop!(
            DcfPallet::report_equivocation(
                RuntimeOrigin::signed(REPORTER),
                Box::new(proof(&pair, 9, 1))
            ),
            Error::<Test>::EquivocationAuthorUnregistered
        );
    });
}

#[test]
fn evidence_naming_another_seal_key_carries_no_penalty() {
    new_test_ext().execute_with(|| {
        let pair = author_pair();
        // The registry holds a different seal key against the draw key.
        registry_record_keys(OFFENDER, DRAW_KEY, [3u8; 32]);
        seed_service(30);

        assert_noop!(
            DcfPallet::report_equivocation(
                RuntimeOrigin::signed(REPORTER),
                Box::new(proof(&pair, 9, 1))
            ),
            Error::<Test>::EquivocationKeysMismatch
        );
    });
}

#[test]
fn any_account_submits_the_evidence() {
    new_test_ext().execute_with(|| {
        let pair = author_pair();
        seed_keys(&pair);
        seed_service(15);

        // The reporter holds no seat and still carries the evidence through.
        assert_ok!(DcfPallet::report_equivocation(
            RuntimeOrigin::signed(99),
            Box::new(proof(&pair, 9, 1))
        ));
        assert!(EquivocationFaults::<Test>::get(OFFENDER).is_some());
    });
}

#[test]
fn the_penalty_leaves_every_other_validator_untouched() {
    new_test_ext().execute_with(|| {
        let pair = author_pair();
        seed_keys(&pair);
        seed_service(20);
        StandingShares::<Test>::insert(3u64, sp_runtime::Perbill::from_percent(70));

        assert_ok!(DcfPallet::report_equivocation(
            RuntimeOrigin::signed(REPORTER),
            Box::new(proof(&pair, 9, 1))
        ));

        assert_eq!(
            StandingShares::<Test>::get(3u64),
            Some(sp_runtime::Perbill::from_percent(70))
        );
        assert!(EquivocationFaults::<Test>::get(3u64).is_none());
    });
}

#[test]
fn two_slots_take_two_penalties() {
    new_test_ext().execute_with(|| {
        let pair = author_pair();
        seed_keys(&pair);
        seed_service(25);

        assert_ok!(DcfPallet::report_equivocation(
            RuntimeOrigin::signed(REPORTER),
            Box::new(proof(&pair, 9, 1))
        ));
        // The standing is already zero. The second equivocation still records.
        assert_ok!(DcfPallet::report_equivocation(
            RuntimeOrigin::signed(REPORTER),
            Box::new(proof(&pair, 10, 1))
        ));

        let fault = EquivocationFaults::<Test>::get(OFFENDER).expect("the chain holds a fault");
        assert_eq!(fault.slot, 10);
        assert!(
            PenalisedEquivocations::<Test>::get(FaultKey::new(9, 1, pair.public().0)).is_some()
        );
        assert!(
            PenalisedEquivocations::<Test>::get(FaultKey::new(10, 1, pair.public().0)).is_some()
        );
    });
}
