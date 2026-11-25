//! A relayed submission dispatches the call the account authorised.
//!
//! A relayer carries the cost of a submission for an account. The account
//! signs an authorisation naming the content hash of one inner call. The
//! relayer module checks the signature and dispatches the inner call with the
//! authoriser as the origin. Permission therefore resolves against the
//! authoriser, and the history names the authoriser.

mod support;

use cerulea_runtime::{AccountId, Runtime, RuntimeCall, RuntimeEvent, RuntimeOrigin};
use codec::Encode;
use frame_support::{assert_noop, assert_ok, BoundedVec};
use pallet_cerulea_relayer::Authorisation;
use pallet_cerulea_schema::primitives::DeploymentId;
use sp_core::Pair;
use sp_keyring::Ed25519Keyring;
use support::*;

/// The fragment the tests install.
const FRAGMENT: &str = "188-land-title-registry.json";

/// The action the tests relay.
const ACTION: &str = "land_title_registry_record_parcel";

type Relayer = pallet_cerulea_relayer::Pallet<Runtime>;

/// The relayer account that carries the cost.
fn relayer() -> AccountId {
    Ed25519Keyring::Bob.to_account_id()
}

/// Install the fragment, the licence and the relayer budget.
///
/// The answer carries the deployment and the inner call that records one
/// parcel.
fn relayed_deployment() -> (DeploymentId, RuntimeCall) {
    let library = fragment_library();
    assert!(
        !library.is_empty(),
        "The content repository sits beside the chain. Clone it before the run."
    );
    let blueprint = resolve(FRAGMENT, &library).expect("The fragment resolves.");
    let deployment = install(&blueprint).expect("The Blueprint installs.");
    seed_identity_and_permissions(deployment, &blueprint);
    install_licence(deployment);

    assert_ok!(Relayer::register_signing_key(
        RuntimeOrigin::signed(actor()),
        deployment,
        Ed25519Keyring::Alice.pair().public().0
    ));
    assert_ok!(Relayer::register_sponsor(
        RuntimeOrigin::signed(actor()),
        deployment,
        relayer(),
        100
    ));
    assert_ok!(Relayer::set_quota(
        RuntimeOrigin::signed(actor()),
        deployment,
        3_600_000,
        100
    ));

    let action = blueprint
        .actions
        .iter()
        .find(|held| held.id.as_str() == ACTION)
        .expect("The fragment declares the action.");
    let arguments =
        sample_arguments(action, &blueprint.types).expect("Every parameter takes a value.");
    let call = RuntimeCall::Logic(pallet_cerulea_logic::Call::execute_action {
        deployment,
        action: name(ACTION),
        arguments: BoundedVec::try_from(arguments.encode()).expect("The arguments fit."),
    });
    (deployment, call)
}

/// The authorisation one account signs for one inner call.
fn authorisation_for(
    deployment: DeploymentId,
    authoriser: AccountId,
    call: &RuntimeCall,
    nonce: u64,
) -> Authorisation<AccountId> {
    Authorisation {
        deployment,
        actor: authoriser,
        action: name(ACTION),
        payload_hash: sp_io::hashing::blake2_256(&call.encode()),
        valid_until: pallet_cerulea_relayer::Pallet::<Runtime>::now_millis() + 60_000,
        nonce,
    }
}

/// A relayed insert lands under the authoriser.
///
/// The logic module runs the action as the authoriser, the state module
/// records the authoriser as the author and the relayer module records the
/// relayer beside it.
#[test]
fn a_relayed_insert_lands_under_the_authoriser() {
    development_ext().execute_with(|| {
        let (deployment, call) = relayed_deployment();
        let held = authorisation_for(deployment, actor(), &call, 1);
        let signature = Ed25519Keyring::Alice.pair().sign(&held.message()).0;

        assert_ok!(Relayer::submit_on_behalf(
            RuntimeOrigin::signed(relayer()),
            deployment,
            held,
            signature,
            Box::new(call)
        ));

        let events: Vec<RuntimeEvent> = frame_system::Pallet::<Runtime>::events()
            .into_iter()
            .map(|record| record.event)
            .collect();
        let inserted_by: Vec<AccountId> = events
            .iter()
            .filter_map(|event| match event {
                RuntimeEvent::State(pallet_cerulea_state::Event::RecordInserted {
                    actor, ..
                }) => Some(actor.clone()),
                _ => None,
            })
            .collect();
        assert!(
            !inserted_by.is_empty(),
            "The relayed action wrote a record."
        );
        assert!(
            inserted_by.iter().all(|author| *author == actor()),
            "Every record names the authoriser as its author."
        );
        assert!(events.iter().any(|event| matches!(
            event,
            RuntimeEvent::Logic(pallet_cerulea_logic::Event::ActionExecuted { actor: who, .. })
                if *who == actor()
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            RuntimeEvent::Relayer(pallet_cerulea_relayer::Event::SubmittedOnBehalf {
                actor: who,
                relayer: carrier,
                ..
            }) if *who == actor() && *carrier == relayer()
        )));
    });
}

/// A relayed write the authoriser holds no permission for is refused.
///
/// The authoriser holds no role over the collection. The inner call meets the
/// permission check under the authoriser, so the whole submission refuses and
/// leaves nothing behind. A call other than the one the authorisation names
/// fails before dispatch.
#[test]
fn a_relayed_write_without_permission_is_refused() {
    development_ext().execute_with(|| {
        let (deployment, call) = relayed_deployment();
        let stranger = Ed25519Keyring::Charlie;
        let stranger_account = stranger.to_account_id();
        assert_ok!(
            pallet_cerulea_identity::Pallet::<Runtime>::register_account(
                RuntimeOrigin::signed(stranger_account.clone()),
                stranger_account.clone(),
                [9u8; 32],
            )
        );
        assert_ok!(Relayer::register_signing_key(
            RuntimeOrigin::signed(stranger_account.clone()),
            deployment,
            stranger.pair().public().0
        ));

        let held = authorisation_for(deployment, stranger_account.clone(), &call, 1);
        let signature = stranger.pair().sign(&held.message()).0;
        assert_noop!(
            Relayer::submit_on_behalf(
                RuntimeOrigin::signed(relayer()),
                deployment,
                held,
                signature,
                Box::new(call.clone())
            ),
            pallet_cerulea_logic::Error::<Runtime>::PermissionDenied
        );

        // The authoriser signs for one call and the relayer carries another.
        let held = authorisation_for(deployment, actor(), &call, 2);
        let signature = Ed25519Keyring::Alice.pair().sign(&held.message()).0;
        let other = RuntimeCall::System(frame_system::Call::remark {
            remark: b"another call".to_vec(),
        });
        assert_noop!(
            Relayer::submit_on_behalf(
                RuntimeOrigin::signed(relayer()),
                deployment,
                held,
                signature,
                Box::new(other)
            ),
            pallet_cerulea_relayer::Error::<Runtime>::CallMismatch
        );
    });
}
