//! Coverage tests for the validation and finality pallet.
//!
//! Each group drives one part of the pallet through the mock runtime. The
//! groups cover the admission calls, the licence path, the key records, the
//! slot committee, the block hook, the round timeout, the vote checks, the
//! unsigned validation and the registry traits the other pallets read.

use crate::engines::admission::licence::{attestation_payload, LicenceAttestation, LicenceClass};
use crate::{mock::*, Error, Event};
use codec::Encode;
use frame_support::traits::{ConstU32, Hooks};
use frame_support::{assert_noop, assert_ok, BoundedVec};
use pallet_cerulea_dcf::traits::{ValidatorRegistryProvider, WeightFreezer};
use pallet_cerulea_dcf::{EjectionReason, EpochStats, ValidatorAction, ValidatorStatus};
use pallet_cerulea_pos::ValidatorHandler;
use sp_core::crypto::VrfSecret;
use sp_core::{ed25519, sr25519, Pair, H256};
use sp_runtime::traits::ValidateUnsigned;
use sp_runtime::transaction_validity::{
    InvalidTransaction, TransactionSource, TransactionValidityError,
};
use sp_runtime::{BuildStorage, DispatchError, DispatchResult, MultiSignature};

/// One vote as the mock runtime carries it.
type Vote = crate::DvfVote<H256, AccountId, MultiSignature>;

/// One licence attestation as the mock runtime carries it.
type Licence = LicenceAttestation<AccountId, u64>;

/// Call one registry trait function on the pallet.
///
/// The pallet carries inherent functions and two traits with shared names.
/// The macro names the trait, so each call reaches the trait body.
macro_rules! provider {
    ($method:ident $(, $arg:expr)* $(,)?) => {
        <Registry as ValidatorRegistryProvider<AccountId, u128, u64>>::$method($($arg),*)
    };
}

/// Call one stake handler function on the pallet.
macro_rules! handler {
    ($method:ident $(, $arg:expr)* $(,)?) => {
        <Registry as ValidatorHandler<AccountId, u128>>::$method($($arg),*)
    };
}

// ----------------------------------------------------------------------------
// Shared helpers
// ----------------------------------------------------------------------------

/// The validator bound the mock runtime sets.
const SEAT_BOUND: u32 = 100;

/// The leave cooldown the mock runtime sets, in blocks.
const COOLDOWN: u32 = 1_000;

/// The stake one validator reserves on joining.
const MIN_STAKE: u128 = 1_000;

fn bounded(accounts: Vec<AccountId>) -> BoundedVec<AccountId, ConstU32<SEAT_BOUND>> {
    BoundedVec::try_from(accounts).expect("the list fits the validator bound")
}

/// A distinct account for each index, cheaper than a key derivation.
fn filler(index: u32) -> AccountId {
    let mut bytes = [7u8; 32];
    bytes[..4].copy_from_slice(&index.to_le_bytes());
    AccountId::from(bytes)
}

fn seat_in_set(accounts: &[AccountId]) {
    crate::ValidatorSet::<Test>::put(bounded(accounts.to_vec()));
}

fn seat_active(accounts: &[AccountId]) {
    crate::ActiveValidators::<Test>::put(bounded(accounts.to_vec()));
}

fn fund(who: &AccountId, amount: u128) {
    assert_ok!(Balances::force_set_balance(
        RuntimeOrigin::root(),
        who.clone(),
        amount
    ));
}

fn join(who: &AccountId) -> DispatchResult {
    Registry::join_validators(RuntimeOrigin::signed(who.clone()), None)
}

/// Move the chain to one block and run the registry hook.
fn step(block: u64) {
    System::set_block_number(block);
    Registry::on_initialize(block);
}

/// One registry state record with the stated score and last active epoch.
fn state(final_score: u64, last_active_epoch: u32) -> crate::ValidatorState<Test> {
    crate::ValidatorState {
        last_active_epoch,
        current: EpochStats {
            epoch: last_active_epoch,
            stake_score: 1_000,
            inference_score: 0,
            final_score,
            authored_blocks: 0,
            missed_blocks: 0,
        },
        history: BoundedVec::new(),
        uptime: 0,
        inference_success_count: 0,
        participation_rate: 0,
        inference_count: 0,
        last_active_block: 0,
        name: None,
        trust_score: 0,
    }
}

/// The key set the mock genesis gives the validator at one seed.
fn genesis_keys(seed: u8) -> crate::keys::ValidatorKeySet {
    crate::keys::ValidatorKeySet::new(
        sr25519::Pair::from_seed(&[seed; 32]).public().0,
        ed25519::Pair::from_seed(&[seed; 32]).public().0,
    )
}

fn vote_payload(vote: &Vote) -> Vec<u8> {
    let mut payload = Vec::new();
    vote.epoch_id.encode_to(&mut payload);
    vote.validator_set_id.encode_to(&mut payload);
    vote.round_id.encode_to(&mut payload);
    vote.block_number.encode_to(&mut payload);
    vote.block_hash.encode_to(&mut payload);
    vote.validator_account.encode_to(&mut payload);
    payload
}

/// One vote the named account signed over the real vote payload.
fn signed_vote(name: &str, set_id: u32, round: u32, height: u32, hash: H256) -> Vote {
    let pair = sr25519::Pair::from_string(&format!("//{}", name), None)
        .expect("the derivation path is valid");
    let mut vote = crate::DvfVote {
        epoch_id: 1,
        validator_set_id: set_id,
        round_id: round,
        block_number: height,
        block_hash: hash,
        validator_account: account_key(name),
        signature: MultiSignature::Sr25519(sr25519::Signature::from_raw([0u8; 64])),
    };
    vote.signature = MultiSignature::Sr25519(pair.sign(&vote_payload(&vote)));
    vote
}

/// The keys and possession proofs one validator presents.
struct Possession {
    draw: [u8; 32],
    seal: [u8; 32],
    draw_proof: [u8; 64],
    seal_proof: [u8; 64],
}

fn possession(who: &AccountId, draw_seed: u8, seal_seed: u8) -> Possession {
    let payload = crate::keys::possession_payload(&who.encode());
    let draw = sr25519::Pair::from_seed(&[draw_seed; 32]);
    let seal = ed25519::Pair::from_seed(&[seal_seed; 32]);
    Possession {
        draw: draw.public().0,
        seal: seal.public().0,
        draw_proof: draw.sign(&payload).0,
        seal_proof: seal.sign(&payload).0,
    }
}

fn register(who: &AccountId, held: &Possession) -> DispatchResult {
    Registry::register_validator_keys(
        RuntimeOrigin::signed(who.clone()),
        held.draw,
        held.seal,
        held.draw_proof,
        held.seal_proof,
    )
}

/// One epoch draw the draw key at one seed produced, with its output.
fn epoch_draw(seed: u8, epoch: u32) -> (crate::keys::DrawProofBytes, [u8; 32]) {
    let pair = sr25519::Pair::from_seed(&[seed; 32]);
    let signature = pair.vrf_sign(&cerulea_primitives::epoch_draw_sign_data(epoch));
    let proof = crate::keys::write_proof(&signature).expect("the proof encodes to its length");
    let output = cerulea_primitives::verify_epoch_draw(&pair.public(), epoch, &signature)
        .expect("a fresh draw verifies");
    (proof, output)
}

/// The key the mock control plane signs licences with.
fn control_plane() -> ed25519::Pair {
    ed25519::Pair::from_seed(&[42u8; 32])
}

fn licence(
    holder: &AccountId,
    class: LicenceClass,
    issued_at: u64,
    expires_at: u64,
    nonce: u64,
) -> Licence {
    LicenceAttestation {
        holder: holder.clone(),
        class,
        issued_at,
        expires_at,
        nonce,
    }
}

fn signed_by(pair: &ed25519::Pair, attestation: &Licence) -> [u8; 64] {
    pair.sign(&attestation_payload(attestation)).0
}

fn install_licence_key(class: LicenceClass) {
    assert_ok!(Registry::set_validator_licence_key(
        RuntimeOrigin::root(),
        control_plane().public().0,
        class
    ));
}

fn present(who: &AccountId, attestation: Licence, signature: [u8; 64]) -> DispatchResult {
    Registry::submit_validator_licence(RuntimeOrigin::signed(who.clone()), attestation, signature)
}

fn ext_from(genesis: crate::GenesisConfig<Test>) -> sp_io::TestExternalities {
    let mut storage = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .expect("the system genesis builds");
    genesis
        .assimilate_storage(&mut storage)
        .expect("the registry genesis builds");
    storage.into()
}

// ----------------------------------------------------------------------------
// Joining and leaving the validator set
// ----------------------------------------------------------------------------

/// One validator walks the whole seat lifecycle.
///
/// The validator joins, asks to leave, cancels, asks again, waits out the
/// cooldown, meets the rejoin cooldown and finally rejoins.
#[test]
fn a_validator_joins_leaves_cancels_and_rejoins_after_the_cooldown() {
    new_test_ext().execute_with(|| {
        let dave = account_key("Dave");
        System::set_block_number(1);
        fund(&dave, 10_000);

        assert_ok!(join(&dave));
        assert!(Registry::validator_set().contains(&dave));
        assert!(!Registry::active_validators().contains(&dave));
        assert_eq!(pallet_cerulea_pos::Stake::<Test>::get(&dave), MIN_STAKE);
        assert_eq!(
            pallet_cerulea_pos::Validators::<Test>::get(&dave),
            Some(true)
        );
        assert_eq!(Balances::reserved_balance(&dave), MIN_STAKE);
        assert_eq!(crate::ValidatorJoinTime::<Test>::get(&dave), Some(1));
        let held = crate::ValidatorStates::<Test>::get(&dave).expect("the join records a state");
        assert_eq!(held.current.final_score, 50);
        assert_eq!(held.current.stake_score, 1_000);
        assert_eq!(held.trust_score, 5_000);
        System::assert_has_event(
            Event::<Test>::ValidatorJoined {
                validator: dave.clone(),
                stake_amount: MIN_STAKE,
            }
            .into(),
        );

        // A seated validator joins once.
        assert_noop!(
            join(&dave),
            pallet_cerulea_pos::Error::<Test>::ValidatorAlreadyExists
        );

        // The validator asks to leave and loses the active seat at once.
        seat_active(&[dave.clone()]);
        assert_ok!(Registry::leave_validators(RuntimeOrigin::signed(
            dave.clone()
        )));
        assert_eq!(crate::ValidatorLeaveRequests::<Test>::get(&dave), Some(1));
        assert!(!Registry::active_validators().contains(&dave));
        assert_eq!(
            pallet_cerulea_pos::Validators::<Test>::get(&dave),
            Some(false)
        );
        System::assert_has_event(
            Event::<Test>::ValidatorLeaveRequested {
                validator: dave.clone(),
                cooldown_expires_at: 1 + COOLDOWN,
            }
            .into(),
        );
        assert_noop!(
            Registry::leave_validators(RuntimeOrigin::signed(dave.clone())),
            pallet_cerulea_pos::Error::<Test>::LeaveCooldownActive
        );
        assert_noop!(
            join(&dave),
            pallet_cerulea_pos::Error::<Test>::LeaveCooldownActive
        );

        // A cancel inside the cooldown restores the active seat.
        System::set_block_number(500);
        assert_ok!(Registry::cancel_leave_request(RuntimeOrigin::signed(
            dave.clone()
        )));
        assert_eq!(crate::ValidatorLeaveRequests::<Test>::get(&dave), None);
        assert!(Registry::active_validators().contains(&dave));
        assert_eq!(
            pallet_cerulea_pos::Validators::<Test>::get(&dave),
            Some(true)
        );
        System::assert_has_event(
            Event::<Test>::ValidatorLeaveCancelled {
                validator: dave.clone(),
            }
            .into(),
        );

        // A second request outlives its cooldown and the cancel closes.
        System::set_block_number(600);
        assert_ok!(Registry::leave_validators(RuntimeOrigin::signed(
            dave.clone()
        )));
        System::set_block_number(600 + COOLDOWN as u64);
        assert_noop!(
            Registry::cancel_leave_request(RuntimeOrigin::signed(dave.clone())),
            pallet_cerulea_pos::Error::<Test>::LeaveCooldownActive
        );

        // The block hook removes the seat and returns the stake.
        step(600 + COOLDOWN as u64);
        assert!(!Registry::validator_set().contains(&dave));
        assert_eq!(Balances::reserved_balance(&dave), 0);
        assert_eq!(pallet_cerulea_pos::Stake::<Test>::get(&dave), 0);
        assert_eq!(pallet_cerulea_pos::Validators::<Test>::get(&dave), None);
        assert_eq!(crate::ValidatorJoinTime::<Test>::get(&dave), None);
        assert_eq!(crate::ValidatorLeaveRequests::<Test>::get(&dave), None);
        assert_eq!(
            crate::RecentlyRemovedValidators::<Test>::get(&dave),
            Some(1_600)
        );
        System::assert_has_event(
            Event::<Test>::ValidatorLeft {
                validator: dave.clone(),
            }
            .into(),
        );

        // The rejoin cooldown holds the validator out for a while.
        System::set_block_number(1_700);
        assert_noop!(
            join(&dave),
            pallet_cerulea_pos::Error::<Test>::CooldownActive
        );

        // After the cooldown the validator rejoins and keeps its state.
        crate::ValidatorStates::<Test>::mutate(&dave, |held| {
            if let Some(held) = held {
                held.trust_score = 7_777;
            }
        });
        System::set_block_number(2_600);
        assert_ok!(join(&dave));
        assert_eq!(crate::RecentlyRemovedValidators::<Test>::get(&dave), None);
        assert!(Registry::validator_set().contains(&dave));
        assert_eq!(
            crate::ValidatorStates::<Test>::get(&dave).map(|held| held.trust_score),
            Some(7_777)
        );
    });
}

/// A join without the minimum free balance is refused.
#[test]
fn a_join_below_the_minimum_stake_is_refused() {
    new_test_ext().execute_with(|| {
        let eve = account_key("Eve");
        fund(&eve, 800);
        assert_noop!(
            join(&eve),
            pallet_cerulea_pos::Error::<Test>::InsufficientStake
        );
    });
}

/// A join into a full validator set is refused and the stake stays free.
#[test]
fn a_join_into_a_full_set_is_refused() {
    new_test_ext().execute_with(|| {
        let fillers: Vec<AccountId> = (0..SEAT_BOUND).map(filler).collect();
        seat_in_set(&fillers);
        let frank = account_key("Frank");
        fund(&frank, 10_000);
        assert_noop!(
            join(&frank),
            pallet_cerulea_pos::Error::<Test>::TooManyValidators
        );
        assert_eq!(Balances::reserved_balance(&frank), 0);
    });
}

/// A cancel with no leave request on record is refused.
#[test]
fn a_cancel_with_no_request_is_refused() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Registry::cancel_leave_request(RuntimeOrigin::signed(account_key("Dave"))),
            pallet_cerulea_pos::Error::<Test>::ValidatorNotInSet
        );
    });
}

/// A cancel for an account outside the set clears the request alone.
#[test]
fn a_cancel_outside_the_set_clears_the_request_alone() {
    new_test_ext().execute_with(|| {
        System::set_block_number(2);
        let eve = account_key("Eve");
        crate::ValidatorLeaveRequests::<Test>::insert(&eve, 1);
        assert_ok!(Registry::cancel_leave_request(RuntimeOrigin::signed(
            eve.clone()
        )));
        assert_eq!(crate::ValidatorLeaveRequests::<Test>::get(&eve), None);
        assert!(!Registry::active_validators().contains(&eve));
        assert_eq!(pallet_cerulea_pos::Validators::<Test>::get(&eve), None);
    });
}

/// A leave from an account outside the set is refused.
#[test]
fn a_leave_from_outside_the_set_is_refused() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Registry::leave_validators(RuntimeOrigin::signed(account_key("Dave"))),
            pallet_cerulea_pos::Error::<Test>::ValidatorNotInSet
        );
    });
}

/// The epoch join request checks seat, stake and score in turn.
#[test]
fn an_epoch_join_request_checks_every_condition() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let alice = account_key("Alice");
        let dave = account_key("Dave");
        let request =
            |who: &AccountId| Registry::join_validator_set(RuntimeOrigin::signed(who.clone()));

        // An active validator asks for nothing new.
        seat_active(&[alice.clone()]);
        assert_noop!(request(&alice), Error::<Test>::NotAllowedInGovernanceMode);

        // An account short of the minimum stake is refused.
        assert_noop!(request(&dave), Error::<Test>::NotEnoughValidators);

        // An account with stake and no registry state is refused.
        pallet_cerulea_pos::Stake::<Test>::insert(&dave, MIN_STAKE);
        assert_noop!(request(&dave), Error::<Test>::ValidatorNotFound);

        // An account below the score floor is refused.
        crate::ValidatorStates::<Test>::insert(&dave, state(10, 0));
        assert_noop!(request(&dave), Error::<Test>::NotValidator);

        // An account meeting every condition waits for the epoch.
        crate::ValidatorStates::<Test>::insert(&dave, state(60, 0));
        assert_ok!(request(&dave));
        assert_eq!(
            crate::PendingValidatorActions::<Test>::get(&dave),
            Some(ValidatorAction::Join)
        );
        System::assert_last_event(
            Event::<Test>::ValidatorJoined {
                validator: dave.clone(),
                stake_amount: MIN_STAKE,
            }
            .into(),
        );

        // A second request while one waits is refused.
        assert_noop!(request(&dave), Error::<Test>::NotAllowedInGovernanceMode);
    });
}

/// The epoch leave request checks the active seat and the pending action.
#[test]
fn an_epoch_leave_request_checks_the_seat_and_the_pending_action() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let alice = account_key("Alice");
        let leave = || Registry::leave_validator_set(RuntimeOrigin::signed(alice.clone()));

        assert_noop!(leave(), Error::<Test>::NotValidator);

        seat_active(&[alice.clone()]);
        assert_ok!(leave());
        assert_eq!(
            crate::PendingValidatorActions::<Test>::get(&alice),
            Some(ValidatorAction::Leave)
        );
        System::assert_last_event(
            Event::<Test>::ValidatorLeft {
                validator: alice.clone(),
            }
            .into(),
        );

        assert_noop!(leave(), Error::<Test>::NotAllowedInGovernanceMode);
    });
}

// ----------------------------------------------------------------------------
// Licence admission
// ----------------------------------------------------------------------------

/// Only the root origin installs the licence key.
#[test]
fn the_root_origin_installs_the_licence_key() {
    new_test_ext().execute_with(|| {
        assert_eq!(crate::ValidatorLicenceKey::<Test>::get(), None);
        assert_noop!(
            Registry::set_validator_licence_key(
                RuntimeOrigin::signed(account_key("Alice")),
                control_plane().public().0,
                LicenceClass::Government
            ),
            DispatchError::BadOrigin
        );

        install_licence_key(LicenceClass::Government);
        assert_eq!(
            crate::ValidatorLicenceKey::<Test>::get(),
            Some(control_plane().public().0)
        );
        assert_eq!(
            crate::RequiredLicenceClass::<Test>::get(),
            LicenceClass::Government
        );
    });
}

/// A chain with no licence key admits through governance alone.
#[test]
fn a_chain_with_no_licence_key_skips_the_licence_check() {
    new_test_ext().execute_with(|| {
        assert_ok!(Registry::ensure_licensed(&account_key("Dave")));
    });
}

/// A licence the control plane signed admits the validator.
#[test]
fn a_valid_licence_admits_the_validator() {
    new_test_ext().execute_with(|| {
        System::set_block_number(100);
        let dave = account_key("Dave");
        install_licence_key(LicenceClass::PublicAccess);

        // Before the licence lands, the join is refused.
        assert_noop!(join(&dave), Error::<Test>::LicenceRequired);
        assert_noop!(
            Registry::ensure_licensed(&dave),
            Error::<Test>::LicenceRequired
        );

        let held = licence(&dave, LicenceClass::PublicAccess, 50, 500, 1);
        assert_ok!(present(
            &dave,
            held.clone(),
            signed_by(&control_plane(), &held)
        ));
        assert_eq!(crate::ValidatorLicences::<Test>::get(&dave), Some(held));
        assert_eq!(crate::LicenceNonces::<Test>::get(&dave), 1);
        assert_ok!(Registry::ensure_licensed(&dave));

        fund(&dave, 10_000);
        assert_ok!(join(&dave));
        assert!(Registry::validator_set().contains(&dave));

        // A newer attestation of a higher class replaces the first.
        let renewed = licence(&dave, LicenceClass::Government, 90, 900, 2);
        assert_ok!(present(
            &dave,
            renewed.clone(),
            signed_by(&control_plane(), &renewed)
        ));
        assert_eq!(crate::ValidatorLicences::<Test>::get(&dave), Some(renewed));
        assert_eq!(crate::LicenceNonces::<Test>::get(&dave), 2);
    });
}

/// A licence the control plane signed for another class is refused.
#[test]
fn a_licence_of_the_wrong_class_is_refused() {
    new_test_ext().execute_with(|| {
        System::set_block_number(100);
        let dave = account_key("Dave");
        install_licence_key(LicenceClass::PublicAccess);

        let held = licence(&dave, LicenceClass::Private, 50, 500, 1);
        assert_noop!(
            present(&dave, held.clone(), signed_by(&control_plane(), &held)),
            Error::<Test>::LicenceClassTooLow
        );
        assert_eq!(crate::ValidatorLicences::<Test>::get(&dave), None);
        fund(&dave, 10_000);
        assert_noop!(join(&dave), Error::<Test>::LicenceRequired);
    });
}

/// A raised class requirement reaches a licence already on chain.
///
/// The validator stored a private licence under a private requirement. Root
/// then raised the requirement to the public-access class. The join that
/// follows reads the raised requirement and refuses the stored licence.
#[test]
fn a_raised_class_requirement_refuses_a_stored_lower_licence() {
    new_test_ext().execute_with(|| {
        System::set_block_number(100);
        let dave = account_key("Dave");
        install_licence_key(LicenceClass::Private);

        let held = licence(&dave, LicenceClass::Private, 50, 500, 1);
        assert_ok!(present(
            &dave,
            held.clone(),
            signed_by(&control_plane(), &held)
        ));
        assert_ok!(Registry::ensure_licensed(&dave));

        install_licence_key(LicenceClass::PublicAccess);
        assert_noop!(
            Registry::ensure_licensed(&dave),
            Error::<Test>::LicenceClassTooLow
        );
        fund(&dave, 10_000);
        assert_noop!(join(&dave), Error::<Test>::LicenceClassTooLow);
        crate::ValidatorStates::<Test>::insert(&dave, state(60, 0));
        pallet_cerulea_pos::Stake::<Test>::insert(&dave, MIN_STAKE);
        assert_noop!(
            Registry::join_validator_set(RuntimeOrigin::signed(dave.clone())),
            Error::<Test>::LicenceClassTooLow
        );
    });
}

/// A licence signed by any key other than the control plane is refused.
#[test]
fn a_licence_from_the_wrong_signer_is_refused() {
    new_test_ext().execute_with(|| {
        System::set_block_number(100);
        let dave = account_key("Dave");
        install_licence_key(LicenceClass::PublicAccess);

        let held = licence(&dave, LicenceClass::PublicAccess, 50, 500, 1);
        let stranger = ed25519::Pair::from_seed(&[43u8; 32]);
        assert_noop!(
            present(&dave, held.clone(), signed_by(&stranger, &held)),
            Error::<Test>::LicenceSignatureRejected
        );

        // A signature over one attestation fails for an altered attestation.
        let signature = signed_by(&control_plane(), &held);
        let mut raised = held.clone();
        raised.class = LicenceClass::Government;
        assert_noop!(
            present(&dave, raised, signature),
            Error::<Test>::LicenceSignatureRejected
        );

        assert_eq!(crate::ValidatorLicences::<Test>::get(&dave), None);
        fund(&dave, 10_000);
        assert_noop!(join(&dave), Error::<Test>::LicenceRequired);
    });
}

/// An expired licence is refused at submission and at the join.
#[test]
fn an_expired_licence_is_refused() {
    new_test_ext().execute_with(|| {
        System::set_block_number(100);
        let dave = account_key("Dave");
        install_licence_key(LicenceClass::PublicAccess);

        // An attestation whose window closed at this block is refused.
        let lapsed = licence(&dave, LicenceClass::PublicAccess, 10, 100, 1);
        assert_noop!(
            present(&dave, lapsed.clone(), signed_by(&control_plane(), &lapsed)),
            Error::<Test>::LicenceLapsed
        );

        // A stored licence that lapses later blocks the join from that block.
        let short = licence(&dave, LicenceClass::PublicAccess, 50, 150, 1);
        assert_ok!(present(
            &dave,
            short.clone(),
            signed_by(&control_plane(), &short)
        ));
        fund(&dave, 10_000);
        System::set_block_number(150);
        assert_noop!(join(&dave), Error::<Test>::LicenceRequired);
        assert_noop!(
            Registry::ensure_licensed(&dave),
            Error::<Test>::LicenceRequired
        );
    });
}

/// An attestation whose window opens later is refused.
#[test]
fn a_licence_from_a_later_block_is_refused() {
    new_test_ext().execute_with(|| {
        System::set_block_number(100);
        let dave = account_key("Dave");
        install_licence_key(LicenceClass::Private);
        let early = licence(&dave, LicenceClass::Private, 200, 500, 1);
        assert_noop!(
            present(&dave, early.clone(), signed_by(&control_plane(), &early)),
            Error::<Test>::LicenceNotYetActive
        );
    });
}

/// A replayed attestation is refused.
#[test]
fn a_replayed_licence_is_refused() {
    new_test_ext().execute_with(|| {
        System::set_block_number(100);
        let dave = account_key("Dave");
        install_licence_key(LicenceClass::Private);

        let first = licence(&dave, LicenceClass::Private, 50, 500, 3);
        assert_ok!(present(
            &dave,
            first.clone(),
            signed_by(&control_plane(), &first)
        ));
        assert_noop!(
            present(&dave, first.clone(), signed_by(&control_plane(), &first)),
            Error::<Test>::LicenceReplayed
        );
        let older = licence(&dave, LicenceClass::Private, 50, 900, 2);
        assert_noop!(
            present(&dave, older.clone(), signed_by(&control_plane(), &older)),
            Error::<Test>::LicenceReplayed
        );
    });
}

/// An attestation names its holder and needs an installed key.
#[test]
fn a_licence_needs_its_holder_and_an_installed_key() {
    new_test_ext().execute_with(|| {
        System::set_block_number(100);
        let dave = account_key("Dave");
        let alice = account_key("Alice");

        let held = licence(&dave, LicenceClass::Private, 50, 500, 1);
        assert_noop!(
            present(&dave, held.clone(), signed_by(&control_plane(), &held)),
            Error::<Test>::LicenceKeyMissing
        );

        install_licence_key(LicenceClass::Private);
        assert_noop!(
            present(&alice, held.clone(), signed_by(&control_plane(), &held)),
            Error::<Test>::LicenceHolderMismatch
        );
    });
}

/// A genesis that names a licence key gates every later join.
#[test]
fn a_genesis_licence_key_gates_every_later_join() {
    let genesis = crate::GenesisConfig::<Test> {
        validator_licence_key: Some(control_plane().public().0),
        required_licence_class: 1,
        initial_validator_weights: vec![(account_key("Alice"), 1_000, 500_000_000)],
        initial_validator_keys: Vec::new(),
        strict_validation: true,
    };
    ext_from(genesis).execute_with(|| {
        assert_eq!(
            crate::ValidatorLicenceKey::<Test>::get(),
            Some(control_plane().public().0)
        );
        assert_eq!(
            crate::RequiredLicenceClass::<Test>::get(),
            LicenceClass::PublicAccess
        );
        assert_noop!(join(&account_key("Dave")), Error::<Test>::LicenceRequired);
    });
}

/// A genesis naming an unknown licence class stops at genesis.
#[test]
#[should_panic(expected = "unknown licence class")]
fn a_genesis_naming_an_unknown_licence_class_stops() {
    let genesis = crate::GenesisConfig::<Test> {
        validator_licence_key: None,
        required_licence_class: 3,
        initial_validator_weights: Vec::new(),
        initial_validator_keys: Vec::new(),
        strict_validation: false,
    };
    let _ = ext_from(genesis);
}

// ----------------------------------------------------------------------------
// Validator key records
// ----------------------------------------------------------------------------

/// Registration checks the seat, both possession proofs and the draw owner.
#[test]
fn key_registration_checks_the_seat_and_both_proofs() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let alice = account_key("Alice");
        let bob = account_key("Bob");
        let fresh = possession(&alice, 11, 11);

        // An account outside the set registers nothing.
        assert_noop!(register(&alice, &fresh), Error::<Test>::NotValidator);
        seat_in_set(&[alice.clone(), bob.clone()]);

        // A draw proof made for another account fails.
        let mut broken = possession(&alice, 11, 11);
        broken.draw_proof = possession(&bob, 11, 11).draw_proof;
        assert_noop!(
            register(&alice, &broken),
            Error::<Test>::DrawKeyPossessionRejected
        );

        // A seal proof made for another account fails.
        let mut broken = possession(&alice, 11, 11);
        broken.seal_proof = possession(&bob, 11, 11).seal_proof;
        assert_noop!(
            register(&alice, &broken),
            Error::<Test>::SealKeyPossessionRejected
        );

        // The draw key of Alice stays with Alice.
        let taken = possession(&bob, 1, 12);
        assert_noop!(register(&bob, &taken), Error::<Test>::DrawKeyAlreadyHeld);

        // A fresh pair waits in the pending record.
        assert_ok!(register(&alice, &fresh));
        let pending = crate::keys::ValidatorKeySet::new(fresh.draw, fresh.seal);
        assert_eq!(crate::PendingKeyRecords::<Test>::get(&alice), Some(pending));
        assert_eq!(
            crate::ValidatorKeyRecords::<Test>::get(&alice),
            Some(genesis_keys(1))
        );
        System::assert_last_event(
            Event::<Test>::ValidatorKeysRegistered {
                validator: alice.clone(),
                keys: pending,
            }
            .into(),
        );

        // A validator registers its own draw key again with a new seal key.
        let reseal = possession(&alice, 1, 13);
        assert_ok!(register(&alice, &reseal));
    });
}

/// The epoch boundary moves pending keys into service once.
#[test]
fn pending_keys_take_effect_at_the_epoch_boundary() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let alice = account_key("Alice");
        let bob = account_key("Bob");
        seat_in_set(&[alice.clone(), bob.clone()]);

        let fresh = possession(&alice, 11, 11);
        assert_ok!(register(&alice, &fresh));
        let new_alice = crate::keys::ValidatorKeySet::new(fresh.draw, fresh.seal);
        // Bob keeps the draw key and changes the seal key.
        let reseal = possession(&bob, 2, 22);
        assert_ok!(register(&bob, &reseal));
        let new_bob = crate::keys::ValidatorKeySet::new(reseal.draw, reseal.seal);

        // Inside the epoch the active record serves and the pending one waits.
        assert_eq!(
            Registry::key_set_for_epoch(&alice, 0),
            Some(genesis_keys(1))
        );
        assert_eq!(Registry::key_set_for_epoch(&alice, 1), Some(new_alice));
        assert_eq!(Registry::rotate_validator_keys_at_epoch_boundary(), 0);
        assert_eq!(
            crate::ValidatorKeyRecords::<Test>::get(&alice),
            Some(genesis_keys(1))
        );

        // The boundary moves both pending records.
        pallet_cerulea_pos::CurrentEpoch::<Test>::put(1);
        assert_eq!(Registry::rotate_validator_keys_at_epoch_boundary(), 2);
        assert_eq!(crate::KeyRotationEpoch::<Test>::get(), 1);
        assert_eq!(
            crate::ValidatorKeyRecords::<Test>::get(&alice),
            Some(new_alice)
        );
        assert_eq!(crate::ValidatorKeyRecords::<Test>::get(&bob), Some(new_bob));
        assert_eq!(crate::PendingKeyRecords::<Test>::get(&alice), None);
        assert_eq!(crate::PendingKeyRecords::<Test>::get(&bob), None);

        // Alice gave up the old draw key. Bob kept the same draw key.
        assert_eq!(crate::DrawKeyOwner::<Test>::get(genesis_keys(1).draw), None);
        assert_eq!(
            crate::DrawKeyOwner::<Test>::get(fresh.draw),
            Some(alice.clone())
        );
        assert_eq!(
            crate::DrawKeyOwner::<Test>::get(reseal.draw),
            Some(bob.clone())
        );
        System::assert_has_event(
            Event::<Test>::ValidatorKeysRotated {
                validator: alice.clone(),
                epoch: 1,
                keys: new_alice,
            }
            .into(),
        );

        // A second call in the same epoch moves nothing.
        assert_eq!(Registry::rotate_validator_keys_at_epoch_boundary(), 0);
    });
}

/// The block hook carries a rotation at the epoch boundary.
#[test]
fn the_block_hook_rotates_pending_keys() {
    new_test_ext().execute_with(|| {
        let charlie = account_key("Charlie");
        seat_in_set(&[charlie.clone()]);
        System::set_block_number(1);
        let fresh = possession(&charlie, 33, 33);
        assert_ok!(register(&charlie, &fresh));

        pallet_cerulea_pos::CurrentEpoch::<Test>::put(2);
        step(2);
        assert_eq!(
            crate::ValidatorKeyRecords::<Test>::get(&charlie),
            Some(crate::keys::ValidatorKeySet::new(fresh.draw, fresh.seal))
        );
        assert_eq!(crate::KeyRotationEpoch::<Test>::get(), 2);
    });
}

/// A chain holding no active key moves pending keys at the next call.
#[test]
fn a_chain_with_no_active_keys_moves_pending_keys_at_once() {
    let genesis = crate::GenesisConfig::<Test> {
        validator_licence_key: None,
        required_licence_class: 0,
        initial_validator_weights: Vec::new(),
        initial_validator_keys: Vec::new(),
        strict_validation: false,
    };
    ext_from(genesis).execute_with(|| {
        let dave = account_key("Dave");
        seat_in_set(&[dave.clone()]);
        let fresh = possession(&dave, 44, 44);
        assert_ok!(register(&dave, &fresh));
        let pending = crate::keys::ValidatorKeySet::new(fresh.draw, fresh.seal);

        // The pending record serves the current epoch while nothing is active.
        assert_eq!(Registry::key_set_for_epoch(&dave, 0), Some(pending));
        assert_eq!(Registry::rotate_validator_keys_at_epoch_boundary(), 1);
        assert_eq!(
            crate::ValidatorKeyRecords::<Test>::get(&dave),
            Some(pending)
        );
        assert_eq!(crate::DrawKeyOwner::<Test>::get(fresh.draw), Some(dave));
    });
}

/// An account with no key record reads no key set.
#[test]
fn an_account_with_no_keys_reads_no_key_set() {
    new_test_ext().execute_with(|| {
        let alice = account_key("Alice");
        let dave = account_key("Dave");
        assert_eq!(Registry::key_set_for_epoch(&dave, 0), None);
        assert_eq!(Registry::key_set_for_epoch(&dave, 5), None);
        // A later epoch with no pending record reads the active record.
        assert_eq!(
            Registry::key_set_for_epoch(&alice, 5),
            Some(genesis_keys(1))
        );
    });
}

// ----------------------------------------------------------------------------
// Epoch draws and the slot committee
// ----------------------------------------------------------------------------

/// A draw made with the registered key for the stated epoch is stored.
#[test]
fn a_valid_epoch_draw_is_stored() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let alice = account_key("Alice");
        let (proof, output) = epoch_draw(1, 0);
        assert_ok!(Registry::publish_epoch_draw(
            RuntimeOrigin::signed(alice.clone()),
            0,
            proof
        ));
        assert_eq!(
            crate::EpochDraws::<Test>::get(0, &alice),
            Some(crate::keys::PublishedDraw {
                epoch: 0,
                output,
                proof
            })
        );
        System::assert_last_event(
            Event::<Test>::EpochDrawPublished {
                validator: alice.clone(),
                epoch: 0,
            }
            .into(),
        );

        // One draw per validator per epoch.
        assert_noop!(
            Registry::publish_epoch_draw(RuntimeOrigin::signed(alice), 0, proof),
            Error::<Test>::EpochDrawAlreadyHeld
        );
    });
}

/// Every faulty draw is refused with its own error.
#[test]
fn faulty_epoch_draws_are_refused() {
    new_test_ext().execute_with(|| {
        let alice = account_key("Alice");
        let dave = account_key("Dave");
        let publish = |who: &AccountId, epoch: u32, proof: crate::keys::DrawProofBytes| {
            Registry::publish_epoch_draw(RuntimeOrigin::signed(who.clone()), epoch, proof)
        };

        // An account with no keys publishes nothing.
        let (proof, _) = epoch_draw(4, 1);
        assert_noop!(publish(&dave, 1, proof), Error::<Test>::ValidatorKeysAbsent);

        // Bytes that fail to decode are refused.
        assert_noop!(
            publish(&alice, 1, [0xffu8; crate::keys::DRAW_PROOF_BYTES]),
            Error::<Test>::EpochDrawRejected
        );

        // A draw for another epoch fails the proof.
        let (other_epoch, _) = epoch_draw(1, 7);
        assert_noop!(
            publish(&alice, 1, other_epoch),
            Error::<Test>::EpochDrawRejected
        );

        // A draw made with another key fails the proof.
        let (other_key, _) = epoch_draw(2, 1);
        assert_noop!(
            publish(&alice, 1, other_key),
            Error::<Test>::EpochDrawRejected
        );

        // A closed epoch takes no draw.
        pallet_cerulea_pos::CurrentEpoch::<Test>::put(2);
        let (late, _) = epoch_draw(1, 1);
        assert_noop!(publish(&alice, 1, late), Error::<Test>::EpochDrawTooLate);
    });
}

/// The active key list names each active validator holding keys.
#[test]
fn the_active_key_list_names_each_keyed_active_validator() {
    new_test_ext().execute_with(|| {
        let alice = account_key("Alice");
        let bob = account_key("Bob");
        let dave = account_key("Dave");
        assert!(Registry::active_validator_keys().is_empty());

        seat_active(&[bob.clone(), dave.clone(), alice.clone()]);
        assert_eq!(
            Registry::active_validator_keys(),
            vec![(bob, genesis_keys(2)), (alice, genesis_keys(1))]
        );
    });
}

/// The slot committee ranks every keyed active validator.
#[test]
fn the_slot_committee_ranks_every_keyed_active_validator() {
    new_test_ext().execute_with(|| {
        let alice = account_key("Alice");
        let bob = account_key("Bob");
        let charlie = account_key("Charlie");
        let dave = account_key("Dave");
        seat_active(&[alice.clone(), bob.clone(), dave.clone(), charlie.clone()]);

        let (proof, output) = epoch_draw(1, 0);
        assert_ok!(Registry::publish_epoch_draw(
            RuntimeOrigin::signed(alice.clone()),
            0,
            proof
        ));

        let slot = 9u64;
        let parent = [5u8; 32];
        let standing = 400_000u32;
        let committee = Registry::slot_committee(slot, parent, |_| standing);

        // Dave holds no keys, so three validators form the committee.
        assert_eq!(committee.len(), 3);
        assert!(committee.iter().all(|seat| seat.validator != dave));

        // The ranking matches the shared Authoring Engine rule.
        let seed = cerulea_primitives::derive_slot_seed(&parent, slot);
        let candidates: Vec<cerulea_primitives::SeatInput> = [
            (genesis_keys(1), output),
            (
                genesis_keys(2),
                crate::keys::starting_epoch_output(&genesis_keys(2).draw),
            ),
            (
                genesis_keys(3),
                crate::keys::starting_epoch_output(&genesis_keys(3).draw),
            ),
        ]
        .iter()
        .map(|(keys, epoch_output)| cerulea_primitives::SeatInput {
            draw_key: keys.draw,
            epoch_output: *epoch_output,
            standing,
        })
        .collect();
        let expected = cerulea_primitives::rank_seats(&candidates, &seed);
        assert_eq!(expected.len(), committee.len());
        for (seat, rule) in committee.iter().zip(expected.iter()) {
            assert_eq!(seat.rank, rule.rank);
            assert_eq!(seat.seat_value, rule.seat_value);
            assert_eq!(seat.keys.draw, rule.draw_key);
            assert_eq!(
                crate::DrawKeyOwner::<Test>::get(seat.keys.draw),
                Some(seat.validator.clone())
            );
        }

        // Alice carries the draw she published. The others carry none yet.
        for seat in &committee {
            if seat.validator == alice {
                assert_eq!(
                    seat.draw,
                    Some(crate::keys::PublishedDraw {
                        epoch: 0,
                        output,
                        proof
                    })
                );
            } else {
                assert_eq!(seat.draw, None);
            }
        }

        // Higher standing lowers the seat value of the same draw.
        let seat_of = |committee: &[crate::keys::SlotCommitteeSeat<AccountId>]| {
            committee
                .iter()
                .find(|seat| seat.validator == bob)
                .map(|seat| seat.seat_value)
                .expect("Bob holds a seat")
        };
        let low = seat_of(&Registry::slot_committee(slot, parent, |_| 0));
        let high = seat_of(&Registry::slot_committee(slot, parent, |_| 1_000_000));
        assert!(high < low);
    });
}

// ----------------------------------------------------------------------------
// The block hook
// ----------------------------------------------------------------------------

/// The hook clears a removal record once the cooldown runs out.
#[test]
fn the_hook_clears_expired_removal_records() {
    new_test_ext().execute_with(|| {
        let alice = account_key("Alice");
        crate::RecentlyRemovedValidators::<Test>::insert(&alice, 5);

        step(4 + COOLDOWN as u64);
        assert_eq!(
            crate::RecentlyRemovedValidators::<Test>::get(&alice),
            Some(5)
        );

        step(5 + COOLDOWN as u64);
        assert_eq!(crate::RecentlyRemovedValidators::<Test>::get(&alice), None);
        System::assert_has_event(Event::<Test>::CooldownExpired { validator: alice }.into());
    });
}

/// The hook ejects low scores and long absences at the check interval.
#[test]
fn the_hook_ejects_underperforming_validators_at_the_interval() {
    new_test_ext().execute_with(|| {
        let low = account_key("Low");
        let stale = account_key("Stale");
        let steady = account_key("Steady");
        let bare = account_key("Bare");
        seat_active(&[low.clone(), stale.clone(), steady.clone(), bare.clone()]);
        pallet_cerulea_pos::CurrentEpoch::<Test>::put(6);
        crate::ValidatorStates::<Test>::insert(&low, state(10, 6));
        crate::ValidatorStates::<Test>::insert(&stale, state(60, 0));
        crate::ValidatorStates::<Test>::insert(&steady, state(60, 6));

        // Off the interval the hook leaves every seat alone.
        step(49);
        assert_eq!(Registry::active_validators().len(), 4);

        step(50);
        assert_eq!(
            Registry::active_validators().into_inner(),
            vec![steady.clone(), bare.clone()]
        );
        for validator in [low, stale] {
            System::assert_has_event(
                Event::<Test>::ValidatorStatusChanged {
                    validator: validator.clone(),
                    old_status: ValidatorStatus::Active,
                    new_status: ValidatorStatus::Ejected,
                    block_number: 50,
                }
                .into(),
            );
            System::assert_has_event(
                Event::<Test>::ValidatorEjected {
                    validator,
                    reason: EjectionReason::ScoreBelowThreshold,
                }
                .into(),
            );
        }
    });
}

/// An ejection of an inactive account records the inactive status.
#[test]
fn an_ejection_outside_the_active_set_records_the_inactive_status() {
    new_test_ext().execute_with(|| {
        System::set_block_number(3);
        let dave = account_key("Dave");
        assert_ok!(Registry::eject_validator(
            &dave,
            EjectionReason::ManualEjection
        ));
        System::assert_has_event(
            Event::<Test>::ValidatorStatusChanged {
                validator: dave.clone(),
                old_status: ValidatorStatus::Inactive,
                new_status: ValidatorStatus::Ejected,
                block_number: 3,
            }
            .into(),
        );

        // The registry trait reaches the same ejection.
        let alice = account_key("Alice");
        seat_active(&[alice.clone()]);
        assert_ok!(provider!(
            eject_validator,
            &alice,
            EjectionReason::RepeatedMisbehavior
        ));
        assert!(Registry::active_validators().is_empty());
        System::assert_last_event(
            Event::<Test>::ValidatorEjected {
                validator: alice,
                reason: EjectionReason::RepeatedMisbehavior,
            }
            .into(),
        );
    });
}

// ----------------------------------------------------------------------------
// Round timeout branches
// ----------------------------------------------------------------------------

/// The round stays open before either timeout condition holds.
#[test]
fn the_round_stays_open_before_the_timeout() {
    new_test_ext().execute_with(|| {
        let timeout = crate::DEFAULT_ROUND_TIMEOUT_BLOCKS as u64;
        // The round opened at block zero, so the first condition fails here.
        assert!(!Registry::close_round_on_timeout(timeout - 1));
        // The checkpoint at block ten has waited less than the timeout here.
        assert!(!Registry::close_round_on_timeout(timeout + 5));
        assert_eq!(crate::CurrentRound::<Test>::get(), 0);
        assert_eq!(crate::RoundOutcomes::<Test>::get(0), None);
    });
}

/// A recorded tally at the threshold commits at the timeout.
#[test]
fn a_tally_at_the_threshold_commits_at_the_timeout() {
    new_test_ext().execute_with(|| {
        System::set_block_number(30);
        let hash = H256::repeat_byte(9);
        let threshold = Registry::threshold_weight();
        crate::VoteTallies::<Test>::insert(hash, threshold);
        crate::VoteRecords::<Test>::insert(
            0,
            account_key("Alice"),
            signed_vote("Alice", 0, 0, 10, hash),
        );

        assert!(Registry::close_round_on_timeout(30));
        assert_eq!(crate::FinalizedBlockNumber::<Test>::get(), 10);
        assert_eq!(crate::FinalizedBlockHash::<Test>::get(), Some(hash));
        assert_eq!(
            crate::RoundOutcomes::<Test>::get(0),
            Some(crate::RoundOutcome::Committed {
                block_hash: hash,
                weight: threshold
            })
        );
        assert_eq!(crate::CurrentRound::<Test>::get(), 1);
        assert_eq!(crate::RoundOpenedAt::<Test>::get(), 30);
        assert_eq!(crate::VoteTallies::<Test>::get(hash), 0);
        System::assert_has_event(
            Event::<Test>::BlockFinalized {
                block_number: 10,
                block_hash: hash,
                round: 0,
                weight: threshold,
            }
            .into(),
        );
    });
}

/// A tally at the threshold for a finalized height closes as a timeout.
#[test]
fn a_tally_for_a_finalized_height_closes_as_a_timeout() {
    new_test_ext().execute_with(|| {
        System::set_block_number(40);
        let hash = H256::repeat_byte(8);
        let threshold = Registry::threshold_weight();
        crate::FinalizedBlockNumber::<Test>::put(10);
        crate::VoteTallies::<Test>::insert(hash, threshold);
        crate::VoteRecords::<Test>::insert(
            0,
            account_key("Alice"),
            signed_vote("Alice", 0, 0, 10, hash),
        );

        assert!(Registry::close_round_on_timeout(40));
        assert_eq!(crate::FinalizedBlockHash::<Test>::get(), None);
        assert_eq!(
            crate::RoundOutcomes::<Test>::get(0),
            Some(crate::RoundOutcome::TimedOut {
                highest_weight: threshold,
                threshold
            })
        );
        System::assert_has_event(
            Event::<Test>::RoundTimedOut {
                round: 0,
                next_round: 1,
                highest_weight: threshold,
                threshold,
            }
            .into(),
        );
    });
}

/// A tally at the threshold with no matching vote record closes as a timeout.
#[test]
fn a_tally_with_no_vote_record_closes_as_a_timeout() {
    new_test_ext().execute_with(|| {
        System::set_block_number(30);
        let hash = H256::repeat_byte(7);
        let threshold = Registry::threshold_weight();
        crate::VoteTallies::<Test>::insert(hash, threshold);

        assert!(Registry::close_round_on_timeout(30));
        assert_eq!(crate::FinalizedBlockNumber::<Test>::get(), 0);
        assert!(matches!(
            crate::RoundOutcomes::<Test>::get(0),
            Some(crate::RoundOutcome::TimedOut { .. })
        ));
        assert_eq!(crate::CurrentRound::<Test>::get(), 1);
    });
}

/// Opening a round past the retention window clears the oldest round.
#[test]
fn opening_a_round_past_the_retention_window_clears_the_oldest_round() {
    new_test_ext().execute_with(|| {
        System::set_block_number(30);
        let alice = account_key("Alice");
        let hash = H256::repeat_byte(6);
        let outcome = crate::RoundOutcome::TimedOut {
            highest_weight: 1,
            threshold: 2,
        };
        crate::VoteRecords::<Test>::insert(0, &alice, signed_vote("Alice", 0, 0, 10, hash));
        crate::VoteRecords::<Test>::insert(1, &alice, signed_vote("Alice", 0, 1, 10, hash));
        crate::RoundOutcomes::<Test>::insert(0, outcome.clone());
        crate::RoundOutcomes::<Test>::insert(1, outcome.clone());
        crate::CurrentRound::<Test>::put(20);

        assert!(Registry::close_round_on_timeout(30));
        assert_eq!(crate::CurrentRound::<Test>::get(), 21);
        assert!(!crate::VoteRecords::<Test>::contains_key(0, &alice));
        assert!(crate::VoteRecords::<Test>::contains_key(1, &alice));
        assert_eq!(crate::RoundOutcomes::<Test>::get(0), None);
        assert_eq!(crate::RoundOutcomes::<Test>::get(1), Some(outcome));
        assert!(matches!(
            crate::RoundOutcomes::<Test>::get(20),
            Some(crate::RoundOutcome::TimedOut { .. })
        ));
    });
}

// ----------------------------------------------------------------------------
// Vote and justification checks
// ----------------------------------------------------------------------------

/// Every faulty vote is refused with its own error.
#[test]
fn faulty_votes_are_refused() {
    new_test_ext().execute_with(|| {
        let hash = H256::repeat_byte(1);
        let submit = |vote: Vote| {
            Registry::submit_dvf_vote(RuntimeOrigin::signed(vote.validator_account.clone()), vote)
        };

        // A vote changed after signing fails its signature.
        let mut altered = signed_vote("Alice", 0, 0, 10, hash);
        altered.block_hash = H256::repeat_byte(2);
        assert_noop!(submit(altered), Error::<Test>::InvalidSignature);

        // An account with no frozen weight casts no vote.
        assert_noop!(
            submit(signed_vote("Dave", 0, 0, 10, hash)),
            Error::<Test>::InvalidValidator
        );

        // A vote at or below the finalized head is refused.
        crate::FinalizedBlockNumber::<Test>::put(20);
        assert_noop!(
            submit(signed_vote("Alice", 0, 0, 20, hash)),
            Error::<Test>::BlockAlreadyFinalized
        );

        // The vote call takes a signed origin.
        assert_noop!(
            Registry::submit_dvf_vote(RuntimeOrigin::none(), signed_vote("Alice", 0, 0, 30, hash)),
            DispatchError::BadOrigin
        );
    });
}

/// Every faulty justification vote is refused with its own error.
#[test]
fn faulty_justification_votes_are_refused() {
    new_test_ext().execute_with(|| {
        let hash = H256::repeat_byte(3);
        let justify = |votes: Vec<Vote>| {
            Registry::submit_justification(
                RuntimeOrigin::none(),
                crate::DvfJustification {
                    round_number: 0,
                    block_hash: hash,
                    votes,
                },
            )
        };

        let mut altered = signed_vote("Alice", 0, 0, 10, hash);
        altered.epoch_id = 2;
        assert_noop!(
            justify(vec![altered, signed_vote("Bob", 0, 0, 10, hash)]),
            Error::<Test>::InvalidSignature
        );

        assert_noop!(
            justify(vec![
                signed_vote("Alice", 0, 0, 10, hash),
                signed_vote("Bob", 1, 0, 10, hash)
            ]),
            Error::<Test>::ValidatorSetIdMismatch
        );

        assert_noop!(
            justify(vec![signed_vote("Alice", 0, 1, 10, hash)]),
            Error::<Test>::RoundMismatch
        );

        assert_noop!(
            justify(vec![signed_vote("Alice", 0, 0, 10, H256::repeat_byte(4))]),
            Error::<Test>::BlockHashMismatch
        );

        assert_noop!(
            justify(vec![
                signed_vote("Alice", 0, 0, 10, hash),
                signed_vote("Dave", 0, 0, 10, hash)
            ]),
            Error::<Test>::InvalidValidator
        );
    });
}

/// The genesis validators reach the threshold and commit through a record.
#[test]
fn the_genesis_validators_commit_through_a_justification() {
    new_test_ext().execute_with(|| {
        System::set_block_number(12);
        let hash = H256::repeat_byte(5);
        let votes = ["Alice", "Bob", "Charlie"]
            .iter()
            .map(|name| signed_vote(name, 0, 0, 10, hash))
            .collect();
        assert_ok!(Registry::submit_justification(
            RuntimeOrigin::none(),
            crate::DvfJustification {
                round_number: 0,
                block_hash: hash,
                votes,
            }
        ));
        assert_eq!(crate::FinalizedBlockNumber::<Test>::get(), 10);
        for name in ["Alice", "Bob", "Charlie"] {
            assert!(crate::VoteRecords::<Test>::contains_key(
                0,
                account_key(name)
            ));
        }
        assert!(Registry::is_block_finalized(10));
        assert!(!Registry::is_block_finalized(11));
    });
}

// ----------------------------------------------------------------------------
// Unsigned validation
// ----------------------------------------------------------------------------

/// The pool validation reads the justification height and the head.
#[test]
fn unsigned_justifications_are_validated_against_the_head() {
    new_test_ext().execute_with(|| {
        let hash = H256::repeat_byte(5);
        let validate = |votes: Vec<Vote>| {
            let call = crate::Call::<Test>::submit_justification {
                justification: crate::DvfJustification {
                    round_number: 0,
                    block_hash: hash,
                    votes,
                },
            };
            <Registry as ValidateUnsigned>::validate_unsigned(TransactionSource::External, &call)
        };

        assert_eq!(
            validate(Vec::new()),
            Err(TransactionValidityError::Invalid(
                InvalidTransaction::Custom(1)
            ))
        );

        let valid = validate(vec![signed_vote("Alice", 0, 0, 20, hash)])
            .expect("a record above the head passes the pool");
        assert_eq!(valid.priority, 20);
        assert_eq!(valid.longevity, 5);
        assert!(valid.propagate);
        let mut tag = b"dvf_justification_".to_vec();
        hash.encode_to(&mut tag);
        assert_eq!(valid.provides, vec![("Registry", tag).encode()]);

        crate::FinalizedBlockNumber::<Test>::put(20);
        assert_eq!(
            validate(vec![signed_vote("Alice", 0, 0, 20, hash)]),
            Err(TransactionValidityError::Invalid(InvalidTransaction::Stale))
        );
    });
}

/// The pool validation checks equivocation proof and the penalty record.
#[test]
fn unsigned_equivocation_reports_are_validated() {
    new_test_ext().execute_with(|| {
        let dave = account_key("Dave");
        let first = signed_vote("Dave", 0, 3, 30, H256::repeat_byte(1));
        let second = signed_vote("Dave", 0, 3, 30, H256::repeat_byte(2));
        let validate = |first: &Vote, second: &Vote| {
            let call = crate::Call::<Test>::report_vote_equivocation {
                first: first.clone(),
                second: second.clone(),
            };
            <Registry as ValidateUnsigned>::validate_unsigned(TransactionSource::External, &call)
        };

        assert_eq!(
            validate(&first, &first),
            Err(TransactionValidityError::Invalid(
                InvalidTransaction::BadProof
            ))
        );

        let valid = validate(&first, &second).expect("a proven pair passes the pool");
        assert_eq!(valid.priority, u64::MAX / 2);
        assert_eq!(valid.longevity, 64);
        let mut tag = b"vote_equivocation_".to_vec();
        3u32.encode_to(&mut tag);
        dave.encode_to(&mut tag);
        assert_eq!(valid.provides, vec![("Registry", tag).encode()]);

        // The report travels unsigned. A signed origin is refused.
        assert_noop!(
            Registry::report_vote_equivocation(
                RuntimeOrigin::signed(dave.clone()),
                first.clone(),
                second.clone()
            ),
            DispatchError::BadOrigin
        );

        assert_ok!(Registry::report_vote_equivocation(
            RuntimeOrigin::none(),
            first.clone(),
            second.clone()
        ));
        assert_eq!(
            validate(&first, &second),
            Err(TransactionValidityError::Invalid(InvalidTransaction::Stale))
        );
    });
}

/// The pool refuses every other call from the unsigned path.
#[test]
fn unsigned_validation_refuses_other_calls() {
    new_test_ext().execute_with(|| {
        let call = crate::Call::<Test>::set_round_timeout { blocks: 5 };
        assert_eq!(
            <Registry as ValidateUnsigned>::validate_unsigned(TransactionSource::External, &call),
            Err(TransactionValidityError::Invalid(InvalidTransaction::Call))
        );
    });
}

// ----------------------------------------------------------------------------
// Epoch weight freeze edges
// ----------------------------------------------------------------------------

/// A set with no recorded standing splits the weight evenly.
#[test]
fn a_set_with_no_standing_splits_the_weight_evenly() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let accounts = [
            account_key("Alice"),
            account_key("Bob"),
            account_key("Dave"),
        ];
        let validators: Vec<(AccountId, u128, u128)> =
            accounts.iter().map(|who| (who.clone(), 1_000, 0)).collect();
        Registry::freeze_epoch_weights(4, &validators);

        let share = crate::VOTE_WEIGHT_SCALE / 3;
        for who in &accounts {
            assert_eq!(crate::EpochVotingWeight::<Test>::get(who), Some(share));
        }
        assert_eq!(crate::TotalVotingWeight::<Test>::get(), share * 3);
        assert_eq!(
            crate::EpochVotingWeight::<Test>::get(account_key("Charlie")),
            None
        );
        System::assert_has_event(
            Event::<Test>::ValidatorSetChanged {
                old_id: 0,
                new_id: 1,
            }
            .into(),
        );
        assert_eq!(crate::ValidatorSetIdChangedAt::<Test>::get(), Some(1));
    });
}

/// An empty set freezes a zero total.
#[test]
fn an_empty_set_freezes_a_zero_total() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        <Registry as WeightFreezer<AccountId>>::freeze_epoch_weights(5, &[]);
        assert_eq!(crate::TotalVotingWeight::<Test>::get(), 0);
        assert_eq!(
            crate::EpochVotingWeight::<Test>::get(account_key("Alice")),
            None
        );
        assert!(crate::PreviousValidatorSet::<Test>::get().is_empty());
        assert_eq!(crate::ValidatorSetId::<Test>::get(), 1);
        System::assert_last_event(
            Event::<Test>::WeightsFrozen {
                epoch: 5,
                total_weight: 0,
            }
            .into(),
        );
        // A threshold over a zero total reads zero.
        assert_eq!(Registry::threshold_weight(), 0);
    });
}

/// A set above the bound keeps the bound in the change record.
#[test]
fn a_set_above_the_bound_keeps_the_bound_in_the_change_record() {
    new_test_ext().execute_with(|| {
        let validators: Vec<(AccountId, u128, u128)> = (0..=SEAT_BOUND)
            .map(|index| (filler(index), 1_000, 1))
            .collect();
        Registry::freeze_epoch_weights(6, &validators);
        assert_eq!(
            crate::PreviousValidatorSet::<Test>::get().len(),
            SEAT_BOUND as usize
        );
        assert_eq!(
            crate::PreviousValidatorSet::<Test>::get().first(),
            Some(&filler(0))
        );
        let share = crate::VOTE_WEIGHT_SCALE / (SEAT_BOUND as u128 + 1);
        assert_eq!(
            crate::EpochVotingWeight::<Test>::get(filler(SEAT_BOUND)),
            Some(share)
        );
    });
}

// ----------------------------------------------------------------------------
// The stake handler the staking pallet calls
// ----------------------------------------------------------------------------

/// A handler join seats the validator and records a starting state.
#[test]
fn a_handler_join_seats_the_validator_once() {
    new_test_ext().execute_with(|| {
        System::set_block_number(7);
        pallet_cerulea_pos::CurrentEpoch::<Test>::put(3);
        let dave = account_key("Dave");

        assert_ok!(handler!(on_joined, &dave, 2_500));
        assert_eq!(Registry::validator_set().into_inner(), vec![dave.clone()]);
        let held = crate::ValidatorStates::<Test>::get(&dave).expect("the join records a state");
        assert_eq!(held.current.epoch, 3);
        assert_eq!(held.last_active_epoch, 3);
        assert_eq!(held.current.stake_score, 2_500);
        assert_eq!(held.current.final_score, 50);
        assert_eq!(held.last_active_block, 7);
        assert_eq!(held.trust_score, 5_000);

        // A second call keeps one seat and the first state.
        assert_ok!(handler!(on_joined, &dave, 9_000));
        assert_eq!(Registry::validator_set().len(), 1);
        assert_eq!(
            crate::ValidatorStates::<Test>::get(&dave).map(|held| held.current.stake_score),
            Some(2_500)
        );
        assert_eq!(handler!(get_validator_score, &dave), 50);
        assert_eq!(handler!(get_validator_score, &account_key("Eve")), 0);
    });
}

/// A handler join into a full set is refused.
#[test]
fn a_handler_join_into_a_full_set_is_refused() {
    new_test_ext().execute_with(|| {
        let fillers: Vec<AccountId> = (0..SEAT_BOUND).map(filler).collect();
        seat_in_set(&fillers);
        assert_noop!(
            handler!(on_joined, &account_key("Dave"), 1_000),
            pallet_cerulea_pos::Error::<Test>::TooManyValidators
        );
    });
}

/// The stake events move the stake score and the trust score.
#[test]
fn stake_events_move_the_recorded_scores() {
    new_test_ext().execute_with(|| {
        let dave = account_key("Dave");
        crate::ValidatorStates::<Test>::insert(&dave, state(60, 0));

        assert_ok!(handler!(on_stake_increased, &dave, 500));
        assert_ok!(handler!(on_stake_decreased, &dave, 200));
        assert_ok!(handler!(on_rewarded, &dave, 100, 40));
        assert_ok!(handler!(on_slashed, &dave, 50, 15));
        let held = crate::ValidatorStates::<Test>::get(&dave).expect("the state stays");
        assert_eq!(held.current.stake_score, 1_000 + 500 - 200 + 100 - 50);
        assert_eq!(held.trust_score, 25);

        // A slash past zero settles at zero.
        assert_ok!(handler!(on_slashed, &dave, 1_000_000, 1_000));
        let held = crate::ValidatorStates::<Test>::get(&dave).expect("the state stays");
        assert_eq!(held.current.stake_score, 0);
        assert_eq!(held.trust_score, 0);

        // An account with no state gains no state from a stake event.
        let eve = account_key("Eve");
        assert_ok!(handler!(on_stake_increased, &eve, 500));
        assert_ok!(handler!(on_rewarded, &eve, 500, 5));
        assert!(crate::ValidatorStates::<Test>::get(&eve).is_none());
    });
}

/// The leave events remove the seat and record the removal.
#[test]
fn handler_leave_events_remove_the_seat() {
    new_test_ext().execute_with(|| {
        System::set_block_number(9);
        let alice = account_key("Alice");
        let bob = account_key("Bob");
        seat_in_set(&[alice.clone(), bob.clone()]);
        seat_active(&[alice.clone(), bob.clone()]);
        crate::ValidatorJoinTime::<Test>::insert(&alice, 1);

        assert_ok!(handler!(on_leave_requested, &alice));
        assert_eq!(crate::ValidatorLeaveRequests::<Test>::get(&alice), Some(9));
        assert_eq!(handler!(get_active_validators), vec![bob.clone()]);

        System::set_block_number(11);
        assert_ok!(handler!(on_left, &alice));
        assert_eq!(Registry::validator_set().into_inner(), vec![bob.clone()]);
        assert_eq!(crate::ValidatorJoinTime::<Test>::get(&alice), None);
        assert_eq!(crate::ValidatorLeaveRequests::<Test>::get(&alice), None);
        assert_eq!(
            crate::RecentlyRemovedValidators::<Test>::get(&alice),
            Some(11)
        );

        // A seated validator leaving both lists leaves both lists.
        assert_ok!(handler!(on_left, &bob));
        assert!(Registry::validator_set().is_empty());
        assert!(Registry::active_validators().is_empty());
    });
}

// ----------------------------------------------------------------------------
// The registry trait the consensus pallet reads
// ----------------------------------------------------------------------------

/// The status reads follow the seat records in order.
#[test]
fn the_status_read_follows_the_seat_records() {
    new_test_ext().execute_with(|| {
        let active = account_key("Alice");
        let seated = account_key("Bob");
        let leaving = account_key("Charlie");
        let removed = account_key("Dave");
        let unknown = account_key("Eve");
        seat_in_set(&[active.clone(), seated.clone()]);
        provider!(update_active_validators, vec![active.clone()]);
        provider!(set_validator_leave_request, &leaving, 4);
        provider!(set_recently_removed_validator, &removed, 4);

        assert_eq!(
            provider!(get_validator_status, &active),
            Some(ValidatorStatus::Active)
        );
        assert_eq!(
            provider!(get_validator_status, &seated),
            Some(ValidatorStatus::Inactive)
        );
        assert_eq!(
            provider!(get_validator_status, &leaving),
            Some(ValidatorStatus::Leaving)
        );
        assert_eq!(
            provider!(get_validator_status, &removed),
            Some(ValidatorStatus::Ejected)
        );
        assert_eq!(provider!(get_validator_status, &unknown), None);

        assert!(provider!(is_validator_active, &active));
        assert!(!provider!(is_validator_active, &seated));
        assert_eq!(provider!(get_active_validators), vec![active.clone()]);
        assert_eq!(
            provider!(get_validator_set),
            vec![active.clone(), seated.clone()]
        );

        provider!(update_validator_set, vec![seated.clone()]);
        assert_eq!(Registry::validator_set().into_inner(), vec![seated]);
    });
}

/// The cooldown read reports the blocks left and whether the wait ended.
#[test]
fn the_cooldown_read_reports_the_blocks_left() {
    new_test_ext().execute_with(|| {
        let leaving = account_key("Alice");
        let removed = account_key("Bob");
        System::set_block_number(110);
        provider!(set_validator_leave_request, &leaving, 10);
        provider!(set_recently_removed_validator, &removed, 100);

        assert_eq!(
            provider!(get_validator_detailed_cooldown_status, &leaving),
            Some((COOLDOWN - 100, false))
        );
        assert_eq!(provider!(get_validator_leave_request, &leaving), Some(10));
        assert_eq!(
            provider!(get_validator_detailed_cooldown_status, &removed),
            Some((COOLDOWN - 10, false))
        );
        assert_eq!(
            provider!(get_recently_removed_validator, &removed),
            Some(100)
        );
        assert_eq!(
            provider!(get_validator_detailed_cooldown_status, &account_key("Eve")),
            None
        );

        System::set_block_number(1_100);
        assert_eq!(
            provider!(get_validator_detailed_cooldown_status, &removed),
            Some((0, true))
        );

        provider!(remove_validator_leave_request, &leaving);
        provider!(remove_recently_removed_validator, &removed);
        assert_eq!(provider!(get_validator_leave_request, &leaving), None);
        assert_eq!(provider!(get_recently_removed_validator, &removed), None);
    });
}

/// The profile read joins the registry state, the stake and the seat.
#[test]
fn the_profile_read_joins_state_stake_and_seat() {
    new_test_ext().execute_with(|| {
        let dave = account_key("Dave");
        assert!(provider!(get_validator_profile, &dave).is_none());

        let mut held = state(60, 2);
        held.trust_score = 900;
        held.inference_count = 3;
        held.last_active_block = 44;
        held.name = Some(BoundedVec::truncate_from(b"Steady".to_vec()));
        crate::ValidatorStates::<Test>::insert(&dave, held);
        pallet_cerulea_pos::Stake::<Test>::insert(&dave, 2_000);

        let profile = provider!(get_validator_profile, &dave).expect("the state yields a profile");
        assert_eq!(profile.stake, 2_000);
        assert_eq!(profile.final_score, 60);
        assert_eq!(profile.trust_score, 900);
        assert_eq!(profile.inference_count, 3);
        assert_eq!(profile.last_active_block, 44);
        assert_eq!(profile.status, ValidatorStatus::Inactive);
        assert_eq!(
            profile.name.map(|name| name.into_inner()),
            Some(b"Steady".to_vec())
        );

        seat_active(&[dave.clone()]);
        let profile = provider!(get_validator_profile, &dave).expect("the state yields a profile");
        assert_eq!(profile.status, ValidatorStatus::Active);
    });
}

/// The state read and write round trip within the record bounds.
#[test]
fn the_state_read_and_write_round_trip_within_the_bounds() {
    new_test_ext().execute_with(|| {
        let dave = account_key("Dave");
        assert!(!provider!(contains_validator_state, &dave));
        assert_eq!(provider!(get_validator_state, &dave), None);

        let history: Vec<EpochStats> = (0..105u32)
            .map(|epoch| EpochStats {
                epoch,
                ..Default::default()
            })
            .collect();
        let written = pallet_cerulea_dcf::traits::ValidatorState {
            last_active_epoch: 4,
            current: EpochStats {
                epoch: 4,
                final_score: 70,
                ..Default::default()
            },
            history,
            uptime: 9,
            inference_success_count: 8,
            participation_rate: 7,
            inference_count: 6,
            last_active_block: 5,
            name: Some(vec![b'n'; 40]),
            trust_score: 321,
        };
        provider!(update_validator_state, &dave, written);
        assert!(provider!(contains_validator_state, &dave));

        let read = provider!(get_validator_state, &dave).expect("the state was written");
        assert_eq!(read.last_active_epoch, 4);
        assert_eq!(read.current.final_score, 70);
        assert_eq!(read.history.len(), 100);
        assert_eq!(read.history.first().map(|stats| stats.epoch), Some(0));
        assert_eq!(read.uptime, 9);
        assert_eq!(read.inference_success_count, 8);
        assert_eq!(read.participation_rate, 7);
        assert_eq!(read.inference_count, 6);
        assert_eq!(read.last_active_block, 5);
        assert_eq!(read.name, Some(vec![b'n'; 32]));
        assert_eq!(read.trust_score, 321);

        provider!(remove_validator_state, &dave);
        assert!(!provider!(contains_validator_state, &dave));
    });
}

/// The name and metadata records round trip.
#[test]
fn the_name_and_metadata_records_round_trip() {
    new_test_ext().execute_with(|| {
        let dave = account_key("Dave");
        assert_eq!(provider!(get_validator_name, &dave), None);
        provider!(set_validator_name, &dave, vec![b'v'; 40]);
        assert_eq!(provider!(get_validator_name, &dave), Some(vec![b'v'; 32]));

        let metadata = pallet_cerulea_dcf::ValidatorMetadataInfo {
            name: BoundedVec::truncate_from(b"Cerulea Node One".to_vec()),
            website: None,
            contact: None,
            description: Some(BoundedVec::truncate_from(b"A Hyderabad node".to_vec())),
            location: None,
            commission_rate: Some(5),
            min_stake_required: Some(1_000),
            created_at: 1,
            updated_at: 2,
        };
        assert_eq!(provider!(get_validator_metadata, &dave), None);
        provider!(set_validator_metadata, &dave, metadata.clone());
        assert_eq!(provider!(get_validator_metadata, &dave), Some(metadata));
        provider!(remove_validator_metadata, &dave);
        assert_eq!(provider!(get_validator_metadata, &dave), None);
    });
}

/// The performance and activity counters round trip.
#[test]
fn the_performance_and_activity_counters_round_trip() {
    new_test_ext().execute_with(|| {
        let dave = account_key("Dave");
        let record = |epoch: u32| pallet_cerulea_dcf::PerformanceRecord {
            epoch,
            blocks_authored: 3,
            blocks_missed: 1,
            uptime_percentage: 90,
            inference_score: 4,
            stake_score: 5,
            final_score: 6,
            participation_rate: 80,
            timestamp: 7,
        };
        let history: Vec<_> = (0..105u32).map(record).collect();
        provider!(set_validator_performance_history, &dave, history);
        let read = provider!(get_validator_performance_history, &dave);
        assert_eq!(read.len(), 100);
        assert_eq!(read.first(), Some(&record(0)));
        provider!(remove_validator_performance_history, &dave);
        assert!(provider!(get_validator_performance_history, &dave).is_empty());

        provider!(set_validator_last_seen, &dave, 11);
        provider!(set_validator_blocks_authored, &dave, 12);
        provider!(set_validator_blocks_missed, &dave, 13);
        provider!(set_validator_join_time, &dave, 14);
        assert_eq!(provider!(get_validator_last_seen, &dave), 11);
        assert_eq!(provider!(get_validator_blocks_authored, &dave), 12);
        assert_eq!(provider!(get_validator_blocks_missed, &dave), 13);
        assert_eq!(provider!(get_validator_join_time, &dave), Some(14));

        provider!(remove_validator_last_seen, &dave);
        provider!(remove_validator_blocks_authored, &dave);
        provider!(remove_validator_blocks_missed, &dave);
        provider!(remove_validator_join_time, &dave);
        assert_eq!(provider!(get_validator_last_seen, &dave), 0);
        assert_eq!(provider!(get_validator_blocks_authored, &dave), 0);
        assert_eq!(provider!(get_validator_blocks_missed, &dave), 0);
        assert_eq!(provider!(get_validator_join_time, &dave), None);
    });
}

/// The pending action records round trip.
#[test]
fn the_pending_action_records_round_trip() {
    new_test_ext().execute_with(|| {
        let dave = account_key("Dave");
        assert!(provider!(get_pending_actions).is_empty());
        provider!(add_pending_action, &dave, ValidatorAction::Leave);
        assert_eq!(
            provider!(get_pending_actions),
            vec![(dave.clone(), ValidatorAction::Leave)]
        );
        provider!(remove_pending_action, &dave);
        assert!(provider!(get_pending_actions).is_empty());
    });
}

/// The key reads name the owner of a draw key and the seal key of a validator.
#[test]
fn the_key_reads_name_the_owner_and_the_seal() {
    new_test_ext().execute_with(|| {
        let alice = account_key("Alice");
        assert_eq!(
            provider!(draw_key_owner, &genesis_keys(1).draw),
            Some(alice.clone())
        );
        assert_eq!(provider!(draw_key_owner, &[0u8; 32]), None);
        assert_eq!(
            provider!(validator_seal_key, &alice),
            Some(genesis_keys(1).seal)
        );
        assert_eq!(provider!(validator_seal_key, &account_key("Dave")), None);
    });
}
