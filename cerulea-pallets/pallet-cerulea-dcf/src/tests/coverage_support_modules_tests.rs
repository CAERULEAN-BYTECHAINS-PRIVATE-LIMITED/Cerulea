//! Line coverage for four support modules of the framework pallet.
//!
//! The traits module carries the empty registry, the empty weight freezer and
//! two default key lookups. The EVM compatibility module converts pallet
//! events into indexed records. The solver turns five declarations into
//! deployment profiles for the Amendment Engine. The policies module holds the
//! eight policy records the Amendment Engine governs.

use crate::engines::amendment::limits::{max_faulty, CandidatePoint, LimitBreach, PartitionChoice};
use crate::engines::amendment::policies::{
    AdmissionPolicy, AmendmentPolicy, AssurancePolicy, AuthoringPolicy, Authority,
    EligibilityPolicy, PenaltyPolicy, PolicyKind, PolicySet, RewardPolicy, RotationPolicy,
};
use crate::engines::amendment::solver::{
    inspect_profile, solve, ConfirmationPreference, Declarations, ErrorCost, OperatorCount,
    Profile, RetentionNeed, SplitBehaviour, MAX_CONSEQUENCES, MAX_CONSEQUENCE_BYTES, MAX_PROFILES,
    MIN_PROFILES,
};
use crate::evm_compatibility::{
    EvmCompatibilityError, EvmCompatibleEvent, EvmEventValidator, MAX_EVM_EVENT_PAYLOAD_SIZE,
};
use crate::mock::{
    frozen_epoch_weights, new_test_ext, registry_record_keys, DcfPallet, Test,
    TestValidatorRegistry, TestWeightFreezer,
};
use crate::traits::{ValidatorRegistryProvider, ValidatorState, WeightFreezer};
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::BoundedVec;
use sp_runtime::Perbill;

// ---------------------------------------------------------------------------
// Traits
// ---------------------------------------------------------------------------

/// The empty registry, read with the mock account, balance and block types.
type Empty = ();

fn sample_state() -> ValidatorState {
    ValidatorState {
        last_active_epoch: 4,
        current: crate::EpochStats {
            epoch: 4,
            stake_score: 600,
            inference_score: 400,
            final_score: 520,
            authored_blocks: 9,
            missed_blocks: 1,
        },
        history: vec![crate::EpochStats::default()],
        uptime: 3,
        inference_success_count: 12,
        participation_rate: 9_000,
        inference_count: 15,
        last_active_block: 400,
        name: Some(b"node-four".to_vec()),
        trust_score: 7_500,
    }
}

fn sample_metadata() -> crate::ValidatorMetadataInfo {
    crate::ValidatorMetadataInfo {
        name: BoundedVec::try_from(b"node".to_vec()).expect("four bytes fit the name bound"),
        website: None,
        contact: None,
        description: None,
        location: None,
        commission_rate: Some(5),
        min_stake_required: Some(1_000),
        created_at: 10,
        updated_at: 20,
    }
}

fn sample_record() -> crate::PerformanceRecord {
    crate::PerformanceRecord {
        epoch: 2,
        blocks_authored: 8,
        blocks_missed: 2,
        uptime_percentage: 8_000,
        inference_score: 300,
        stake_score: 700,
        final_score: 540,
        participation_rate: 7_500,
        timestamp: 1_000,
    }
}

#[test]
fn the_empty_weight_freezer_records_nothing() {
    new_test_ext().execute_with(|| {
        let before = frozen_epoch_weights();

        <() as WeightFreezer<u64>>::freeze_epoch_weights(4_242, &[(1, 1_000, 600_000_000)]);
        assert_eq!(frozen_epoch_weights(), before);

        // The mock freezer records the same call, so the call itself carries data.
        <TestWeightFreezer as WeightFreezer<u64>>::freeze_epoch_weights(
            4_242,
            &[(1, 1_000, 600_000_000)],
        );
        let after = frozen_epoch_weights();
        assert!(after.contains(&(4_242, vec![(1, 1_000, 600_000_000)])));
        assert_eq!(after.len(), before.len() + 1);
    });
}

#[test]
fn the_empty_registry_answers_every_read_with_nothing() {
    new_test_ext().execute_with(|| {
        // The seeded registry holds three validators. The empty registry
        // answers the same reads with nothing.
        assert_eq!(
            <TestValidatorRegistry as ValidatorRegistryProvider<u64, u128, u64>>::get_active_validators(),
            vec![1, 2, 3]
        );
        assert!(<Empty as ValidatorRegistryProvider<u64, u128, u64>>::get_active_validators().is_empty());
        assert!(<Empty as ValidatorRegistryProvider<u64, u128, u64>>::get_validator_set().is_empty());
        assert!(<Empty as ValidatorRegistryProvider<u64, u128, u64>>::get_pending_actions().is_empty());

        assert_eq!(
            <TestValidatorRegistry as ValidatorRegistryProvider<u64, u128, u64>>::get_validator_status(&1),
            Some(crate::ValidatorStatus::Active)
        );
        assert_eq!(
            <Empty as ValidatorRegistryProvider<u64, u128, u64>>::get_validator_status(&1),
            None
        );
        assert_eq!(
            <Empty as ValidatorRegistryProvider<u64, u128, u64>>::get_validator_profile(&1),
            None
        );
        assert!(!<Empty as ValidatorRegistryProvider<u64, u128, u64>>::is_validator_active(&1));
        assert_eq!(
            <Empty as ValidatorRegistryProvider<u64, u128, u64>>::get_validator_state(&1),
            None
        );
        assert!(!<Empty as ValidatorRegistryProvider<u64, u128, u64>>::contains_validator_state(&1));
        assert_eq!(
            <Empty as ValidatorRegistryProvider<u64, u128, u64>>::get_validator_name(&1),
            None
        );
        assert_eq!(
            <Empty as ValidatorRegistryProvider<u64, u128, u64>>::get_validator_metadata(&1),
            None
        );
        assert_eq!(
            <Empty as ValidatorRegistryProvider<u64, u128, u64>>::get_validator_detailed_cooldown_status(&1),
            None
        );
        assert_eq!(
            <Empty as ValidatorRegistryProvider<u64, u128, u64>>::get_validator_leave_request(&1),
            None
        );
        assert_eq!(
            <TestValidatorRegistry as ValidatorRegistryProvider<u64, u128, u64>>::get_validator_join_time(&1),
            Some(0)
        );
        assert_eq!(
            <Empty as ValidatorRegistryProvider<u64, u128, u64>>::get_validator_join_time(&1),
            None
        );
        assert_eq!(
            <Empty as ValidatorRegistryProvider<u64, u128, u64>>::get_recently_removed_validator(&1),
            None
        );
        assert!(
            <Empty as ValidatorRegistryProvider<u64, u128, u64>>::get_validator_performance_history(&1)
                .is_empty()
        );
        assert_eq!(
            <Empty as ValidatorRegistryProvider<u64, u128, u64>>::get_validator_last_seen(&1),
            0
        );
        assert_eq!(
            <Empty as ValidatorRegistryProvider<u64, u128, u64>>::get_validator_blocks_authored(&1),
            0
        );
        assert_eq!(
            <Empty as ValidatorRegistryProvider<u64, u128, u64>>::get_validator_blocks_missed(&1),
            0
        );
    });
}

#[test]
fn the_empty_registry_writes_leave_every_record_in_place() {
    new_test_ext().execute_with(|| {
        type Held = TestValidatorRegistry;
        let validator = 1u64;

        // Seed the mock registry with one record of each kind.
        <Held as ValidatorRegistryProvider<u64, u128, u64>>::update_validator_state(
            &validator,
            sample_state(),
        );
        <Held as ValidatorRegistryProvider<u64, u128, u64>>::set_validator_name(
            &validator,
            b"held".to_vec(),
        );
        <Held as ValidatorRegistryProvider<u64, u128, u64>>::set_validator_metadata(
            &validator,
            sample_metadata(),
        );
        <Held as ValidatorRegistryProvider<u64, u128, u64>>::set_validator_leave_request(
            &validator, 40,
        );
        <Held as ValidatorRegistryProvider<u64, u128, u64>>::set_recently_removed_validator(
            &validator, 30,
        );
        <Held as ValidatorRegistryProvider<u64, u128, u64>>::set_validator_performance_history(
            &validator,
            vec![sample_record()],
        );
        <Held as ValidatorRegistryProvider<u64, u128, u64>>::set_validator_last_seen(
            &validator, 77,
        );
        <Held as ValidatorRegistryProvider<u64, u128, u64>>::set_validator_blocks_authored(
            &validator, 12,
        );
        <Held as ValidatorRegistryProvider<u64, u128, u64>>::set_validator_blocks_missed(
            &validator, 3,
        );
        <Held as ValidatorRegistryProvider<u64, u128, u64>>::add_pending_action(
            &validator,
            crate::ValidatorAction::Join,
        );

        // Every write through the empty registry is a no-op.
        <Empty as ValidatorRegistryProvider<u64, u128, u64>>::update_validator_set(vec![9]);
        <Empty as ValidatorRegistryProvider<u64, u128, u64>>::update_active_validators(vec![9]);
        assert_eq!(
            <Empty as ValidatorRegistryProvider<u64, u128, u64>>::eject_validator(
                &validator,
                crate::EjectionReason::ManualEjection
            ),
            Ok(())
        );
        <Empty as ValidatorRegistryProvider<u64, u128, u64>>::update_validator_state(
            &validator,
            sample_state(),
        );
        <Empty as ValidatorRegistryProvider<u64, u128, u64>>::remove_validator_state(&validator);
        <Empty as ValidatorRegistryProvider<u64, u128, u64>>::set_validator_name(
            &validator,
            b"other".to_vec(),
        );
        <Empty as ValidatorRegistryProvider<u64, u128, u64>>::set_validator_metadata(
            &validator,
            sample_metadata(),
        );
        <Empty as ValidatorRegistryProvider<u64, u128, u64>>::remove_validator_metadata(&validator);
        <Empty as ValidatorRegistryProvider<u64, u128, u64>>::remove_pending_action(&validator);
        <Empty as ValidatorRegistryProvider<u64, u128, u64>>::add_pending_action(
            &9,
            crate::ValidatorAction::Leave,
        );
        <Empty as ValidatorRegistryProvider<u64, u128, u64>>::set_validator_join_time(
            &validator, 99,
        );
        <Empty as ValidatorRegistryProvider<u64, u128, u64>>::remove_validator_join_time(
            &validator,
        );
        <Empty as ValidatorRegistryProvider<u64, u128, u64>>::set_validator_leave_request(
            &validator, 99,
        );
        <Empty as ValidatorRegistryProvider<u64, u128, u64>>::remove_validator_leave_request(
            &validator,
        );
        <Empty as ValidatorRegistryProvider<u64, u128, u64>>::set_recently_removed_validator(
            &validator, 99,
        );
        <Empty as ValidatorRegistryProvider<u64, u128, u64>>::remove_recently_removed_validator(
            &validator,
        );
        <Empty as ValidatorRegistryProvider<u64, u128, u64>>::set_validator_performance_history(
            &validator,
            Vec::new(),
        );
        <Empty as ValidatorRegistryProvider<u64, u128, u64>>::remove_validator_performance_history(
            &validator,
        );
        <Empty as ValidatorRegistryProvider<u64, u128, u64>>::set_validator_last_seen(
            &validator, 99,
        );
        <Empty as ValidatorRegistryProvider<u64, u128, u64>>::remove_validator_last_seen(
            &validator,
        );
        <Empty as ValidatorRegistryProvider<u64, u128, u64>>::set_validator_blocks_authored(
            &validator, 99,
        );
        <Empty as ValidatorRegistryProvider<u64, u128, u64>>::remove_validator_blocks_authored(
            &validator,
        );
        <Empty as ValidatorRegistryProvider<u64, u128, u64>>::set_validator_blocks_missed(
            &validator, 99,
        );
        <Empty as ValidatorRegistryProvider<u64, u128, u64>>::remove_validator_blocks_missed(
            &validator,
        );

        // The mock registry still holds every seeded record.
        assert_eq!(
            <Held as ValidatorRegistryProvider<u64, u128, u64>>::get_validator_set(),
            vec![1, 2, 3]
        );
        assert!(
            <Held as ValidatorRegistryProvider<u64, u128, u64>>::is_validator_active(&validator)
        );
        assert_eq!(
            <Held as ValidatorRegistryProvider<u64, u128, u64>>::get_validator_state(&validator),
            Some(sample_state())
        );
        assert_eq!(
            <Held as ValidatorRegistryProvider<u64, u128, u64>>::get_validator_name(&validator),
            Some(b"held".to_vec())
        );
        assert_eq!(
            <Held as ValidatorRegistryProvider<u64, u128, u64>>::get_validator_metadata(&validator),
            Some(sample_metadata())
        );
        let pending = <Held as ValidatorRegistryProvider<u64, u128, u64>>::get_pending_actions();
        assert!(pending.contains(&(validator, crate::ValidatorAction::Join)));
        assert!(pending.iter().all(|(held, _)| *held != 9));
        assert_eq!(
            <Held as ValidatorRegistryProvider<u64, u128, u64>>::get_validator_join_time(
                &validator
            ),
            Some(0)
        );
        assert_eq!(
            <Held as ValidatorRegistryProvider<u64, u128, u64>>::get_validator_leave_request(
                &validator
            ),
            Some(40)
        );
        assert_eq!(
            <Held as ValidatorRegistryProvider<u64, u128, u64>>::get_recently_removed_validator(
                &validator
            ),
            Some(30)
        );
        assert_eq!(
            <Held as ValidatorRegistryProvider<u64, u128, u64>>::get_validator_performance_history(
                &validator
            ),
            vec![sample_record()]
        );
        assert_eq!(
            <Held as ValidatorRegistryProvider<u64, u128, u64>>::get_validator_last_seen(
                &validator
            ),
            77
        );
        assert_eq!(
            <Held as ValidatorRegistryProvider<u64, u128, u64>>::get_validator_blocks_authored(
                &validator
            ),
            12
        );
        assert_eq!(
            <Held as ValidatorRegistryProvider<u64, u128, u64>>::get_validator_blocks_missed(
                &validator
            ),
            3
        );

        // The empty registry still answers with nothing after the writes.
        assert_eq!(
            <Empty as ValidatorRegistryProvider<u64, u128, u64>>::get_validator_name(&validator),
            None
        );
        assert!(
            <Empty as ValidatorRegistryProvider<u64, u128, u64>>::get_pending_actions().is_empty()
        );
    });
}

#[test]
fn the_default_key_lookups_answer_none_for_an_implementor_that_keeps_them() {
    new_test_ext().execute_with(|| {
        let draw_key = [7u8; 32];
        let seal_key = [8u8; 32];
        registry_record_keys(7, draw_key, seal_key);

        // The mock registry overrides both lookups and answers the recorded keys.
        assert_eq!(
            <TestValidatorRegistry as ValidatorRegistryProvider<u64, u128, u64>>::draw_key_owner(&draw_key),
            Some(7)
        );
        assert_eq!(
            <TestValidatorRegistry as ValidatorRegistryProvider<u64, u128, u64>>::validator_seal_key(&7),
            Some(seal_key)
        );

        // The empty registry keeps the default bodies, which answer none.
        assert_eq!(
            <Empty as ValidatorRegistryProvider<u64, u128, u64>>::draw_key_owner(&draw_key),
            None
        );
        assert_eq!(
            <Empty as ValidatorRegistryProvider<u64, u128, u64>>::validator_seal_key(&7),
            None
        );
    });
}

#[test]
fn the_validator_state_record_survives_an_encoding_round_trip() {
    let state = sample_state();
    let encoded = state.encode();
    let decoded = ValidatorState::decode(&mut &encoded[..]).expect("the record decodes");
    assert_eq!(decoded, state);

    let mut changed = state.clone();
    changed.trust_score = 1;
    assert_ne!(changed, state);

    let printed = format!("{:?}", state);
    assert!(printed.contains("ValidatorState"));
    assert!(printed.contains("trust_score"));

    let info = <ValidatorState as scale_info::TypeInfo>::type_info();
    assert_eq!(info.path.ident(), Some("ValidatorState"));
}

// ---------------------------------------------------------------------------
// EVM compatibility
// ---------------------------------------------------------------------------

fn joined(validator: u64) -> crate::Event<Test> {
    crate::Event::<Test>::ValidatorJoined {
        validator,
        stake_amount: 1_000,
    }
}

fn left(validator: u64) -> crate::Event<Test> {
    crate::Event::<Test>::ValidatorLeft { validator }
}

fn epoch_started(epoch: u32, validators: Vec<u64>) -> crate::Event<Test> {
    crate::Event::<Test>::EpochStarted { epoch, validators }
}

fn proposal_submitted() -> crate::Event<Test> {
    crate::Event::<Test>::ProposalSubmitted {
        proposal_id: 9,
        proposer: 4,
        action: crate::ProposalAction::Slash {
            validator: 2,
            amount: 100,
        },
    }
}

/// Every event the type table names, with its expected type number.
fn every_named_event() -> Vec<(crate::Event<Test>, u32)> {
    vec![
        (joined(1), 1),
        (left(2), 2),
        (
            crate::Event::<Test>::ValidatorEjected {
                validator: 3,
                reason: crate::EjectionReason::ManualEjection,
            },
            5,
        ),
        (epoch_started(7, vec![1, 2]), 6),
        (proposal_submitted(), 7),
        (
            crate::Event::<Test>::ProposalExecuted {
                proposal_id: 11,
                status: crate::ProposalStatus::Executed,
            },
            8,
        ),
        (
            crate::Event::<Test>::ValidatorScoreUpdated {
                validator: 1,
                stake_score: 10,
                inference_score: 20,
                final_score: 30,
            },
            9,
        ),
        (
            crate::Event::<Test>::ConsensusWeightsUpdated {
                pos_weight: 6_000,
                poi_weight: 4_000,
            },
            10,
        ),
        (
            crate::Event::<Test>::GovernanceModeToggled { enabled: true },
            11,
        ),
        (
            crate::Event::<Test>::ValidatorMisbehaviorReported {
                reported_validator: 2,
                reporter: 1,
                evidence: BoundedVec::try_from(vec![1u8, 2, 3])
                    .expect("three bytes fit the evidence bound"),
                total_reports: 1,
            },
            12,
        ),
        (
            crate::Event::<Test>::InvariantViolationsDetected {
                epoch: 3,
                violations: BoundedVec::new(),
                severity: crate::InvariantSeverity::Low,
            },
            13,
        ),
        (
            crate::Event::<Test>::PrivateChainModeEnabled {
                allowlist_size: 3,
                allow_updates: true,
            },
            14,
        ),
        (crate::Event::<Test>::PrivateChainModeDisabled, 15),
        (
            crate::Event::<Test>::ValidatorAddedToAllowlist { validator: 5 },
            16,
        ),
        (
            crate::Event::<Test>::ValidatorRemovedFromAllowlist { validator: 5 },
            17,
        ),
        (
            crate::Event::<Test>::ValidatorForcedToLeave {
                validator: 5,
                reason: b"removed".to_vec(),
            },
            18,
        ),
        // An event outside the table carries the catch-all type number.
        (
            crate::Event::<Test>::CustodyShareRecorded {
                validator: 6,
                share: 500_000,
            },
            999,
        ),
    ]
}

fn indexed(event: &EvmCompatibleEvent) -> Vec<Vec<u8>> {
    event
        .indexed_fields
        .iter()
        .map(|field| field.to_vec())
        .collect()
}

#[test]
fn both_evm_switches_report_the_feature_available() {
    assert!(EvmEventValidator::is_evm_enabled());
    assert!(DcfPallet::is_evm_enabled());
}

#[test]
fn a_payload_at_the_size_limit_passes_and_one_byte_more_is_refused() {
    let at_limit = vec![0u8; MAX_EVM_EVENT_PAYLOAD_SIZE as usize];
    assert_eq!(EvmEventValidator::validate_event_payload(&at_limit), Ok(()));

    let over_limit = vec![0u8; MAX_EVM_EVENT_PAYLOAD_SIZE as usize + 1];
    assert_eq!(
        EvmEventValidator::validate_event_payload(&over_limit),
        Err(EvmCompatibilityError::PayloadTooLarge {
            actual_size: 1_025,
            max_size: 1_024,
        })
    );

    assert_eq!(EvmEventValidator::validate_event_payload(&[]), Ok(()));
}

#[test]
fn a_payload_nested_ten_deep_passes_and_eleven_deep_is_refused() {
    let ten_deep = b"[[[[[[[[[[1]]]]]]]]]]";
    assert_eq!(EvmEventValidator::validate_event_payload(ten_deep), Ok(()));

    let eleven_deep = b"[[[[[[[[[[[1]]]]]]]]]]]";
    assert_eq!(
        EvmEventValidator::validate_event_payload(eleven_deep),
        Err(EvmCompatibilityError::ExcessiveNesting {
            level: 11,
            max_level: 10,
        })
    );
}

#[test]
fn the_nesting_count_reads_every_bracket_kind() {
    // Round, square and curly brackets each open one level.
    let mixed = b"({[({[({[({[(x)]})]})]})]})";
    assert_eq!(
        EvmEventValidator::validate_event_payload(mixed),
        Err(EvmCompatibilityError::ExcessiveNesting {
            level: 13,
            max_level: 10,
        })
    );

    // Closing brackets before any opening bracket hold the count at zero.
    let closing_first = b")))]]]}}}{{";
    assert_eq!(
        EvmEventValidator::validate_event_payload(closing_first),
        Ok(())
    );

    // Sibling groups reuse the same depth, so ten groups of depth two pass.
    let siblings = b"[[]][[]][[]][[]][[]][[]][[]][[]][[]][[]]";
    assert_eq!(EvmEventValidator::validate_event_payload(siblings), Ok(()));
}

#[test]
fn every_named_event_receives_its_type_number() {
    new_test_ext().execute_with(|| {
        for (event, expected) in every_named_event() {
            let converted =
                EvmEventValidator::convert_to_evm_format(&event).expect("the event converts");
            assert_eq!(
                converted.event_type, expected,
                "The event receives type number {}.",
                expected
            );
        }
    });
}

#[test]
fn the_converted_record_carries_the_full_encoding_as_data() {
    new_test_ext().execute_with(|| {
        for (event, _) in every_named_event() {
            let encoded = event.encode();
            let converted =
                EvmEventValidator::convert_to_evm_format(&event).expect("the event converts");
            assert_eq!(converted.data_fields.to_vec(), encoded);
            assert_eq!(converted.raw_data.to_vec(), encoded);
        }
    });
}

#[test]
fn validator_events_index_the_validator_account() {
    new_test_ext().execute_with(|| {
        let converted = EvmEventValidator::convert_to_evm_format(&joined(1)).expect("converts");
        assert_eq!(indexed(&converted), vec![1u64.encode()]);

        let converted = EvmEventValidator::convert_to_evm_format(&left(2)).expect("converts");
        assert_eq!(indexed(&converted), vec![2u64.encode()]);

        let ejected = crate::Event::<Test>::ValidatorEjected {
            validator: 3,
            reason: crate::EjectionReason::ManualEjection,
        };
        let converted = EvmEventValidator::convert_to_evm_format(&ejected).expect("converts");
        assert_eq!(indexed(&converted), vec![3u64.encode()]);
    });
}

#[test]
fn epoch_and_proposal_events_index_their_identifiers() {
    new_test_ext().execute_with(|| {
        let converted = EvmEventValidator::convert_to_evm_format(&epoch_started(7, vec![1, 2]))
            .expect("converts");
        assert_eq!(indexed(&converted), vec![7u32.encode()]);

        // A submitted proposal indexes the proposal number and the proposer.
        let converted =
            EvmEventValidator::convert_to_evm_format(&proposal_submitted()).expect("converts");
        assert_eq!(indexed(&converted), vec![9u32.encode(), 4u64.encode()]);

        let executed = crate::Event::<Test>::ProposalExecuted {
            proposal_id: 11,
            status: crate::ProposalStatus::Executed,
        };
        let converted = EvmEventValidator::convert_to_evm_format(&executed).expect("converts");
        assert_eq!(indexed(&converted), vec![11u32.encode()]);
    });
}

#[test]
fn every_other_event_carries_an_empty_index() {
    new_test_ext().execute_with(|| {
        for (event, expected) in every_named_event() {
            if matches!(expected, 1 | 2 | 5 | 6 | 7 | 8) {
                continue;
            }
            let converted =
                EvmEventValidator::convert_to_evm_format(&event).expect("the event converts");
            assert!(
                converted.indexed_fields.is_empty(),
                "Type number {} carries an empty index.",
                expected
            );
        }
    });
}

#[test]
fn an_oversized_event_is_refused_with_its_true_size() {
    new_test_ext().execute_with(|| {
        // Two hundred accounts at eight bytes each pass the payload limit.
        let event = epoch_started(1, (0..200u64).collect());
        let size = event.encode().len() as u32;
        assert_eq!(size, 1_607);
        assert_eq!(
            EvmEventValidator::convert_to_evm_format(&event),
            Err(EvmCompatibilityError::PayloadTooLarge {
                actual_size: size,
                max_size: MAX_EVM_EVENT_PAYLOAD_SIZE,
            })
        );
    });
}

#[test]
fn a_deeply_nested_event_is_refused_with_its_depth() {
    new_test_ext().execute_with(|| {
        let event = crate::Event::<Test>::ValidatorForcedToLeave {
            validator: 5,
            reason: vec![b'['; 12],
        };
        let encoded = event.encode();
        // The variant index byte opens one more level where it equals an
        // opening bracket. The account bytes and the length byte open none.
        let index_opens = matches!(encoded[0], b'[' | b'{' | b'(');
        let expected = 12 + u32::from(index_opens);
        assert_eq!(
            EvmEventValidator::convert_to_evm_format(&event),
            Err(EvmCompatibilityError::ExcessiveNesting {
                level: expected,
                max_level: 10,
            })
        );
    });
}

#[test]
fn an_emitted_event_lands_under_the_current_block() {
    new_test_ext().execute_with(|| {
        frame_system::Pallet::<Test>::set_block_number(7);
        assert_eq!(DcfPallet::emit_evm_compatible_event(&joined(1)), Ok(()));
        assert_eq!(DcfPallet::emit_evm_compatible_event(&left(2)), Ok(()));

        let held = DcfPallet::get_evm_events_for_block(7);
        assert_eq!(held.len(), 2);
        assert_eq!(held[0].event_type, 1);
        assert_eq!(held[1].event_type, 2);
        assert_eq!(held[0].raw_data.to_vec(), joined(1).encode());
        assert!(DcfPallet::get_evm_events_for_block(6).is_empty());
    });
}

#[test]
fn a_refused_event_leaves_the_block_record_empty() {
    new_test_ext().execute_with(|| {
        frame_system::Pallet::<Test>::set_block_number(8);
        let event = epoch_started(1, (0..200u64).collect());
        assert!(matches!(
            DcfPallet::emit_evm_compatible_event(&event),
            Err(EvmCompatibilityError::PayloadTooLarge { .. })
        ));
        assert!(DcfPallet::get_evm_events_for_block(8).is_empty());
    });
}

#[test]
fn one_block_holds_at_most_one_hundred_records() {
    new_test_ext().execute_with(|| {
        frame_system::Pallet::<Test>::set_block_number(3);
        for validator in 0..101u64 {
            assert_eq!(
                DcfPallet::emit_evm_compatible_event(&left(validator)),
                Ok(())
            );
        }
        let held = DcfPallet::get_evm_events_for_block(3);
        assert_eq!(held.len(), 100);
        // The first hundred records stay. The record past the bound is dropped.
        assert_eq!(held[99].raw_data.to_vec(), left(99).encode());
    });
}

#[test]
fn the_query_filters_by_type_and_block_range() {
    new_test_ext().execute_with(|| {
        frame_system::Pallet::<Test>::set_block_number(4);
        assert_eq!(DcfPallet::emit_evm_compatible_event(&joined(1)), Ok(()));
        assert_eq!(DcfPallet::emit_evm_compatible_event(&left(1)), Ok(()));
        frame_system::Pallet::<Test>::set_block_number(5);
        assert_eq!(
            DcfPallet::emit_evm_compatible_event(&epoch_started(2, vec![1])),
            Ok(())
        );

        let epochs = DcfPallet::query_evm_events(Some(6), 4, 5);
        assert_eq!(epochs.len(), 1);
        assert_eq!(epochs[0].0, 5);
        assert_eq!(epochs[0].1.event_type, 6);

        let everything = DcfPallet::query_evm_events(None, 4, 5);
        let blocks: Vec<u32> = everything.iter().map(|(block, _)| *block).collect();
        assert_eq!(blocks, vec![4, 4, 5]);

        assert!(DcfPallet::query_evm_events(Some(999), 0, 10).is_empty());
        // An inverted range reads no block.
        assert!(DcfPallet::query_evm_events(None, 5, 4).is_empty());
    });
}

#[test]
fn the_cleanup_keeps_the_recent_window_and_removes_older_blocks() {
    new_test_ext().execute_with(|| {
        for block in 0..=10u64 {
            frame_system::Pallet::<Test>::set_block_number(block);
            assert_eq!(DcfPallet::emit_evm_compatible_event(&left(1)), Ok(()));
        }

        // A window wider than the chain removes nothing.
        DcfPallet::cleanup_old_evm_events(50);
        for block in 0..=10u32 {
            assert_eq!(DcfPallet::get_evm_events_for_block(block).len(), 1);
        }

        // At block ten a window of three keeps block seven and later.
        DcfPallet::cleanup_old_evm_events(3);
        for block in 0..7u32 {
            assert!(DcfPallet::get_evm_events_for_block(block).is_empty());
        }
        for block in 7..=10u32 {
            assert_eq!(DcfPallet::get_evm_events_for_block(block).len(), 1);
        }
    });
}

#[test]
fn the_event_samples_hold_one_event_of_every_variant() {
    new_test_ext().execute_with(|| {
        let samples = DcfPallet::sample_events().expect("the samples build");
        let variants = match <crate::Event<Test> as scale_info::TypeInfo>::type_info().type_def {
            scale_info::TypeDef::Variant(def) => {
                def.variants
                    .iter()
                    .map(|variant| variant.index)
                    .collect::<sp_std::collections::btree_set::BTreeSet<u8>>()
            }
            other => panic!(
                "The event type reads {:?}. A variant type was expected.",
                other
            ),
        };
        let sampled: sp_std::collections::btree_set::BTreeSet<u8> =
            samples.iter().map(|event| event.encode()[0]).collect();
        // One sample per variant: no repeat and no gap.
        assert_eq!(samples.len(), 83);
        assert_eq!(sampled.len(), samples.len());
        assert_eq!(sampled, variants);
        // Every sample passes the conversion the check runs.
        for event in &samples {
            let record =
                EvmEventValidator::convert_to_evm_format(event).expect("every sample converts");
            assert_eq!(record.raw_data.to_vec(), event.encode());
        }
    });
}

#[test]
fn the_full_event_check_passes_and_stores_nothing() {
    new_test_ext().execute_with(|| {
        frame_system::Pallet::<Test>::set_block_number(12);
        assert_eq!(DcfPallet::validate_all_events_evm_compatible(), Ok(()));
        assert!(DcfPallet::get_evm_events_for_block(12).is_empty());
    });
}

#[test]
fn every_compatibility_error_survives_an_encoding_round_trip() {
    let errors = [
        EvmCompatibilityError::PayloadTooLarge {
            actual_size: 2_000,
            max_size: 1_024,
        },
        EvmCompatibilityError::ExcessiveNesting {
            level: 12,
            max_level: 10,
        },
        EvmCompatibilityError::IndexedFieldTooLarge {
            field_size: 40,
            max_size: 32,
        },
        EvmCompatibilityError::UnsupportedEventType,
        EvmCompatibilityError::EncodingFailed,
    ];
    for (index, error) in errors.iter().enumerate() {
        let encoded = error.encode();
        assert_eq!(encoded[0] as usize, index);
        let decoded = EvmCompatibilityError::decode(&mut &encoded[..]).expect("the error decodes");
        assert_eq!(&decoded, error);
        assert_eq!(decoded.clone(), *error);
        assert!(!format!("{:?}", error).is_empty());
    }
}

#[test]
fn the_compatible_record_survives_an_encoding_round_trip() {
    new_test_ext().execute_with(|| {
        let record =
            EvmEventValidator::convert_to_evm_format(&proposal_submitted()).expect("converts");
        let encoded = record.encode();
        let decoded = EvmCompatibleEvent::decode(&mut &encoded[..]).expect("the record decodes");
        assert_eq!(decoded, record);
        assert!(format!("{:?}", record).contains("EvmCompatibleEvent"));

        // Four bytes of type, three fields of up to 32 bytes, 1024 bytes of
        // data and 2048 bytes of raw encoding, each behind a length prefix.
        assert_eq!(EvmCompatibleEvent::max_encoded_len(), 3_180);
        assert!(encoded.len() <= EvmCompatibleEvent::max_encoded_len());
    });
}

// ---------------------------------------------------------------------------
// Amendment Engine solver
// ---------------------------------------------------------------------------

fn sentence_text(profile: &Profile, at: usize) -> String {
    String::from_utf8(profile.consequences[at].to_vec()).expect("the sentence reads as text")
}

fn point(
    validator_count: u32,
    routine: u8,
    constitutional: u8,
    choice: PartitionChoice,
) -> CandidatePoint {
    CandidatePoint {
        validator_count,
        tolerated_failures: max_faulty(validator_count),
        routine_percent: routine,
        constitutional_percent: constitutional,
        partition_choice: choice,
        partition_choice_stated: true,
    }
}

#[test]
fn the_default_declarations_take_the_first_choice_of_each_question() {
    assert_eq!(
        Declarations::default(),
        Declarations {
            confirmation: ConfirmationPreference::InstantConfirmation,
            operators: OperatorCount::SingleOrganisation,
            error_cost: ErrorCost::Recoverable,
            split: SplitBehaviour::KeepServing,
            retention: RetentionNeed::RecentOnly,
        }
    );
    assert_eq!(Declarations::default().encode(), vec![0, 0, 0, 0, 0]);
}

#[test]
fn one_small_organisation_receives_five_ranked_small_profiles() {
    // Each set size scores n / 4 + |n - 4| / 2 at a 51 percent routine share,
    // an 80 percent constitutional share and the stated split choice. The
    // five lowest scores belong to sets of 4, 7, 10, 21 and 50.
    let profiles = solve(Declarations::default());
    assert_eq!(profiles.len(), MAX_PROFILES);

    let sizes: Vec<u32> = profiles
        .iter()
        .map(|held| held.point.validator_count)
        .collect();
    assert_eq!(sizes, vec![4, 7, 10, 21, 50]);

    for held in profiles.iter() {
        assert_eq!(held.point.routine_percent, 51);
        assert_eq!(held.point.constitutional_percent, 80);
        assert_eq!(held.point.partition_choice, PartitionChoice::KeepServing);
        assert!(held.point.partition_choice_stated);
        assert_eq!(
            held.point.tolerated_failures,
            max_faulty(held.point.validator_count)
        );
        assert_eq!(held.retention_blocks, 432_000);
        assert_eq!(held.consequences.len(), 6);
    }

    assert_eq!(
        profiles[0].point,
        point(4, 51, 80, PartitionChoice::KeepServing)
    );
    assert_eq!(profiles[0].point.tolerated_failures, 1);
    assert_eq!(profiles[0].routine_finality_blocks, 1);
    assert_eq!(profiles[0].constitutional_finality_blocks, 3);
    assert_eq!(profiles[4].point.tolerated_failures, 16);
    assert_eq!(profiles[4].routine_finality_blocks, 3);
    assert_eq!(profiles[4].constitutional_finality_blocks, 5);
}

#[test]
fn the_small_profile_states_its_consequences_in_order() {
    let profiles = solve(Declarations::default());

    let small = &profiles[0];
    assert!(sentence_text(small, 0).starts_with("The network runs on a small set."));
    assert!(sentence_text(small, 1)
        .starts_with("A routine record confirms at a simple majority of weight."));
    assert!(
        sentence_text(small, 2).starts_with("A policy change confirms at four fifths of weight.")
    );
    assert_eq!(
        sentence_text(small, 3),
        PartitionChoice::KeepServing.plain_language()
    );
    assert!(
        sentence_text(small, 4).starts_with("The deployment keeps roughly one month of history.")
    );
    assert!(sentence_text(small, 5)
        .starts_with("The network tolerates failures in fewer than a third of the set."));

    // A set of seven reads as a moderate set. A set of 21 reads as a wide set.
    assert!(sentence_text(&profiles[1], 0).starts_with("The network runs on a moderate set."));
    assert!(sentence_text(&profiles[3], 0).starts_with("The network runs on a wide set."));
}

#[test]
fn many_organisations_with_irreversible_cost_receive_strict_profiles() {
    // Maximum certainty scores (67 - r) * 2 + (100 - n) / 4. Many operators add
    // |n - 50| / 2. The five lowest scores belong to 50, 100, 21, 10 and 7.
    let held = Declarations {
        confirmation: ConfirmationPreference::MaximumCertainty,
        operators: OperatorCount::ManyOrganisations,
        error_cost: ErrorCost::Irreversible,
        split: SplitBehaviour::HaltToStayConsistent,
        retention: RetentionNeed::Permanent,
    };
    let profiles = solve(held);

    let sizes: Vec<u32> = profiles
        .iter()
        .map(|profile| profile.point.validator_count)
        .collect();
    assert_eq!(sizes, vec![50, 100, 21, 10, 7]);

    for profile in profiles.iter() {
        assert_eq!(profile.point.routine_percent, 67);
        assert_eq!(profile.point.constitutional_percent, 95);
        assert_eq!(
            profile.point.partition_choice,
            PartitionChoice::HaltToStayConsistent
        );
        assert_eq!(profile.retention_blocks, 0);
    }

    let widest = &profiles[1];
    assert_eq!(widest.point.tolerated_failures, 33);
    assert_eq!(widest.routine_finality_blocks, 5);
    assert_eq!(widest.constitutional_finality_blocks, 7);

    let first = &profiles[0];
    assert!(sentence_text(first, 0).starts_with("The network runs on a wide set."));
    assert!(
        sentence_text(first, 1).starts_with("A routine record confirms at two thirds of weight.")
    );
    assert!(sentence_text(first, 2)
        .starts_with("A policy change confirms at nineteen twentieths of weight."));
    assert_eq!(
        sentence_text(first, 3),
        PartitionChoice::HaltToStayConsistent.plain_language()
    );
    assert!(sentence_text(first, 4).starts_with("The deployment keeps every block from genesis."));
}

#[test]
fn several_organisations_with_expensive_cost_receive_nine_tenths_profiles() {
    // Sets of 7 and 10 tie at a score of two. The smaller set sorts first.
    let held = Declarations {
        confirmation: ConfirmationPreference::InstantConfirmation,
        operators: OperatorCount::SeveralOrganisations,
        error_cost: ErrorCost::Expensive,
        split: SplitBehaviour::KeepServing,
        retention: RetentionNeed::SeveralYears,
    };
    let profiles = solve(held);

    let sizes: Vec<u32> = profiles
        .iter()
        .map(|profile| profile.point.validator_count)
        .collect();
    assert_eq!(sizes, vec![7, 10, 4, 21, 50]);

    for profile in profiles.iter() {
        assert_eq!(profile.point.routine_percent, 51);
        assert_eq!(profile.point.constitutional_percent, 90);
        assert_eq!(profile.retention_blocks, 15_768_000);
        assert!(sentence_text(profile, 2)
            .starts_with("A policy change confirms at nine tenths of weight."));
        assert!(sentence_text(profile, 4)
            .starts_with("The deployment keeps roughly three years of history."));
    }
}

#[test]
fn every_profile_stays_inside_the_returned_bounds_for_every_declaration() {
    let confirmations = [
        ConfirmationPreference::InstantConfirmation,
        ConfirmationPreference::MaximumCertainty,
    ];
    let operators = [
        OperatorCount::SingleOrganisation,
        OperatorCount::SeveralOrganisations,
        OperatorCount::ManyOrganisations,
    ];
    let costs = [
        ErrorCost::Recoverable,
        ErrorCost::Expensive,
        ErrorCost::Irreversible,
    ];
    let splits = [
        SplitBehaviour::KeepServing,
        SplitBehaviour::HaltToStayConsistent,
    ];
    let retentions = [
        RetentionNeed::RecentOnly,
        RetentionNeed::SeveralYears,
        RetentionNeed::Permanent,
    ];

    for confirmation in confirmations {
        for operator in operators {
            for cost in costs {
                for split in splits {
                    for retention in retentions {
                        let profiles = solve(Declarations {
                            confirmation,
                            operators: operator,
                            error_cost: cost,
                            split,
                            retention,
                        });
                        assert!(profiles.len() >= MIN_PROFILES);
                        assert!(profiles.len() <= MAX_PROFILES);
                        for profile in profiles.iter() {
                            assert!(profile.consequences.len() as u32 <= MAX_CONSEQUENCES);
                            for line in profile.consequences.iter() {
                                assert!(line.len() as u32 <= MAX_CONSEQUENCE_BYTES);
                            }
                            // The best routine share for the confirmation
                            // preference always wins inside one set size.
                            let expected_routine = match confirmation {
                                ConfirmationPreference::InstantConfirmation => 51,
                                ConfirmationPreference::MaximumCertainty => 67,
                            };
                            assert_eq!(profile.point.routine_percent, expected_routine);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn the_profile_inspection_refuses_a_point_past_a_fixed_limit() {
    let solved = solve(Declarations::default());
    assert_eq!(inspect_profile(&solved[0]), Ok(()));

    let mut small = solved[0].clone();
    small.point.validator_count = 3;
    small.point.tolerated_failures = 0;
    assert_eq!(inspect_profile(&small), Err(LimitBreach::SetTooSmall));

    let mut crossed = solved[0].clone();
    crossed.point.validator_count = 10;
    crossed.point.tolerated_failures = 4;
    assert_eq!(
        inspect_profile(&crossed),
        Err(LimitBreach::FaultBoundCrossed)
    );

    let mut unstated = solved[0].clone();
    unstated.point.partition_choice_stated = false;
    assert_eq!(
        inspect_profile(&unstated),
        Err(LimitBreach::PartitionChoiceAbsent)
    );

    let bare = Profile {
        point: CandidatePoint::default(),
        routine_finality_blocks: 0,
        constitutional_finality_blocks: 0,
        retention_blocks: 0,
        consequences: BoundedVec::default(),
    };
    assert_eq!(inspect_profile(&bare), Err(LimitBreach::SetTooSmall));
}

#[test]
fn every_declaration_type_defaults_to_its_first_variant_and_encodes_its_index() {
    assert_eq!(
        ConfirmationPreference::default(),
        ConfirmationPreference::InstantConfirmation
    );
    assert_eq!(OperatorCount::default(), OperatorCount::SingleOrganisation);
    assert_eq!(ErrorCost::default(), ErrorCost::Recoverable);
    assert_eq!(SplitBehaviour::default(), SplitBehaviour::KeepServing);
    assert_eq!(RetentionNeed::default(), RetentionNeed::RecentOnly);

    assert_eq!(ConfirmationPreference::MaximumCertainty.encode(), vec![1]);
    assert_eq!(OperatorCount::ManyOrganisations.encode(), vec![2]);
    assert_eq!(ErrorCost::Irreversible.encode(), vec![2]);
    assert_eq!(SplitBehaviour::HaltToStayConsistent.encode(), vec![1]);
    assert_eq!(RetentionNeed::Permanent.encode(), vec![2]);
}

#[test]
fn the_declarations_and_profiles_survive_an_encoding_round_trip() {
    let held = Declarations {
        confirmation: ConfirmationPreference::MaximumCertainty,
        operators: OperatorCount::ManyOrganisations,
        error_cost: ErrorCost::Irreversible,
        split: SplitBehaviour::HaltToStayConsistent,
        retention: RetentionNeed::Permanent,
    };
    let encoded = held.encode();
    assert_eq!(encoded, vec![1, 2, 2, 1, 2]);
    assert_eq!(Declarations::decode(&mut &encoded[..]), Ok(held));
    assert_eq!(Declarations::max_encoded_len(), 5);

    for profile in solve(held) {
        let bytes = profile.encode();
        assert!(bytes.len() <= Profile::max_encoded_len());
        let decoded = Profile::decode(&mut &bytes[..]).expect("the profile decodes");
        assert_eq!(decoded, profile);
    }

    // A point is two counts, two shares, one choice and one flag.
    assert_eq!(CandidatePoint::max_encoded_len(), 12);
    // A profile adds three counts and up to eight sentences of 256 bytes.
    assert_eq!(Profile::max_encoded_len(), 12 + 12 + 1 + 8 * (2 + 256));

    assert!(format!("{:?}", held).contains("MaximumCertainty"));
    assert_eq!(
        <Profile as scale_info::TypeInfo>::type_info().path.ident(),
        Some("Profile")
    );
}

// ---------------------------------------------------------------------------
// Amendment Engine policies
// ---------------------------------------------------------------------------

#[test]
fn the_default_policy_set_is_the_starting_profile() {
    assert_eq!(PolicySet::default(), PolicySet::starting_profile());

    let held = PolicySet::default();
    assert_eq!(
        held.admission,
        AdmissionPolicy {
            identity_required: true,
            approvals_required: 1,
            credentials_required: 0,
            licence_class: 1,
        }
    );
    assert_eq!(
        held.rotation,
        RotationPolicy {
            epoch_blocks: 100,
            set_size: 50,
            rotate_at_boundary_only: true,
        }
    );
    assert_eq!(held.authoring.rank_timeout_blocks, 1);
    assert_eq!(held.authoring.trust_score_weight, Perbill::one());
    assert_eq!(held.eligibility.uptime_floor, Perbill::from_percent(80));
    assert_eq!(
        held.eligibility.participation_floor,
        Perbill::from_percent(50)
    );
    assert_eq!(held.eligibility.standing_floor, Perbill::from_percent(33));
    assert_eq!(held.penalty.offence_count, 5);
    assert_eq!(held.penalty.light_loss, Perbill::from_percent(5));
    assert_eq!(held.penalty.severe_loss, Perbill::from_percent(50));
    assert_eq!(held.reward.throughput_quota, 1_000);
}

#[test]
fn every_single_policy_record_defaults_to_zero() {
    assert_eq!(
        AdmissionPolicy::default(),
        AdmissionPolicy {
            identity_required: false,
            approvals_required: 0,
            credentials_required: 0,
            licence_class: 0,
        }
    );
    assert_eq!(
        RotationPolicy::default(),
        RotationPolicy {
            epoch_blocks: 0,
            set_size: 0,
            rotate_at_boundary_only: false,
        }
    );
    assert_eq!(
        AuthoringPolicy::default(),
        AuthoringPolicy {
            committee_size: 0,
            rank_timeout_blocks: 0,
            trust_score_weight: Perbill::zero(),
        }
    );
    assert_eq!(
        AssurancePolicy::default(),
        AssurancePolicy {
            routine_percent: 0,
            financial_percent: 0,
            structural_percent: 0,
            constitutional_percent: 0,
            constitutional_delay_blocks: 0,
        }
    );
    assert_eq!(
        EligibilityPolicy::default(),
        EligibilityPolicy {
            uptime_floor: Perbill::zero(),
            participation_floor: Perbill::zero(),
            standing_floor: Perbill::zero(),
        }
    );
    assert_eq!(
        PenaltyPolicy::default(),
        PenaltyPolicy {
            offence_count: 0,
            light_loss: Perbill::zero(),
            severe_loss: Perbill::zero(),
            misconduct_resets_standing: false,
        }
    );
    assert_eq!(
        RewardPolicy::default(),
        RewardPolicy {
            base_share: Perbill::zero(),
            performance_share: Perbill::zero(),
            top_performer_share: Perbill::zero(),
            throughput_quota: 0,
        }
    );
    assert_eq!(
        AmendmentPolicy::default(),
        AmendmentPolicy {
            authority: Authority::Operator,
            quorum: Perbill::zero(),
            delay_blocks: 0,
        }
    );
    assert_eq!(Authority::default(), Authority::Operator);
    assert_eq!(PolicyKind::default(), PolicyKind::Admission);
}

#[test]
fn every_policy_kind_carries_its_exact_label() {
    let labels: Vec<&'static str> = PolicyKind::ALL.iter().map(|kind| kind.label()).collect();
    assert_eq!(
        labels,
        vec![
            "admission",
            "rotation",
            "authoring",
            "assurance",
            "eligibility",
            "penalty",
            "reward",
            "amendment",
        ]
    );
}

#[test]
fn every_policy_sentence_opens_with_the_policy_name() {
    let names = [
        "Admission governs",
        "Rotation governs",
        "Authoring governs",
        "Assurance governs",
        "Eligibility governs",
        "Penalty governs",
        "Reward governs",
        "Amendment governs",
    ];
    for (kind, name) in PolicyKind::ALL.iter().zip(names.iter()) {
        assert!(
            kind.plain_language().starts_with(name),
            "The {} sentence opens with its policy name.",
            kind.label()
        );
        assert!(kind.plain_language().ends_with('.'));
    }
}

#[test]
fn the_amendment_engine_governs_exactly_the_first_seven_kinds() {
    let governed: Vec<PolicyKind> = PolicyKind::ALL
        .iter()
        .copied()
        .filter(|kind| kind.governed_by_amendment())
        .collect();
    assert_eq!(governed, PolicyKind::GOVERNED.to_vec());
    assert_eq!(&PolicyKind::ALL[..7], &PolicyKind::GOVERNED[..]);
    assert_eq!(PolicyKind::ALL[7], PolicyKind::Amendment);
}

#[test]
fn the_policy_kinds_sort_in_record_order() {
    let mut shuffled = vec![
        PolicyKind::Amendment,
        PolicyKind::Reward,
        PolicyKind::Admission,
        PolicyKind::Eligibility,
        PolicyKind::Rotation,
        PolicyKind::Penalty,
        PolicyKind::Assurance,
        PolicyKind::Authoring,
    ];
    shuffled.sort();
    assert_eq!(shuffled, PolicyKind::ALL.to_vec());
    assert!(PolicyKind::Admission < PolicyKind::Amendment);
    assert_eq!(
        PolicyKind::Rotation.partial_cmp(&PolicyKind::Authoring),
        Some(core::cmp::Ordering::Less)
    );
    assert_eq!(PolicyKind::ALL.iter().max(), Some(&PolicyKind::Amendment));
}

#[test]
fn every_authority_states_its_exact_sentence() {
    assert_eq!(
        Authority::Operator.plain_language(),
        "The deployment operator opens an amendment."
    );
    assert_eq!(
        Authority::ValidatorSet.plain_language(),
        "The seated validators open an amendment by vote."
    );
    assert_eq!(
        Authority::GoverningBody.plain_language(),
        "A named governing body opens an amendment."
    );
}

#[test]
fn the_policy_records_encode_their_variant_index() {
    for (index, kind) in PolicyKind::ALL.iter().enumerate() {
        assert_eq!(kind.encode(), vec![index as u8]);
        assert_eq!(PolicyKind::decode(&mut &kind.encode()[..]), Ok(*kind));
    }
    assert_eq!(Authority::Operator.encode(), vec![0]);
    assert_eq!(Authority::ValidatorSet.encode(), vec![1]);
    assert_eq!(Authority::GoverningBody.encode(), vec![2]);
}

#[test]
fn the_policy_set_survives_an_encoding_round_trip_at_its_fixed_size() {
    let held = PolicySet::starting_profile();
    let encoded = held.encode();
    assert_eq!(PolicySet::decode(&mut &encoded[..]), Ok(held));

    // Admission 10, Rotation 9, Authoring 12, Assurance 8, Eligibility 12,
    // Penalty 13, Reward 16 and Amendment 9 bytes.
    assert_eq!(PolicySet::max_encoded_len(), 89);
    assert_eq!(encoded.len(), 89);

    assert!(format!("{:?}", held).contains("PolicySet"));
    assert_eq!(
        <PolicySet as scale_info::TypeInfo>::type_info()
            .path
            .ident(),
        Some("PolicySet")
    );

    let mut changed = held;
    changed.amendment.authority = Authority::GoverningBody;
    assert_ne!(changed, held);
    assert_eq!(
        PolicySet::decode(&mut &changed.encode()[..]).map(|decoded| decoded.amendment.authority),
        Ok(Authority::GoverningBody)
    );
}
