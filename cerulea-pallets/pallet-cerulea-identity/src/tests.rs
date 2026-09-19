//! The tests of the identity module.

use crate::{mock::*, Assignments, Error, HeldRoles, IdentityRegistry, Pallet, Units};
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
