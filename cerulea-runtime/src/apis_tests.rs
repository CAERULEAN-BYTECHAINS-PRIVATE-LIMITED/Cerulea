//! The runtime interfaces the node calls, run against real runtime state.
//!
//! Each test builds the development genesis preset and calls the interface
//! implementation the runtime exposes, through the same trait the node
//! reaches over the executor. The answers therefore come from the state the
//! genesis build wrote and the blocks the tests execute.

use crate::genesis_config_presets::{starting_blueprint, starting_deployment};
use crate::platform_tests::development_ext;
use crate::{
    AccountId, Balance, Block, BlockNumber, Header, Nonce, Runtime, RuntimeCall, SessionKeys,
    SignedPayload, TxExtension, UncheckedExtrinsic, VERSION,
};
use codec::{Decode, Encode};
use sp_core::{Pair, H256};
use sp_keyring::Ed25519Keyring;
use sp_runtime::{
    generic::Era,
    traits::{Block as BlockT, Header as HeaderT},
    transaction_validity::{InvalidTransaction, TransactionSource, TransactionValidityError},
    MultiSignature,
};

use cerulea_platform_api::runtime_decl_for_cerulea_platform_api::CeruleaPlatformApiV1 as PlatformApi;
use frame_system_rpc_runtime_api::runtime_decl_for_account_nonce_api::AccountNonceApiV1 as NonceApi;
use pallet_cerulea_dcf::runtime_decl_for_dcf_api::DcfApiV1 as DcfApi;
use pallet_cerulea_dvf::runtime_decl_for_cerulea_authoring_api::CeruleaAuthoringApiV1 as AuthoringApi;
use pallet_cerulea_dvf::runtime_decl_for_dvf_api::DvfApiV1 as DvfApi;
use pallet_cerulea_poi::runtime_decl_for_poi_api::PoiApiV1 as PoiApi;
use pallet_cerulea_pos::runtime_decl_for_pos_api::PosApiV1 as PosApi;
use pallet_transaction_payment_rpc_runtime_api::runtime_decl_for_transaction_payment_api::TransactionPaymentApiV4 as PaymentApi;
use pallet_transaction_payment_rpc_runtime_api::runtime_decl_for_transaction_payment_call_api::TransactionPaymentCallApiV3 as PaymentCallApi;
use sp_api::runtime_decl_for_core::CoreV5 as CoreApi;
use sp_api::runtime_decl_for_metadata::MetadataV2 as MetadataApi;
use sp_block_builder::runtime_decl_for_block_builder::BlockBuilderV6 as BlockBuilderApi;
use sp_genesis_builder::runtime_decl_for_genesis_builder::GenesisBuilderV1 as GenesisApi;
use sp_session::runtime_decl_for_session_keys::SessionKeysV1 as SessionApi;
use sp_transaction_pool::runtime_api::runtime_decl_for_tagged_transaction_queue::TaggedTransactionQueueV3 as PoolApi;

fn alice() -> AccountId {
    Ed25519Keyring::Alice.to_account_id()
}

fn bob() -> AccountId {
    Ed25519Keyring::Bob.to_account_id()
}

/// Sign one call as Alice with the full transaction extension the runtime
/// checks.
fn signed_by_alice(call: RuntimeCall, nonce: Nonce) -> UncheckedExtrinsic {
    let genesis = frame_system::BlockHash::<Runtime>::get(0);
    let extension: TxExtension = (
        frame_system::CheckNonZeroSender::<Runtime>::new(),
        frame_system::CheckSpecVersion::<Runtime>::new(),
        frame_system::CheckTxVersion::<Runtime>::new(),
        frame_system::CheckGenesis::<Runtime>::new(),
        frame_system::CheckEra::<Runtime>::from(Era::Immortal),
        frame_system::CheckNonce::<Runtime>::from(nonce),
        frame_system::CheckWeight::<Runtime>::new(),
        pallet_transaction_payment::ChargeTransactionPayment::<Runtime>::from(0),
        frame_metadata_hash_extension::CheckMetadataHash::<Runtime>::new(false),
        frame_system::WeightReclaim::<Runtime>::new(),
    );
    let implicit = (
        (),
        VERSION.spec_version,
        VERSION.transaction_version,
        genesis,
        genesis,
        (),
        (),
        (),
        None,
        (),
    );
    let payload = SignedPayload::from_raw(call.clone(), extension.clone(), implicit);
    let signature = payload.using_encoded(|bytes| Ed25519Keyring::Alice.pair().sign(bytes));
    UncheckedExtrinsic::new_signed(
        call,
        sp_runtime::MultiAddress::Id(alice()),
        MultiSignature::Ed25519(signature),
        extension,
    )
}

fn remark(text: &[u8]) -> RuntimeCall {
    RuntimeCall::System(frame_system::Call::remark {
        remark: text.to_vec(),
    })
}

/// The timestamp inherent for one moment.
fn timestamp_inherent(moment: u64) -> sp_inherents::InherentData {
    let mut data = sp_inherents::InherentData::new();
    let identifier = <pallet_timestamp::Pallet<Runtime> as frame_support::inherent::ProvideInherent>::INHERENT_IDENTIFIER;
    data.put_data(identifier, &moment)
        .expect("the timestamp encodes");
    data
}

/// Build block one over the development genesis and return it.
fn build_block_one() -> Block {
    let mut ext = development_ext();
    ext.execute_with(|| {
        let parent = frame_system::BlockHash::<Runtime>::get(0);
        let header = Header::new(
            1,
            Default::default(),
            Default::default(),
            parent,
            Default::default(),
        );
        <Runtime as CoreApi<Block>>::initialize_block(&header);
        let inherents =
            <Runtime as BlockBuilderApi<Block>>::inherent_extrinsics(timestamp_inherent(6_000));
        assert_eq!(inherents.len(), 1, "the timestamp module sets one inherent");
        let mut extrinsics = Vec::new();
        for inherent in inherents {
            let outcome = <Runtime as BlockBuilderApi<Block>>::apply_extrinsic(inherent.clone());
            assert!(matches!(outcome, Ok(Ok(()))), "the inherent applies");
            extrinsics.push(inherent);
        }
        let transfer = signed_by_alice(remark(b"the registry opened"), 0);
        let outcome = <Runtime as BlockBuilderApi<Block>>::apply_extrinsic(transfer.clone());
        assert!(matches!(outcome, Ok(Ok(()))), "the signed remark applies");
        extrinsics.push(transfer);
        let header = <Runtime as BlockBuilderApi<Block>>::finalize_block();
        Block::new(header, extrinsics)
    })
}

#[test]
fn the_core_interface_answers_the_runtime_version() {
    let version = <Runtime as CoreApi<Block>>::version();
    assert_eq!(version.spec_name, "cerulea-runtime");
    assert_eq!(version.spec_version, VERSION.spec_version);
    assert!(!version.apis.is_empty());
}

#[test]
fn a_block_built_through_the_block_builder_executes_on_a_fresh_chain() {
    let block = build_block_one();
    assert_eq!(*block.header().number(), 1);
    assert_eq!(block.extrinsics().len(), 2);

    // A second node holding the same genesis replays the block and reaches
    // the same state root. Execution stops on any mismatch.
    let mut replay = development_ext();
    replay.execute_with(|| {
        <Runtime as CoreApi<Block>>::execute_block(block.clone());
        assert_eq!(frame_system::Pallet::<Runtime>::block_number(), 1);
        assert_eq!(
            <Runtime as NonceApi<Block, AccountId, Nonce>>::account_nonce(alice()),
            1
        );
    });

    let mut checker = development_ext();
    checker.execute_with(|| {
        let result = <Runtime as BlockBuilderApi<Block>>::check_inherents(
            block.clone(),
            timestamp_inherent(6_000),
        );
        assert!(result.ok(), "the block carries a valid timestamp");
    });
}

#[test]
fn the_pool_interface_admits_a_signed_call_and_refuses_a_stale_nonce() {
    development_ext().execute_with(|| {
        let parent = frame_system::BlockHash::<Runtime>::get(0);
        let fresh = signed_by_alice(remark(b"hello"), 0);
        let validity = <Runtime as PoolApi<Block>>::validate_transaction(
            TransactionSource::External,
            fresh,
            parent,
        )
        .expect("a fresh signed call is valid");
        assert!(!validity.provides.is_empty());

        frame_system::Account::<Runtime>::mutate(alice(), |held| held.nonce = 3);
        let stale = signed_by_alice(remark(b"hello"), 1);
        let refused = <Runtime as PoolApi<Block>>::validate_transaction(
            TransactionSource::External,
            stale,
            parent,
        );
        assert_eq!(
            refused,
            Err(TransactionValidityError::Invalid(InvalidTransaction::Stale))
        );
    });
}

#[test]
fn the_fee_interfaces_answer_for_a_signed_call() {
    development_ext().execute_with(|| {
        let call = remark(b"fee estimate");
        let extrinsic = signed_by_alice(call.clone(), 0);
        let length = extrinsic.encoded_size() as u32;

        let info = <Runtime as PaymentApi<Block, Balance>>::query_info(extrinsic.clone(), length);
        assert!(info.weight.ref_time() > 0);
        let details = <Runtime as PaymentApi<Block, Balance>>::query_fee_details(extrinsic, length);
        assert!(details.inclusion_fee.is_some());

        let call_info = <Runtime as PaymentCallApi<Block, Balance, RuntimeCall>>::query_call_info(
            call.clone(),
            length,
        );
        // The call weight leaves out the transaction extension weight.
        assert!(call_info.weight.ref_time() > 0);
        assert!(call_info.weight.ref_time() < info.weight.ref_time());
        let call_details =
            <Runtime as PaymentCallApi<Block, Balance, RuntimeCall>>::query_call_fee_details(
                call, length,
            );
        assert!(call_details.inclusion_fee.is_some());

        let weight = info.weight;
        assert_eq!(
            <Runtime as PaymentApi<Block, Balance>>::query_weight_to_fee(weight),
            <Runtime as PaymentCallApi<Block, Balance, RuntimeCall>>::query_weight_to_fee(weight)
        );
        assert_eq!(
            <Runtime as PaymentApi<Block, Balance>>::query_length_to_fee(length),
            <Runtime as PaymentCallApi<Block, Balance, RuntimeCall>>::query_length_to_fee(length)
        );
    });
}

#[test]
fn the_metadata_interface_answers_every_published_version() {
    development_ext().execute_with(|| {
        let metadata = <Runtime as MetadataApi<Block>>::metadata();
        assert!(!metadata.is_empty());
        let versions = <Runtime as MetadataApi<Block>>::metadata_versions();
        assert!(versions.contains(&15));
        for version in versions.iter() {
            assert!(<Runtime as MetadataApi<Block>>::metadata_at_version(*version).is_some());
        }
        assert!(<Runtime as MetadataApi<Block>>::metadata_at_version(1).is_none());
    });
}

#[test]
fn the_session_interface_decodes_the_commit_key() {
    let key = Ed25519Keyring::Alice.public();
    let keys = SessionKeys {
        dcf: crate::DcfPublic::from(key),
    };
    let decoded = <Runtime as SessionApi<Block>>::decode_session_keys(keys.encode())
        .expect("the keys decode");
    assert_eq!(decoded.len(), 1);
    assert_eq!(decoded[0].0, key.0.to_vec());
    assert_eq!(decoded[0].1, crate::cerulea_key_crypto::KEY_TYPE);
    assert_eq!(
        <Runtime as SessionApi<Block>>::decode_session_keys(vec![1, 2, 3]),
        None
    );
}

#[test]
fn the_genesis_interface_names_and_builds_every_preset() {
    let names = <Runtime as GenesisApi<Block>>::preset_names();
    assert!(names.len() >= 4);
    for preset in names.iter() {
        let bytes = <Runtime as GenesisApi<Block>>::get_preset(&Some(preset.clone()))
            .expect("every named preset answers");
        let mut ext = sp_io::TestExternalities::new(Default::default());
        ext.execute_with(|| {
            assert_eq!(<Runtime as GenesisApi<Block>>::build_state(bytes), Ok(()));
        });
    }
    assert!(<Runtime as GenesisApi<Block>>::get_preset(&None).is_some());
    assert!(<Runtime as GenesisApi<Block>>::get_preset(&Some("unknown".into())).is_none());
}

#[test]
fn the_stake_and_inference_interfaces_answer_for_the_genesis_validator() {
    development_ext().execute_with(|| {
        assert!(<Runtime as PosApi<Block, AccountId, Balance>>::get_validator_stake(alice()) > 0);
        assert_eq!(
            <Runtime as PosApi<Block, AccountId, Balance>>::get_validator_stake(bob()),
            0
        );
        assert_eq!(
            <Runtime as PosApi<Block, AccountId, Balance>>::get_validator_score(alice()),
            80
        );
        assert_eq!(
            <Runtime as PosApi<Block, AccountId, Balance>>::get_validator_score(bob()),
            0
        );
        assert_eq!(
            <Runtime as PosApi<Block, AccountId, Balance>>::get_slashing_count(bob()),
            0
        );
        let _ = <Runtime as PosApi<Block, AccountId, Balance>>::get_active_validators();

        assert_eq!(
            <Runtime as PoiApi<Block, AccountId>>::get_inference_result(bob()),
            None
        );
        assert_eq!(
            <Runtime as PoiApi<Block, AccountId>>::get_challenge(bob()),
            None
        );
        assert_eq!(
            <Runtime as PoiApi<Block, AccountId>>::get_current_epoch(),
            0
        );
    });
}

#[test]
fn the_standing_interface_answers_for_the_genesis_validator() {
    development_ext().execute_with(|| {
        type Api = Runtime;
        let dcf = |validator: AccountId| {
            <Api as DcfApi<Block, AccountId, Balance, BlockNumber>>::is_validator_active(validator)
        };
        assert!(dcf(alice()));
        assert!(!dcf(bob()));

        assert_eq!(
            <Api as DcfApi<Block, AccountId, Balance, BlockNumber>>::get_api_version(),
            pallet_cerulea_dcf::DCF_API_VERSION
        );
        let scores = <Api as DcfApi<Block, AccountId, Balance, BlockNumber>>::get_validator_scores();
        assert!(scores.iter().any(|(validator, _)| *validator == alice()));
        assert_eq!(
            <Api as DcfApi<Block, AccountId, Balance, BlockNumber>>::get_current_epoch(),
            0
        );
        assert_eq!(
            <Api as DcfApi<Block, AccountId, Balance, BlockNumber>>::get_validator_stake_score(
                bob()
            ),
            0
        );
        assert_eq!(
            <Api as DcfApi<Block, AccountId, Balance, BlockNumber>>::get_validator_inference_score(
                bob()
            ),
            0
        );
        let (first, second) =
            <Api as DcfApi<Block, AccountId, Balance, BlockNumber>>::get_consensus_weights();
        assert_eq!(
            (first, second),
            (
                pallet_cerulea_dcf::Pallet::<Runtime>::pos_weight(),
                pallet_cerulea_dcf::Pallet::<Runtime>::poi_weight()
            )
        );
        assert!(
            <Api as DcfApi<Block, AccountId, Balance, BlockNumber>>::get_validator_score_history(
                bob()
            )
            .is_empty()
        );
        assert_eq!(
            <Api as DcfApi<Block, AccountId, Balance, BlockNumber>>::get_validator_participation(
                bob()
            ),
            (0, 0)
        );
        let (authored, missed) =
            <Api as DcfApi<Block, AccountId, Balance, BlockNumber>>::get_validator_participation(
                alice(),
            );
        assert_eq!((authored, missed), (0, 0));
        assert!(
            <Api as DcfApi<Block, AccountId, Balance, BlockNumber>>::get_active_validators()
                .contains(&alice())
        );
        assert_eq!(
            <Api as DcfApi<Block, AccountId, Balance, BlockNumber>>::get_validator_last_active(
                bob()
            ),
            0
        );
        assert!(
            <Api as DcfApi<Block, AccountId, Balance, BlockNumber>>::get_validator_profile(alice())
                .is_some()
        );
        assert!(
            <Api as DcfApi<Block, AccountId, Balance, BlockNumber>>::get_validator_profile(bob())
                .is_none()
        );
        let _ = <Api as DcfApi<Block, AccountId, Balance, BlockNumber>>::get_validator_score_breakdown(alice());
        let _ = <Api as DcfApi<Block, AccountId, Balance, BlockNumber>>::get_validator_uptime(alice());
        assert!(
            <Api as DcfApi<Block, AccountId, Balance, BlockNumber>>::get_slashing_history(alice())
                .is_empty()
        );
        let _ = <Api as DcfApi<Block, AccountId, Balance, BlockNumber>>::get_system_constants();
        assert_eq!(
            <Api as DcfApi<Block, AccountId, Balance, BlockNumber>>::get_validator_cooldown_status(
                bob()
            ),
            None
        );
        let _ = <Api as DcfApi<Block, AccountId, Balance, BlockNumber>>::get_validator_detailed_cooldown_status(bob());
        let _ = <Api as DcfApi<Block, AccountId, Balance, BlockNumber>>::get_inference_result(bob());
        assert!(
            <Api as DcfApi<Block, AccountId, Balance, BlockNumber>>::get_epoch_history(999)
                .is_none()
        );
        let _ = <Api as DcfApi<Block, AccountId, Balance, BlockNumber>>::get_recent_epochs(3);
        let _ = <Api as DcfApi<Block, AccountId, Balance, BlockNumber>>::get_governance_mode();
        let _ = <Api as DcfApi<Block, AccountId, Balance, BlockNumber>>::get_validator_consensus_contribution(alice());
        let epoch = <Api as DcfApi<Block, AccountId, Balance, BlockNumber>>::get_epoch_config();
        assert_eq!(epoch.blocks_per_epoch, 100);
        let _ = <Api as DcfApi<Block, AccountId, Balance, BlockNumber>>::get_validator_epoch_stats(alice(), 0);
        assert!(
            <Api as DcfApi<Block, AccountId, Balance, BlockNumber>>::get_total_validators_count()
                >= 1
        );
        let _ = <Api as DcfApi<Block, AccountId, Balance, BlockNumber>>::get_validator_set_info();
        let ranked = <Api as DcfApi<Block, AccountId, Balance, BlockNumber>>::get_validators_by_score();
        assert!(ranked.iter().any(|(validator, _)| *validator == alice()));
        assert_eq!(
            <Api as DcfApi<Block, AccountId, Balance, BlockNumber>>::get_misbehavior_report_count(
                alice()
            ),
            0
        );
        assert!(
            <Api as DcfApi<Block, AccountId, Balance, BlockNumber>>::get_misbehavior_reporters(
                alice()
            )
            .is_empty()
        );
        assert_eq!(
            <Api as DcfApi<Block, AccountId, Balance, BlockNumber>>::get_misbehavior_evidence(
                alice(),
                bob()
            ),
            None
        );
        let _ = <Api as DcfApi<Block, AccountId, Balance, BlockNumber>>::is_validator_at_risk(alice());
        assert_eq!(
            <Api as DcfApi<Block, AccountId, Balance, BlockNumber>>::get_validator_stake(alice()),
            pallet_cerulea_pos::Pallet::<Runtime>::stake(alice())
        );
    });
}

#[test]
fn the_standing_interface_reports_the_leave_request_window() {
    development_ext().execute_with(|| {
        type Api = Runtime;
        assert_eq!(
            <Api as DcfApi<Block, AccountId, Balance, BlockNumber>>::get_leave_request_status(
                alice()
            ),
            None
        );
        assert_eq!(
            <Api as DcfApi<Block, AccountId, Balance, BlockNumber>>::get_validator_leave_request(
                alice()
            ),
            None
        );
        pallet_cerulea_dvf::ValidatorLeaveRequests::<Runtime>::insert(alice(), 1);
        let cooldown: u32 =
            <<Runtime as pallet_cerulea_pos::Config>::LeaveCooldown as frame_support::traits::Get<u32>>::get();
        assert_eq!(
            <Api as DcfApi<Block, AccountId, Balance, BlockNumber>>::get_validator_leave_request(
                alice()
            ),
            Some(1)
        );
        assert_eq!(
            <Api as DcfApi<Block, AccountId, Balance, BlockNumber>>::get_leave_request_status(
                alice()
            ),
            Some((1, 1 + cooldown, false))
        );
        frame_system::Pallet::<Runtime>::set_block_number(1 + cooldown);
        assert_eq!(
            <Api as DcfApi<Block, AccountId, Balance, BlockNumber>>::get_leave_request_status(
                alice()
            ),
            Some((1, 1 + cooldown, true))
        );
    });
}

#[test]
fn the_standing_interface_answers_the_engine_figures() {
    development_ext().execute_with(|| {
        type Api = Runtime;
        let figures = <Api as DcfApi<Block, AccountId, Balance, BlockNumber>>::get_epoch_manager_config();
        assert_eq!(figures.0, 30);
        assert_eq!(figures.1, 80);
        assert_eq!(figures.2, 50);
        assert_eq!(figures.3, 90);
        assert_eq!(figures.4, 10);
        assert_eq!(figures.5, 2);
        assert_eq!(figures.6, 50);
        assert_eq!(figures.7, 80);
        assert_eq!(figures.8, 5);
        assert_eq!(figures.10, 5);
        assert_eq!(
            <Api as DcfApi<Block, AccountId, Balance, BlockNumber>>::get_epoch_length(),
            <<Runtime as pallet_cerulea_dcf::Config>::EpochLength as frame_support::traits::Get<u32>>::get()
        );
        let not_active = <<Runtime as pallet_cerulea_dcf::Config>::AuthorNotActiveErrorCode as frame_support::traits::Get<u8>>::get();
        assert_eq!(
            <Api as DcfApi<Block, AccountId, Balance, BlockNumber>>::validate_block_author_strict(
                1,
                bob()
            ),
            Err(not_active)
        );
        let _ = <Api as DcfApi<Block, AccountId, Balance, BlockNumber>>::validate_block_author_strict(1, alice());
        let _ = <Api as DcfApi<Block, AccountId, Balance, BlockNumber>>::validate_expected_author(1, alice());
        let _ = <Api as DcfApi<Block, AccountId, Balance, BlockNumber>>::get_expected_author(1);
        <Api as DcfApi<Block, AccountId, Balance, BlockNumber>>::validate_block_author(1, alice());
        let _ = <Api as DcfApi<Block, AccountId, Balance, BlockNumber>>::report_author_mismatch(
            1,
            Some(alice()),
            bob(),
        );
        let _ = <Api as DcfApi<Block, AccountId, Balance, BlockNumber>>::report_successful_block_authorship(1, alice());
        let _ = <Api as DcfApi<Block, AccountId, Balance, BlockNumber>>::report_missed_block(
            1,
            alice(),
        );
        assert!(
            !<Api as DcfApi<Block, AccountId, Balance, BlockNumber>>::get_governance_config()
                .is_empty()
        );
        let _ = <Api as DcfApi<Block, AccountId, Balance, BlockNumber>>::get_parameter_value(
            b"epoch_length".to_vec(),
        );
        let _ = <Api as DcfApi<Block, AccountId, Balance, BlockNumber>>::validate_parameter_value(
            b"epoch_length".to_vec(),
            100u32.encode(),
        );
        assert_eq!(
            <Api as DcfApi<Block, AccountId, Balance, BlockNumber>>::get_latest_invariant_report(),
            None
        );
        assert_eq!(
            <Api as DcfApi<Block, AccountId, Balance, BlockNumber>>::get_invariant_report_for_epoch(
                0
            ),
            None
        );
        assert!(!<Api as DcfApi<Block, AccountId, Balance, BlockNumber>>::has_invariant_violations());
        assert!(
            !<Api as DcfApi<Block, AccountId, Balance, BlockNumber>>::get_system_metrics()
                .is_empty()
        );
        assert!(
            !<Api as DcfApi<Block, AccountId, Balance, BlockNumber>>::get_performance_indicators()
                .is_empty()
        );
        let _ = <Api as DcfApi<Block, AccountId, Balance, BlockNumber>>::get_metrics_last_updated();
    });
}

#[test]
fn the_registry_interface_answers_the_finality_figures() {
    development_ext().execute_with(|| {
        type Api = Runtime;
        assert_eq!(
            <Api as DvfApi<Block, BlockNumber, AccountId, H256>>::get_finality_threshold_perbill(),
            sp_runtime::Perbill::from_percent(67)
        );
        assert_eq!(
            <Api as DvfApi<Block, BlockNumber, AccountId, H256>>::get_finality_checkpoint_interval(
            ),
            10
        );
        assert_eq!(
            <Api as DvfApi<Block, BlockNumber, AccountId, H256>>::get_vote_retention_rounds(),
            20
        );
        assert_eq!(
            <Api as DvfApi<Block, BlockNumber, AccountId, H256>>::get_dvf_finalized_block(),
            0
        );
        assert_eq!(
            <Api as DvfApi<Block, BlockNumber, AccountId, H256>>::get_current_epoch(),
            0
        );
        assert!(
            <Api as DvfApi<Block, BlockNumber, AccountId, H256>>::get_validator_set()
                .contains(&alice())
        );
        let weights = <Api as DvfApi<Block, BlockNumber, AccountId, H256>>::get_validator_weights();
        assert!(weights.iter().any(|(validator, _)| *validator == alice()));
        let _ = <Api as DvfApi<Block, BlockNumber, AccountId, H256>>::get_finality_info(1);
        let _ = <Api as DvfApi<Block, BlockNumber, AccountId, H256>>::get_validator_set_id();
        let _ = <Api as DvfApi<Block, BlockNumber, AccountId, H256>>::get_current_round();
        let _ =
            <Api as DvfApi<Block, BlockNumber, AccountId, H256>>::get_validator_set_id_changed_at();
        assert_eq!(
            <Api as DvfApi<Block, BlockNumber, AccountId, H256>>::get_vote_tally(
                H256::repeat_byte(3)
            ),
            0
        );
    });
}

#[test]
fn the_authoring_interface_seats_the_genesis_validator() {
    development_ext().execute_with(|| {
        type Api = Runtime;
        let keys = <Api as AuthoringApi<Block, AccountId>>::active_validator_keys();
        let (validator, set) = keys
            .iter()
            .find(|(validator, _)| *validator == alice())
            .expect("the genesis validator carries keys");
        assert_eq!(*validator, alice());
        assert_eq!(set.seal, Ed25519Keyring::Alice.public().0);

        let seats = <Api as AuthoringApi<Block, AccountId>>::slot_committee(0, [0u8; 32]);
        assert!(!seats.is_empty());
        assert_eq!(
            <Api as AuthoringApi<Block, AccountId>>::authoring_epoch(),
            0
        );
        assert_eq!(
            <Api as AuthoringApi<Block, AccountId>>::published_epoch_draw(bob(), 0),
            None
        );
    });
}

#[test]
fn the_platform_interface_reaches_every_platform_read() {
    development_ext().execute_with(|| {
        type Api = Runtime;
        let deployment = starting_deployment();
        assert_eq!(
            <Api as PlatformApi<Block, AccountId>>::deployments(),
            vec![deployment]
        );
        assert_eq!(
            <Api as PlatformApi<Block, AccountId>>::blueprint_address(deployment),
            Some(starting_blueprint().id().0)
        );
        let encoded = <Api as PlatformApi<Block, AccountId>>::blueprint(deployment)
            .expect("the Blueprint is installed");
        assert_eq!(
            cerulea_blueprint::Blueprint::decode(&mut encoded.as_slice()).ok(),
            Some(starting_blueprint())
        );
        assert!(<Api as PlatformApi<Block, AccountId>>::type_names(deployment).is_empty());
        assert_eq!(
            <Api as PlatformApi<Block, AccountId>>::type_definition(deployment, b"x".to_vec()),
            None
        );
        assert!(<Api as PlatformApi<Block, AccountId>>::collections(deployment).is_empty());
        assert_eq!(
            <Api as PlatformApi<Block, AccountId>>::record(
                deployment,
                b"c".to_vec(),
                b"k".to_vec()
            ),
            None
        );
        assert!(<Api as PlatformApi<Block, AccountId>>::records(
            deployment,
            b"c".to_vec(),
            Vec::new(),
            5
        )
        .is_empty());
        // A collection the Blueprint never declared carries no root.
        assert_eq!(
            <Api as PlatformApi<Block, AccountId>>::collection_root(deployment, b"c".to_vec()),
            None
        );
        assert!(
            !<Api as PlatformApi<Block, AccountId>>::verify_record_proof(Vec::new(), [0u8; 32])
        );
        let refusal = <Api as PlatformApi<Block, AccountId>>::resolve_permission(
            deployment,
            bob(),
            b"c".to_vec(),
            b"read".to_vec(),
        );
        assert!(!refusal.admitted);
        let role = <Api as PlatformApi<Block, AccountId>>::simulate_as_role(
            deployment,
            b"clerk".to_vec(),
            b"c".to_vec(),
        );
        assert!(!role.read);
        let view =
            <Api as PlatformApi<Block, AccountId>>::evaluate_view(deployment, bob(), b"v".to_vec());
        assert!(!view.evaluated);
        assert!(<Api as PlatformApi<Block, AccountId>>::workflow_names(deployment).is_empty());
        assert_eq!(
            <Api as PlatformApi<Block, AccountId>>::workflow_instance(deployment, b"w".to_vec(), 0),
            None
        );
        assert!(<Api as PlatformApi<Block, AccountId>>::workflow_instances(
            deployment,
            b"w".to_vec(),
            5
        )
        .is_empty());
        assert!(<Api as PlatformApi<Block, AccountId>>::proposals(deployment, 5).is_empty());
        assert_eq!(
            <Api as PlatformApi<Block, AccountId>>::proposal(deployment, 0),
            None
        );
        assert!(<Api as PlatformApi<Block, AccountId>>::proposal_votes(deployment, 0).is_empty());
        assert!(<Api as PlatformApi<Block, AccountId>>::executed_changes(deployment, 5).is_empty());
        let length = <Api as PlatformApi<Block, AccountId>>::audit_length(deployment);
        assert_eq!(
            <Api as PlatformApi<Block, AccountId>>::audit_entries(deployment, 0, 512).len() as u64,
            length
        );
        let _ = <Api as PlatformApi<Block, AccountId>>::audit_by_actor(deployment, alice(), 5);
        let licence = <Api as PlatformApi<Block, AccountId>>::licence_state(deployment)
            .expect("the starting deployment holds a licence");
        assert!(licence.production_permitted);
        assert!(<Api as PlatformApi<Block, AccountId>>::production_permitted());
        assert!(<Api as PlatformApi<Block, AccountId>>::oracle_feeds(deployment).is_empty());
        assert_eq!(
            <Api as PlatformApi<Block, AccountId>>::oracle_value(deployment, b"rate".to_vec()),
            None
        );
        assert!(<Api as PlatformApi<Block, AccountId>>::versions(deployment).is_empty());
        assert!(<Api as PlatformApi<Block, AccountId>>::rollback_windows(deployment).is_empty());
        assert_eq!(
            <Api as PlatformApi<Block, AccountId>>::operator_accounts(deployment),
            vec![crate::genesis_config_presets::cerulea_operator()]
        );
        assert_eq!(
            <Api as PlatformApi<Block, AccountId>>::consensus_profile().financial_percent,
            67
        );
        assert!(<Api as PlatformApi<Block, AccountId>>::custody_history(alice(), 5).is_empty());
        assert_eq!(
            <Api as PlatformApi<Block, AccountId>>::action_manifest(deployment, b"a".to_vec()),
            None
        );
        assert!(<Api as PlatformApi<Block, AccountId>>::action_names(deployment).is_empty());
    });
}
