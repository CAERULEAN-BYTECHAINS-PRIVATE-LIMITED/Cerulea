//! The tests of the identity module.

use crate::{
    mock::*, Assignments, CredentialFault, Error, HeldRoles, IdentityRegistry, Pallet, Units,
};
use frame_support::{assert_noop, assert_ok, BoundedVec};
use pallet_cerulea_schema::primitives::{name_of, BoundedName, DeploymentId};

const DEPLOYMENT: DeploymentId = [8u8; 32];
const STEWARD: u64 = 1;
const ANITHA: u64 = 2;
const BENNY: u64 = 3;

fn name(text: &str) -> BoundedName {
    name_of(text).expect("the name fits")
}

fn text(value: &str) -> BoundedVec<u8, frame_support::traits::ConstU32<128>> {
    BoundedVec::try_from(value.as_bytes().to_vec()).expect("the text fits")
}

fn credentials(names: &[&str]) -> BoundedVec<BoundedName, frame_support::traits::ConstU32<4>> {
    BoundedVec::try_from(names.iter().map(|held| name(held)).collect::<Vec<_>>())
        .expect("the credential list fits")
}

fn register(who: u64, identity: u8) {
    assert_ok!(Identity::register_account(
        RuntimeOrigin::signed(STEWARD),
        who,
        [identity; 32]
    ));
}

fn build_tree() {
    assert_ok!(Identity::create_unit(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        name("district"),
        None,
        text("Kollam District Administration"),
        text("The district administration holds the land register.")
    ));
    assert_ok!(Identity::create_unit(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        name("taluk"),
        Some(name("district")),
        text("Kollam Taluk Office"),
        text("The taluk office records land transfers.")
    ));
}

fn define_registrar() {
    assert_ok!(Identity::define_role(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        name("registrar"),
        Some(name("taluk")),
        credentials(&[]),
        text("Registrar")
    ));
}

#[test]
fn the_mock_runtime_implements_every_module_trait() {
    // The mock and the trait stay in step. A trait that grows past its
    // mock stops this test target from compiling.
    fn requires_registry<R: IdentityRegistry>() {}
    requires_registry::<Pallet<Test>>();
}

#[test]
fn a_registered_account_carries_its_identity_value() {
    new_test_ext().execute_with(|| {
        register(ANITHA, 7);
        assert!(<Pallet<Test> as IdentityRegistry>::is_registered(&ANITHA));
        assert_eq!(
            <Pallet<Test> as IdentityRegistry>::identity_of(&ANITHA),
            Some([7u8; 32])
        );
        assert!(!<Pallet<Test> as IdentityRegistry>::is_registered(&BENNY));
    });
}

#[test]
fn an_account_registers_once() {
    new_test_ext().execute_with(|| {
        register(ANITHA, 7);
        assert_noop!(
            Identity::register_account(RuntimeOrigin::signed(STEWARD), ANITHA, [8u8; 32]),
            Error::<Test>::AccountPresent
        );
    });
}

#[test]
fn a_suspended_account_resolves_to_no_role() {
    new_test_ext().execute_with(|| {
        register(ANITHA, 7);
        build_tree();
        define_registrar();
        assert_ok!(Identity::assign_role(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            ANITHA,
            name("registrar"),
            None
        ));
        assert!(<Pallet<Test> as IdentityRegistry>::has_role(
            &DEPLOYMENT,
            &ANITHA,
            &name("registrar"),
            START
        ));
        assert_ok!(Identity::suspend_account(
            RuntimeOrigin::signed(STEWARD),
            ANITHA
        ));
        assert!(!<Pallet<Test> as IdentityRegistry>::is_registered(&ANITHA));
        assert!(
            <Pallet<Test> as IdentityRegistry>::roles_of(&DEPLOYMENT, &ANITHA, START).is_empty()
        );
    });
}

#[test]
fn a_unit_chain_runs_to_the_top_of_the_tree() {
    new_test_ext().execute_with(|| {
        build_tree();
        let chain = <Pallet<Test> as IdentityRegistry>::unit_chain(&DEPLOYMENT, &name("taluk"));
        assert_eq!(chain, vec![name("taluk"), name("district")]);
        let top = <Pallet<Test> as IdentityRegistry>::unit_chain(&DEPLOYMENT, &name("district"));
        assert_eq!(top, vec![name("district")]);
    });
}

#[test]
fn a_unit_under_an_absent_parent_is_declined() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Identity::create_unit(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                name("taluk"),
                Some(name("district")),
                text("Kollam Taluk Office"),
                text("The taluk office records land transfers.")
            ),
            Error::<Test>::UnitUnknown
        );
    });
}

#[test]
fn a_unit_naming_itself_as_its_parent_is_declined() {
    new_test_ext().execute_with(|| {
        build_tree();
        assert_noop!(
            Identity::create_unit(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                name("district"),
                Some(name("district")),
                text("Kollam District Administration"),
                text("The district administration holds the land register.")
            ),
            Error::<Test>::UnitPresent
        );
    });
}

#[test]
fn a_tree_past_its_depth_is_declined() {
    new_test_ext().execute_with(|| {
        build_tree();
        assert_ok!(Identity::create_unit(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name("village"),
            Some(name("taluk")),
            text("Village Office"),
            text("The village office records survey numbers.")
        ));
        assert_ok!(Identity::create_unit(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name("ward"),
            Some(name("village")),
            text("Ward Office"),
            text("The ward office records households.")
        ));
        assert_noop!(
            Identity::create_unit(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                name("street"),
                Some(name("ward")),
                text("Street Office"),
                text("The street office records addresses.")
            ),
            Error::<Test>::UnitDepthReached
        );
    });
}

#[test]
fn a_role_belongs_to_the_unit_it_names() {
    new_test_ext().execute_with(|| {
        build_tree();
        define_registrar();
        assert_eq!(
            <Pallet<Test> as IdentityRegistry>::unit_of(&DEPLOYMENT, &name("registrar")),
            Some(name("taluk"))
        );
    });
}

#[test]
fn a_role_naming_an_absent_unit_is_declined() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Identity::define_role(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                name("registrar"),
                Some(name("taluk")),
                credentials(&[]),
                text("Registrar")
            ),
            Error::<Test>::UnitUnknown
        );
    });
}

#[test]
fn a_role_defines_once() {
    new_test_ext().execute_with(|| {
        build_tree();
        define_registrar();
        assert_noop!(
            Identity::define_role(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                name("registrar"),
                None,
                credentials(&[]),
                text("Registrar")
            ),
            Error::<Test>::RolePresent
        );
    });
}

#[test]
fn an_assignment_reaches_the_account_and_reads_back() {
    new_test_ext().execute_with(|| {
        register(ANITHA, 7);
        build_tree();
        define_registrar();
        assert_ok!(Identity::assign_role(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            ANITHA,
            name("registrar"),
            None
        ));
        assert_eq!(
            <Pallet<Test> as IdentityRegistry>::roles_of(&DEPLOYMENT, &ANITHA, START),
            vec![name("registrar")]
        );
        let held = Assignments::<Test>::get((DEPLOYMENT, ANITHA, name("registrar")))
            .expect("the registry holds the assignment");
        assert_eq!(held.granter, STEWARD);
        assert_eq!(held.granted_at, START);
    });
}

#[test]
fn an_assignment_past_its_expiry_resolves_to_nothing() {
    new_test_ext().execute_with(|| {
        register(ANITHA, 7);
        build_tree();
        define_registrar();
        assert_ok!(Identity::assign_role(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            ANITHA,
            name("registrar"),
            Some(START + 1_000)
        ));
        assert!(<Pallet<Test> as IdentityRegistry>::has_role(
            &DEPLOYMENT,
            &ANITHA,
            &name("registrar"),
            START + 500
        ));
        assert!(!<Pallet<Test> as IdentityRegistry>::has_role(
            &DEPLOYMENT,
            &ANITHA,
            &name("registrar"),
            START + 1_000
        ));
        set_clock(START + 5_000);
        assert!(<Pallet<Test> as IdentityRegistry>::roles_of(
            &DEPLOYMENT,
            &ANITHA,
            <Pallet<Test> as IdentityRegistry>::now_millis()
        )
        .is_empty());
    });
}

#[test]
fn an_expiry_in_the_past_is_declined() {
    new_test_ext().execute_with(|| {
        register(ANITHA, 7);
        build_tree();
        define_registrar();
        assert_noop!(
            Identity::assign_role(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                ANITHA,
                name("registrar"),
                Some(START)
            ),
            Error::<Test>::ExpiryInThePast
        );
    });
}

#[test]
fn an_assignment_to_an_absent_account_is_declined() {
    new_test_ext().execute_with(|| {
        build_tree();
        define_registrar();
        assert_noop!(
            Identity::assign_role(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                ANITHA,
                name("registrar"),
                None
            ),
            Error::<Test>::AccountUnknown
        );
    });
}

#[test]
fn an_assignment_of_an_absent_role_is_declined() {
    new_test_ext().execute_with(|| {
        register(ANITHA, 7);
        assert_noop!(
            Identity::assign_role(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                ANITHA,
                name("registrar"),
                None
            ),
            Error::<Test>::RoleUnknown
        );
    });
}

#[test]
fn a_repeated_assignment_is_declined() {
    new_test_ext().execute_with(|| {
        register(ANITHA, 7);
        build_tree();
        define_registrar();
        assert_ok!(Identity::assign_role(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            ANITHA,
            name("registrar"),
            None
        ));
        assert_noop!(
            Identity::assign_role(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                ANITHA,
                name("registrar"),
                None
            ),
            Error::<Test>::RoleAlreadyHeld
        );
    });
}

#[test]
fn a_revoked_assignment_leaves_the_registry() {
    new_test_ext().execute_with(|| {
        register(ANITHA, 7);
        build_tree();
        define_registrar();
        assert_ok!(Identity::assign_role(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            ANITHA,
            name("registrar"),
            None
        ));
        assert_ok!(Identity::revoke_role(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            ANITHA,
            name("registrar")
        ));
        assert!(HeldRoles::<Test>::get(DEPLOYMENT, ANITHA).is_empty());
        assert!(Assignments::<Test>::get((DEPLOYMENT, ANITHA, name("registrar"))).is_none());
        assert_noop!(
            Identity::revoke_role(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                ANITHA,
                name("registrar")
            ),
            Error::<Test>::RoleNotHeld
        );
    });
}

#[test]
fn two_roles_read_back_in_name_order() {
    new_test_ext().execute_with(|| {
        register(ANITHA, 7);
        build_tree();
        define_registrar();
        assert_ok!(Identity::define_role(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name("auditor"),
            None,
            credentials(&[]),
            text("Auditor")
        ));
        for role in ["registrar", "auditor"] {
            assert_ok!(Identity::assign_role(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                ANITHA,
                name(role),
                None
            ));
        }
        assert_eq!(
            <Pallet<Test> as IdentityRegistry>::roles_of(&DEPLOYMENT, &ANITHA, START),
            vec![name("auditor"), name("registrar")]
        );
    });
}

#[test]
fn the_stored_unit_carries_its_parent_and_its_title() {
    new_test_ext().execute_with(|| {
        build_tree();
        let held = Units::<Test>::get(DEPLOYMENT, name("taluk")).expect("the tree holds the unit");
        assert_eq!(held.parent, Some(name("district")));
        assert_eq!(held.title, text("Kollam Taluk Office"));
    });
}

fn define_surveyor() {
    assert_ok!(Identity::define_role(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        name("surveyor"),
        Some(name("taluk")),
        credentials(&["survey_licence"]),
        text("Surveyor")
    ));
}

fn attest(issuer: u64, who: u64, credential: &str, expires_at: Option<u64>) {
    assert_ok!(Identity::attest_credential(
        RuntimeOrigin::signed(issuer),
        DEPLOYMENT,
        who,
        name(credential),
        [1u8; 32],
        expires_at
    ));
}

#[test]
fn an_attestation_carries_its_issuer_and_its_expiry() {
    new_test_ext().execute_with(|| {
        register(ANITHA, 7);
        attest(BENNY, ANITHA, "survey_licence", Some(START + 10_000));
        let held = crate::Credentials::<Test>::get((DEPLOYMENT, ANITHA, name("survey_licence")))
            .expect("the registry holds the attestation");
        assert_eq!(held.issuer, BENNY);
        assert_eq!(held.issued_at, START);
        assert_eq!(held.expires_at, Some(START + 10_000));
        assert_eq!(held.evidence, [1u8; 32]);
        assert!(!held.withdrawn);
        assert!(<Pallet<Test> as IdentityRegistry>::has_credential(
            &DEPLOYMENT,
            &ANITHA,
            &name("survey_licence"),
            START
        ));
    });
}

#[test]
fn a_credential_past_its_expiry_counts_as_absent() {
    new_test_ext().execute_with(|| {
        register(ANITHA, 7);
        attest(BENNY, ANITHA, "survey_licence", Some(START + 10_000));
        assert!(<Pallet<Test> as IdentityRegistry>::has_credential(
            &DEPLOYMENT,
            &ANITHA,
            &name("survey_licence"),
            START + 9_999
        ));
        assert!(!<Pallet<Test> as IdentityRegistry>::has_credential(
            &DEPLOYMENT,
            &ANITHA,
            &name("survey_licence"),
            START + 10_000
        ));
        assert_eq!(
            Pallet::<Test>::credential_state(
                &DEPLOYMENT,
                &ANITHA,
                &name("survey_licence"),
                START + 20_000
            ),
            Err(CredentialFault::Expired)
        );
    });
}

#[test]
fn a_withdrawn_attestation_counts_as_absent() {
    new_test_ext().execute_with(|| {
        register(ANITHA, 7);
        attest(BENNY, ANITHA, "survey_licence", None);
        assert_ok!(Identity::revoke_credential(
            RuntimeOrigin::signed(BENNY),
            DEPLOYMENT,
            ANITHA,
            name("survey_licence")
        ));
        assert_eq!(
            Pallet::<Test>::credential_state(&DEPLOYMENT, &ANITHA, &name("survey_licence"), START),
            Err(CredentialFault::Withdrawn)
        );
    });
}

#[test]
fn only_the_issuer_withdraws_an_attestation() {
    new_test_ext().execute_with(|| {
        register(ANITHA, 7);
        attest(BENNY, ANITHA, "survey_licence", None);
        assert_noop!(
            Identity::revoke_credential(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                ANITHA,
                name("survey_licence")
            ),
            Error::<Test>::NotTheIssuer
        );
    });
}

#[test]
fn a_withdrawal_without_an_attestation_names_the_fix() {
    new_test_ext().execute_with(|| {
        register(ANITHA, 7);
        assert_noop!(
            Identity::revoke_credential(
                RuntimeOrigin::signed(BENNY),
                DEPLOYMENT,
                ANITHA,
                name("survey_licence")
            ),
            Error::<Test>::CredentialAbsent
        );
    });
}

#[test]
fn a_role_requiring_an_absent_credential_resolves_to_nothing() {
    new_test_ext().execute_with(|| {
        register(ANITHA, 7);
        build_tree();
        define_surveyor();
        assert_ok!(Identity::assign_role(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            ANITHA,
            name("surveyor"),
            None
        ));
        assert!(
            <Pallet<Test> as IdentityRegistry>::roles_of(&DEPLOYMENT, &ANITHA, START).is_empty()
        );
        let outcome = <Pallet<Test> as IdentityRegistry>::requirements_of(
            &DEPLOYMENT,
            &ANITHA,
            &name("surveyor"),
            START,
        );
        assert!(!outcome.holds());
        let (credential, fault) = outcome.first().expect("the outcome names the credential");
        assert_eq!(credential, &name("survey_licence"));
        assert_eq!(fault, &CredentialFault::Absent);
        assert!(!fault.fix().is_empty());
    });
}

#[test]
fn an_attested_credential_brings_the_role_back() {
    new_test_ext().execute_with(|| {
        register(ANITHA, 7);
        build_tree();
        define_surveyor();
        assert_ok!(Identity::assign_role(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            ANITHA,
            name("surveyor"),
            None
        ));
        attest(BENNY, ANITHA, "survey_licence", Some(START + 10_000));
        assert_eq!(
            <Pallet<Test> as IdentityRegistry>::roles_of(&DEPLOYMENT, &ANITHA, START),
            vec![name("surveyor")]
        );
        assert!(<Pallet<Test> as IdentityRegistry>::requirements_of(
            &DEPLOYMENT,
            &ANITHA,
            &name("surveyor"),
            START
        )
        .holds());
    });
}

#[test]
fn an_expired_credential_takes_the_role_away_without_a_further_call() {
    new_test_ext().execute_with(|| {
        register(ANITHA, 7);
        build_tree();
        define_surveyor();
        assert_ok!(Identity::assign_role(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            ANITHA,
            name("surveyor"),
            None
        ));
        attest(BENNY, ANITHA, "survey_licence", Some(START + 10_000));
        assert_eq!(
            <Pallet<Test> as IdentityRegistry>::roles_of(&DEPLOYMENT, &ANITHA, START),
            vec![name("surveyor")]
        );
        set_clock(START + 20_000);
        assert!(<Pallet<Test> as IdentityRegistry>::roles_of(
            &DEPLOYMENT,
            &ANITHA,
            <Pallet<Test> as IdentityRegistry>::now_millis()
        )
        .is_empty());
    });
}

#[test]
fn an_attestation_expiring_in_the_past_is_declined() {
    new_test_ext().execute_with(|| {
        register(ANITHA, 7);
        assert_noop!(
            Identity::attest_credential(
                RuntimeOrigin::signed(BENNY),
                DEPLOYMENT,
                ANITHA,
                name("survey_licence"),
                [1u8; 32],
                Some(START)
            ),
            Error::<Test>::ExpiryInThePast
        );
    });
}

#[test]
fn an_attestation_to_an_absent_account_is_declined() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Identity::attest_credential(
                RuntimeOrigin::signed(BENNY),
                DEPLOYMENT,
                ANITHA,
                name("survey_licence"),
                [1u8; 32],
                None
            ),
            Error::<Test>::AccountUnknown
        );
    });
}

#[test]
fn a_held_credential_reads_back_in_name_order() {
    new_test_ext().execute_with(|| {
        register(ANITHA, 7);
        attest(BENNY, ANITHA, "survey_licence", None);
        attest(BENNY, ANITHA, "audit_licence", None);
        assert_eq!(
            Pallet::<Test>::held_credentials(&DEPLOYMENT, &ANITHA),
            vec![name("audit_licence"), name("survey_licence")]
        );
    });
}

#[test]
fn a_repeated_attestation_replaces_the_earlier_one() {
    new_test_ext().execute_with(|| {
        register(ANITHA, 7);
        attest(BENNY, ANITHA, "survey_licence", Some(START + 1_000));
        attest(STEWARD, ANITHA, "survey_licence", Some(START + 50_000));
        let held = crate::Credentials::<Test>::get((DEPLOYMENT, ANITHA, name("survey_licence")))
            .expect("the registry holds the attestation");
        assert_eq!(held.issuer, STEWARD);
        assert_eq!(held.expires_at, Some(START + 50_000));
        assert_eq!(
            Pallet::<Test>::held_credentials(&DEPLOYMENT, &ANITHA).len(),
            1
        );
    });
}

#[test]
fn an_attestation_without_an_expiry_holds_until_a_withdrawal() {
    new_test_ext().execute_with(|| {
        register(ANITHA, 7);
        attest(BENNY, ANITHA, "survey_licence", None);
        let held = crate::Credentials::<Test>::get((DEPLOYMENT, ANITHA, name("survey_licence")))
            .expect("the registry holds the attestation");
        assert_eq!(held.horizon(), u64::MAX);
        assert!(<Pallet<Test> as IdentityRegistry>::has_credential(
            &DEPLOYMENT,
            &ANITHA,
            &name("survey_licence"),
            u64::MAX - 1
        ));
    });
}
